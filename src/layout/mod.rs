//! Layout: a taffy flex/grid tree turns measured text into concrete geometry.

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
    /// `false` when this source has no value for the row and is skipped.
    pub present: bool,
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
    /// Section heading this row belongs to; `None` rows form an untitled block.
    pub group: Option<String>,
    pub values: Vec<SeriesValue>,
    pub rme_text: Option<String>,
    pub tooltip: String,
}

#[derive(Debug, Clone, Copy)]
pub struct BarBox {
    /// Source index, used for the legend colour and the value lookup.
    pub series: usize,
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

/// A rendered section heading.
pub struct SectionBox {
    pub title: String,
    pub rect: Rect,
}

pub struct LayoutPlan {
    pub width: f32,
    pub height: f32,
    pub padding: f32,
    pub title: Rect,
    pub subtitle: Rect,
    pub legend: Vec<LegendBox>,
    pub sections: Vec<SectionBox>,
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
pub const NUMBER_SIZE: f32 = 13.0;

/// Number badge geometry, all in pixels. The badge is a cat: the digits sit on
/// its body, between the tail end and the head.
pub const NUMBER_BADGE_HEIGHT: f32 = 30.0;
/// Left pad: the tail occupies the first few pixels of the badge.
pub const NUMBER_BADGE_LEFT: f32 = 7.0;
/// Gap between the digit ink and the head.
pub const NUMBER_BADGE_GAP: f32 = 5.0;
/// Width reserved for the head (a 0.9-scaled copy of the 16px head path).
pub const NUMBER_BADGE_HEAD: f32 = 15.0;
/// Right pad after the head.
pub const NUMBER_BADGE_RIGHT: f32 = 2.0;
/// Vertical centre of the cat's body inside the 30px badge.
pub const NUMBER_BADGE_BODY_CENTER: f32 = 18.5;

/// Badge width for a digit string with `ink_width` pixels of ink.
pub fn number_badge_width(ink_width: f32) -> f32 {
    NUMBER_BADGE_LEFT + ink_width + NUMBER_BADGE_GAP + NUMBER_BADGE_HEAD + NUMBER_BADGE_RIGHT
}

/// Horizontal centre of the digit ink inside a badge starting at `badge_x`.
pub fn number_ink_center_x(badge_x: f32, ink_width: f32) -> f32 {
    badge_x + NUMBER_BADGE_LEFT + ink_width / 2.0
}
pub const LEGEND_SIZE: f32 = 11.0;
pub const SECTION_SIZE: f32 = 11.0;

mod badge;
mod engine;
mod section;

pub use engine::layout;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::Font;
    use std::path::Path;

    fn font() -> Option<Font> {
        let path = Path::new("fonts/mplus-rounded.ttf");
        if !path.is_file() {
            return None;
        }
        Some(Font::load(path).expect("font"))
    }

    fn row(label: &str, group: Option<&str>) -> RowPlan {
        RowPlan {
            label: label.to_owned(),
            note: Some("note".to_owned()),
            group: group.map(str::to_owned),
            values: vec![SeriesValue {
                text: "2.10 GiB/s".to_owned(),
                fraction: 1.0,
                present: true,
            }],
            rme_text: Some("±1.20%".to_owned()),
            tooltip: String::new(),
        }
    }

    fn options() -> LayoutOptions {
        LayoutOptions {
            width: 920.0,
            min_height: 0.0,
            card_height: 38.0,
            row_gap: 10.0,
            padding: 22.0,
            title: "blit benchmarks".to_owned(),
            subtitle: "Windows · Dart 3.13.5".to_owned(),
            footer: "generated".to_owned(),
            legend: Vec::new(),
        }
    }

    #[test]
    fn min_height_pins_the_footer() {
        let Some(font) = font() else { return };
        let mut opts = options();
        opts.min_height = 520.0;
        let plan = layout(&font, &[row("Sequential read", None)], &opts).expect("layout");
        assert!(
            (plan.footer.bottom() + plan.padding - plan.height).abs() < 2.0,
            "footer bottom {} vs card {}",
            plan.footer.bottom() + plan.padding,
            plan.height
        );
    }

    #[test]
    fn lays_out_rows_below_the_header() {
        let Some(font) = font() else { return };
        let plan = layout(&font, &[row("Sequential read", None)], &options()).expect("layout");
        assert_eq!(plan.width, 920.0);
        assert!(plan.rows[0].card.y > plan.title.y);
        assert!(plan.rows[0].bar_track_w() > 0.0);
        assert!(plan.rows[0].value.x > plan.rows[0].card.x);
        assert!(plan.height > 100.0);
        assert!(plan.sections.is_empty());
    }

    #[test]
    fn groups_become_sections_in_first_seen_order() {
        let Some(font) = font() else { return };
        let rows = vec![
            row("read", Some("I/O")),
            row("write", Some("I/O")),
            row("parse", Some("CPU")),
        ];
        let plan = layout(&font, &rows, &options()).expect("layout");
        assert_eq!(plan.sections.len(), 2);
        assert_eq!(plan.sections[0].title, "I/O");
        assert_eq!(plan.sections[1].title, "CPU");
        assert!(plan.sections[0].rect.y < plan.rows[0].card.y);
        assert!(plan.rows[1].card.y > plan.rows[0].card.y);
        assert!(plan.rows[2].card.y > plan.rows[1].card.y);
    }

    impl RowBox {
        fn bar_track_w(&self) -> f32 {
            self.bars.first().map(|bar| bar.track.w).unwrap_or(0.0)
        }
    }
}
