//! The taffy layout pass: header, one grid per section, footer.

use taffy::prelude::*;
use taffy::tree::{LayoutInput, LayoutOutput};

use crate::font::Font;

use super::badge::badge_geometry;
use super::section::sections_of;
use super::*;
// Shadow taffy's `Rect` (used fully qualified where needed).
use super::Rect;

pub fn layout(
    font: &Font,
    rows: &[RowPlan],
    options: &LayoutOptions,
) -> Result<LayoutPlan, LayoutError> {
    let content_width = options.width - options.padding * 2.0;
    let multi_series = rows.iter().any(|row| row.values.len() > 1);
    let series_count = rows.iter().map(|row| row.values.len()).max().unwrap_or(1);

    // The number badge is a cat: left pad (over the tail) + digit ink + gap +
    // head + right pad. Widths come from the glyph ink, so a 1-digit badge is
    // narrower than a 4-digit one; every badge is centred in one fixed slot.
    let max_digits = rows.len().max(1).to_string();
    let max_ink = font.text_ink(&max_digits, NUMBER_SIZE);
    let slot_width = number_badge_width(max_ink.right - max_ink.left);
    let number_gutter = 14.0 + slot_width + 12.0;

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

    // Legend: 8px below the header, one line of text, then 14px before the
    // first card. Without a legend the first card keeps its usual gap.
    let first_top = if options.legend.is_empty() {
        20.0
    } else {
        font.line_height_px(LEGEND_SIZE) + 22.0
    };

    let mut tree: TaffyTree<TextSpec> = TaffyTree::new();

    let root_style = Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        size: Size {
            width: length(options.width),
            height: auto(),
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
    tree.add_child(root, header)?;

    let mut cells_per_section: Vec<Vec<[NodeId; 4]>> = Vec::new();
    let mut grids: Vec<NodeId> = Vec::new();
    let mut title_nodes: Vec<Option<NodeId>> = Vec::new();

    for (section_index, (heading, row_indices)) in sections_of(rows).iter().enumerate() {
        let first_section = section_index == 0;
        if let Some(heading) = heading {
            let node = tree.new_leaf_with_context(
                Style {
                    margin: taffy::geometry::Rect {
                        left: length(14.0),
                        right: length(0.0),
                        top: length(if first_section { first_top } else { 18.0 }),
                        bottom: length(0.0),
                    },
                    ..Default::default()
                },
                TextSpec::line(heading.clone(), SECTION_SIZE),
            )?;
            tree.add_child(root, node)?;
            title_nodes.push(Some(node));
        } else {
            title_nodes.push(None);
        }

        let mut grid_children: Vec<NodeId> = Vec::with_capacity(row_indices.len() * 4);
        let mut cells: Vec<[NodeId; 4]> = Vec::with_capacity(row_indices.len());
        for rows_index in row_indices {
            let row = &rows[*rows_index];
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
            cells.push([label, bar, value, rme]);
        }

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
            align_content: AlignContent::START,
            margin: taffy::geometry::Rect {
                left: length(0.0),
                right: length(0.0),
                top: length(if heading.is_some() {
                    4.0
                } else if first_section {
                    first_top
                } else {
                    18.0
                }),
                bottom: length(0.0),
            },
            ..Default::default()
        };
        let grid = tree.new_with_children(grid_style, &grid_children)?;
        tree.add_child(root, grid)?;
        grids.push(grid);
        cells_per_section.push(cells);
    }

    // Absorb free space with a spacer: the footer is pushed down after the
    // layout pass when `min_height` is larger than the content (taffy clamps
    // `min_size` after the flex pass, so growing inside the tree is unreliable).
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
    tree.add_child(root, footer)?;

    let available = Size {
        width: AvailableSpace::Definite(options.width),
        height: AvailableSpace::MaxContent,
    };
    let mut measure =
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
        };
    tree.compute_layout_with_measure(root, available, &mut measure)?;
    let root_box = tree.layout(root)?;
    let header_box = tree.layout(header)?;
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

    // The legend is drawn by hand on a fixed baseline; the first section has
    // already reserved `legend_height` for it through its top margin.
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

    let mut sections: Vec<SectionBox> = Vec::new();
    let mut row_boxes: Vec<RowBox> = Vec::new();

    let grouped = sections_of(rows);
    for (section_index, (heading, row_indices)) in grouped.iter().enumerate() {
        if let (Some(title_node), Some(heading)) = (title_nodes[section_index], heading) {
            let box_ = tree.layout(title_node)?;
            sections.push(SectionBox {
                title: heading.clone(),
                rect: Rect {
                    x: box_.location.x,
                    y: box_.location.y,
                    w: box_.size.width,
                    h: box_.size.height,
                },
            });
        }

        let grid_box = tree.layout(grids[section_index])?;
        for (cell_index, cells) in cells_per_section[section_index].iter().enumerate() {
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

            let rows_index = row_indices[cell_index];
            let values = &rows[rows_index].values;
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

            let ordinal = row_boxes.len() + 1;
            let badge = badge_geometry(card, slot_width, ordinal, font);
            row_boxes.push(RowBox {
                card,
                number_pill: Rect {
                    x: badge.x,
                    y: badge.y,
                    w: badge.w,
                    h: badge.h,
                },
                number_center: (
                    number_ink_center_x(badge.x, badge.ink_width),
                    badge.y + NUMBER_BADGE_BODY_CENTER,
                ),
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
                row: rows[rows_index].clone(),
            });
        }
    }

    let mut plan = LayoutPlan {
        width,
        height,
        padding: options.padding,
        title: title_rect,
        subtitle: subtitle_rect,
        legend,
        sections,
        rows: row_boxes,
        footer: Rect {
            x: footer_box.location.x,
            y: footer_box.location.y,
            w: footer_box.size.width,
            h: footer_box.size.height,
        },
        card_height,
        multi_series,
    };

    // `min_height` keeps the footer on the bottom edge while rows stay top
    // aligned: the extra height goes to the gap above the footer.
    if plan.height < options.min_height {
        let extra = options.min_height - plan.height;
        plan.height = options.min_height;
        plan.footer.y += extra;
    }
    Ok(plan)
}
