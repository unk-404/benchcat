//! Layout: a taffy flex/grid tree turns measured text into concrete geometry.

use taffy::prelude::*;
use taffy::tree::{LayoutInput, LayoutOutput};

use crate::font::Font;

/// Text content attached to a leaf so taffy can measure it.
#[derive(Debug, Clone, Default)]
pub struct TextSpec {
    /// `(text, font size)` pairs rendered as stacked lines.
    pub lines: Vec<(String, f32)>,
}

impl TextSpec {
    pub fn line(text: impl Into<String>, size: f32) -> Self {
        Self {
            lines: vec![(text.into(), size)],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

/// One value of one row, already formatted and scaled to 0..1 of the row max.
#[derive(Debug, Clone)]
pub struct SeriesValue {
    pub text: String,
    pub fraction: f32,
}

/// A legend entry: a colour and the source it identifies.
#[derive(Debug, Clone)]
pub struct LegendItem {
    pub label: String,
    pub color: String,
}

/// One rendered row.
#[derive(Debug, Clone)]
pub struct RowPlan {
    pub label: String,
    pub note: Option<String>,
    pub values: Vec<SeriesValue>,
    pub rme_text: Option<String>,
    pub tooltip: String,
}

#[derive(Debug, Clone, Copy)]
pub struct BarBox {
    pub track: Rect,
    pub fill: Rect,
}

pub struct RowBox {
    pub card: Rect,
    pub number_pill: Rect,
    pub number_center: (f32, f32),
    pub label: Rect,
    pub bars: Vec<BarBox>,
    pub value: Rect,
    pub rme: Rect,
    pub row: RowPlan,
}

pub struct LegendBox {
    pub dot: (f32, f32),
    pub label: Rect,
    pub color: String,
    pub text: String,
}

pub struct LayoutPlan {
    pub width: f32,
    pub height: f32,
    pub padding: f32,
    pub title: Rect,
    pub subtitle: Rect,
    pub legend: Vec<LegendBox>,
    pub rows: Vec<RowBox>,
    pub footer: Rect,
    pub card_height: f32,
    pub multi_series: bool,
}

pub struct LayoutOptions {
    pub width: f32,
    pub min_height: f32,
    pub card_height: f32,
    pub row_gap: f32,
    pub padding: f32,
    pub title: String,
    pub subtitle: String,
    pub footer: String,
    pub legend: Vec<LegendItem>,
}

#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    #[error("layout failed: {0}")]
    Taffy(#[from] taffy::TaffyError),
}

pub const TITLE_SIZE: f32 = 28.0;
pub const SUBTITLE_SIZE: f32 = 13.0;
pub const LABEL_SIZE: f32 = 14.0;
pub const NOTE_SIZE: f32 = 10.5;
pub const VALUE_SIZE: f32 = 13.5;
pub const SERIES_VALUE_SIZE: f32 = 10.5;
pub const RME_SIZE: f32 = 10.0;
pub const FOOTER_SIZE: f32 = 10.5;
pub const NUMBER_SIZE: f32 = 11.0;
pub const LEGEND_SIZE: f32 = 11.0;

pub fn layout(
    font: &Font,
    rows: &[RowPlan],
    options: &LayoutOptions,
) -> Result<LayoutPlan, LayoutError> {
    let content_width = options.width - options.padding * 2.0;
    let multi_series = rows.iter().any(|row| row.values.len() > 1);
    let series_count = rows.iter().map(|row| row.values.len()).max().unwrap_or(1);

    // The number chip is a circle for 1-2 digits and grows into a pill for
    // 100+, so three-digit indices never spill outside their badge.
    let number_text = rows.len().max(1).to_string();
    let pill_width = (font.text_width(&number_text, NUMBER_SIZE) + 12.0).max(18.0);
    let number_gutter = 14.0 + pill_width + 12.0;

    let label_text_width = rows
        .iter()
        .map(|row| {
            let label = font.text_width(&row.label, LABEL_SIZE);
            let note = row
                .note
                .as_deref()
                .map_or(0.0, |note| font.text_width(note, NOTE_SIZE));
            label.max(note)
        })
        .fold(0.0f32, f32::max);
    let value_width = rows
        .iter()
        .flat_map(|row| {
            let size = if multi_series {
                SERIES_VALUE_SIZE
            } else {
                VALUE_SIZE
            };
            row.values
                .iter()
                .map(move |value| font.text_width(&value.text, size))
        })
        .fold(0.0f32, f32::max);
    let rme_width = if multi_series {
        0.0
    } else {
        rows.iter()
            .filter_map(|row| row.rme_text.as_deref())
            .map(|text| font.text_width(text, RME_SIZE))
            .fold(0.0f32, f32::max)
    };

    // Card height follows its content: the tallest of the bar stack and the
    // label block, so several sources get real breathing room instead of
    // squeezed lines.
    let series_pitch = if multi_series {
        (font.line_height_px(SERIES_VALUE_SIZE) + 3.0).max(13.0)
    } else {
        0.0
    };
    let bar_stack_height = |count: usize| -> f32 {
        if count <= 1 {
            10.0
        } else {
            (count as f32 - 1.0) * series_pitch + 6.0
        }
    };
    let card_height = if multi_series {
        let bars = bar_stack_height(series_count);
        let label_block = font.line_height_px(LABEL_SIZE) + font.line_height_px(NOTE_SIZE);
        (bars + 20.0)
            .max(label_block + 18.0)
            .max(options.card_height)
    } else {
        options.card_height
    };

    let label_w = (number_gutter + label_text_width + 16.0).clamp(180.0, content_width * 0.46);
    let value_w = (value_width + 20.0).clamp(96.0, content_width * 0.30);
    let rme_w = if multi_series {
        4.0
    } else {
        // Room for the right inset so the ±RME label never touches the edge.
        (rme_width + 26.0).max(52.0)
    };
    let col_gap = 12.0;
    let bar_w = (content_width - label_w - value_w - rme_w - col_gap * 3.0).max(120.0);

    let legend_height = if options.legend.is_empty() {
        0.0
    } else {
        font.line_height_px(LEGEND_SIZE) + 12.0
    };

    let mut tree: TaffyTree<TextSpec> = TaffyTree::new();

    let root_style = Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        size: Size {
            width: length(options.width),
            height: auto(),
        },
        min_size: Size {
            width: auto(),
            height: length(options.min_height),
        },
        padding: taffy::geometry::Rect {
            left: length(options.padding),
            right: length(options.padding),
            top: length(options.padding),
            bottom: length(options.padding),
        },
        ..Default::default()
    };
    let root = tree.new_leaf(root_style)?;

    let title = tree.new_leaf_with_context(
        Style::default(),
        TextSpec::line(options.title.clone(), TITLE_SIZE),
    )?;
    let subtitle = tree.new_leaf_with_context(
        Style::default(),
        TextSpec::line(options.subtitle.clone(), SUBTITLE_SIZE),
    )?;
    tree.set_style(
        subtitle,
        Style {
            margin: taffy::geometry::Rect {
                left: length(0.0),
                right: length(0.0),
                top: length(6.0),
                bottom: length(0.0),
            },
            ..Default::default()
        },
    )?;

    let header_style = Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        ..Default::default()
    };
    let header = tree.new_with_children(header_style, &[title, subtitle])?;

    let grid_style = Style {
        display: Display::Grid,
        grid_template_columns: vec![
            length(label_w),
            length(bar_w),
            length(value_w),
            length(rme_w),
        ],
        grid_auto_rows: vec![length(card_height)],
        gap: Size {
            width: length(col_gap),
            height: length(options.row_gap),
        },
        // Absorb the free space a `min_height` creates: the footer stays pinned
        // to the bottom and the rows stay top-aligned (centring looks worse).
        flex_grow: 1.0,
        align_content: AlignContent::START,
        margin: taffy::geometry::Rect {
            left: length(0.0),
            right: length(0.0),
            top: length(20.0 + legend_height),
            bottom: length(0.0),
        },
        ..Default::default()
    };

    let mut cell_nodes: Vec<[NodeId; 4]> = Vec::with_capacity(rows.len());
    let mut grid_children: Vec<NodeId> = Vec::with_capacity(rows.len() * 4);
    for row in rows {
        let label_spec = match row.note.as_deref() {
            Some(note) => TextSpec {
                lines: vec![
                    (row.label.clone(), LABEL_SIZE),
                    (note.to_owned(), NOTE_SIZE),
                ],
            },
            None => TextSpec::line(row.label.clone(), LABEL_SIZE),
        };
        let label = tree.new_leaf_with_context(Style::default(), label_spec)?;
        let bar = tree.new_leaf(Style::default())?;
        let value = tree.new_leaf_with_context(
            Style::default(),
            TextSpec::line(
                row.values
                    .first()
                    .map(|value| value.text.clone())
                    .unwrap_or_default(),
                if multi_series {
                    SERIES_VALUE_SIZE
                } else {
                    VALUE_SIZE
                },
            ),
        )?;
        let rme = tree.new_leaf_with_context(
            Style::default(),
            TextSpec::line(row.rme_text.clone().unwrap_or_default(), RME_SIZE),
        )?;
        for node in [label, bar, value, rme] {
            grid_children.push(node);
        }
        cell_nodes.push([label, bar, value, rme]);
    }
    let grid = tree.new_with_children(grid_style, &grid_children)?;

    let footer_style = Style {
        margin: taffy::geometry::Rect {
            left: length(0.0),
            right: length(0.0),
            top: length(22.0),
            bottom: length(0.0),
        },
        ..Default::default()
    };
    let footer = tree.new_leaf_with_context(
        footer_style,
        TextSpec::line(options.footer.clone(), FOOTER_SIZE),
    )?;

    tree.add_child(root, header)?;
    tree.add_child(root, grid)?;
    tree.add_child(root, footer)?;

    let available = Size {
        width: AvailableSpace::Definite(options.width),
        height: AvailableSpace::MaxContent,
    };
    tree.compute_layout_with_measure(
        root,
        available,
        |input: LayoutInput, _node: NodeId, context: Option<&mut TextSpec>, _style: &Style| {
            let Some(spec) = context else {
                return LayoutOutput::from_outer_size(Size::ZERO);
            };
            let mut width = 0.0f32;
            let mut height = 0.0f32;
            for (text, size) in &spec.lines {
                width = width.max(font.text_width(text, *size));
                height += font.line_height_px(*size);
            }
            if let Some(known) = input.known_dimensions.width {
                width = width.min(known);
            }
            LayoutOutput::from_outer_size(Size { width, height })
        },
    )?;

    let root_box = tree.layout(root)?;
    let header_box = tree.layout(header)?;
    let grid_box = tree.layout(grid)?;
    let title_box = tree.layout(title)?;
    let subtitle_box = tree.layout(subtitle)?;
    let footer_box = tree.layout(footer)?;

    let width = root_box.size.width;
    let height = root_box.size.height;

    let title_rect = Rect {
        x: header_box.location.x + title_box.location.x,
        y: header_box.location.y + title_box.location.y,
        w: title_box.size.width,
        h: title_box.size.height,
    };
    let subtitle_rect = Rect {
        x: header_box.location.x + subtitle_box.location.x,
        y: header_box.location.y + subtitle_box.location.y,
        w: subtitle_box.size.width,
        h: subtitle_box.size.height,
    };

    // The legend is drawn by hand on a fixed baseline; the grid already
    // reserved `legend_height` for it through its top margin.
    let mut legend = Vec::with_capacity(options.legend.len());
    if !options.legend.is_empty() {
        let legend_y = header_box.location.y + header_box.size.height + 8.0;
        let mut cursor = options.padding;
        for item in &options.legend {
            let text_width = font.text_width(&item.label, LEGEND_SIZE);
            legend.push(LegendBox {
                dot: (
                    cursor + 4.0,
                    legend_y + font.line_height_px(LEGEND_SIZE) / 2.0,
                ),
                label: Rect {
                    x: cursor + 15.0,
                    y: legend_y,
                    w: text_width,
                    h: font.line_height_px(LEGEND_SIZE),
                },
                color: item.color.clone(),
                text: item.label.clone(),
            });
            cursor += 15.0 + text_width + 22.0;
        }
    }

    let mut row_boxes = Vec::with_capacity(rows.len());
    for (index, cells) in cell_nodes.iter().enumerate() {
        let [label_node, bar_node, value_node, rme_node] = *cells;
        let label_box = tree.layout(label_node)?;
        let bar_box = tree.layout(bar_node)?;
        let value_box = tree.layout(value_node)?;
        let rme_box = tree.layout(rme_node)?;
        let card_y = grid_box.location.y + label_box.location.y;
        let card = Rect {
            x: grid_box.location.x,
            y: card_y,
            w: grid_box.size.width,
            h: label_box.size.height.max(card_height),
        };
        let track_full = Rect {
            x: grid_box.location.x + bar_box.location.x,
            y: card.y,
            w: bar_box.size.width,
            h: card.h,
        };

        let values = &rows[index].values;
        let count = values.len().max(1);
        let (bar_height, gap) = if count == 1 {
            (10.0, 0.0)
        } else {
            (6.0, series_pitch - 6.0)
        };
        let content = count as f32 * bar_height + (count as f32 - 1.0) * gap;
        let top = card.y + (card.h - content) / 2.0;
        let mut bars = Vec::with_capacity(count);
        for (series, value) in values.iter().enumerate() {
            let y = top + series as f32 * (bar_height + gap);
            let fraction = value.fraction.clamp(0.0, 1.0);
            let track = Rect {
                x: track_full.x,
                y,
                w: track_full.w,
                h: bar_height,
            };
            bars.push(BarBox {
                track,
                fill: Rect {
                    x: track.x,
                    y,
                    w: if fraction <= 0.0 {
                        0.0
                    } else {
                        (track.w * fraction).clamp(4.0, track.w)
                    },
                    h: bar_height,
                },
            });
        }
        if bars.is_empty() {
            let y = card.y + (card.h - 10.0) / 2.0;
            bars.push(BarBox {
                track: Rect {
                    x: track_full.x,
                    y,
                    w: track_full.w,
                    h: 10.0,
                },
                fill: Rect {
                    x: track_full.x,
                    y,
                    w: 4.0,
                    h: 10.0,
                },
            });
        }

        row_boxes.push(RowBox {
            card,
            number_pill: Rect {
                x: card.x + 14.0,
                y: card.y + (card.h - 18.0) / 2.0,
                w: pill_width,
                h: 18.0,
            },
            number_center: (card.x + 14.0 + pill_width / 2.0, card.y + card.h / 2.0),
            label: Rect {
                x: grid_box.location.x + label_box.location.x + number_gutter,
                y: card_y,
                w: label_box.size.width - number_gutter,
                h: card.h,
            },
            bars,
            value: Rect {
                x: grid_box.location.x + value_box.location.x,
                y: card_y,
                w: value_box.size.width,
                h: card.h,
            },
            rme: Rect {
                x: grid_box.location.x + rme_box.location.x,
                y: card_y,
                w: rme_box.size.width,
                h: card.h,
            },
            row: rows[index].clone(),
        });
    }

    Ok(LayoutPlan {
        width,
        height,
        padding: options.padding,
        title: title_rect,
        subtitle: subtitle_rect,
        legend,
        rows: row_boxes,
        footer: Rect {
            x: footer_box.location.x,
            y: footer_box.location.y,
            w: footer_box.size.width,
            h: footer_box.size.height,
        },
        card_height,
        multi_series,
    })
}
