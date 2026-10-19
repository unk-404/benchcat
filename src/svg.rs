//! SVG assembly. Text is emitted as glyph outlines so the viewer never needs the
//! font and GitHub can never substitute a different typeface.

use std::collections::HashMap;

use crate::font::Font;
use crate::layout::{
    FOOTER_SIZE, LABEL_SIZE, LEGEND_SIZE, LayoutPlan, NOTE_SIZE, NUMBER_SIZE, RME_SIZE,
    SECTION_SIZE, SERIES_VALUE_SIZE, SUBTITLE_SIZE, TITLE_SIZE, VALUE_SIZE,
};

/// Palette and decoration settings for one visual theme.
pub struct Theme {
    pub bg: [&'static str; 3],
    pub title: &'static str,
    pub subtitle: &'static str,
    pub footer: &'static str,
    pub dot_colors: &'static [&'static str],
    pub series_colors: &'static [&'static str],
    pub number: &'static str,
    pub label: &'static str,
    pub note: &'static str,
    pub track: &'static str,
    pub value: &'static str,
    pub rme: &'static str,
    pub card: &'static str,
    pub card_opacity: f32,
    pub shadow: &'static str,
    pub shadow_opacity: f32,
    pub star: &'static str,
    pub sparkle: &'static str,
    pub kaomoji: &'static str,
}

pub const PASTEL: Theme = Theme {
    bg: ["#FFF8F0", "#FFF0F6", "#EEF7FF"],
    title: "#4A3542",
    subtitle: "#8A7080",
    footer: "#A88A98",
    dot_colors: &[
        "#FFB7C5", "#A8E6CF", "#BDE0FE", "#FFD6A5", "#CDB4DB", "#FFC8DD", "#B9FBC0",
    ],
    series_colors: &["#F4A6BE", "#7FD1B9", "#8CB8F0", "#F2B984", "#B49BE0"],
    number: "#6B4A5B",
    label: "#4A3542",
    note: "#9A7F8D",
    track: "#F3E5EC",
    value: "#4A3542",
    rme: "#A88A98",
    card: "#FFFFFF",
    card_opacity: 0.78,
    shadow: "#C084A0",
    shadow_opacity: 0.18,
    star: "#FFD166",
    sparkle: "#F4A7B9",
    kaomoji: "#B07C96",
};

pub const DARK: Theme = Theme {
    bg: ["#241C26", "#1F1A2B", "#191F2B"],
    title: "#FAEDF3",
    subtitle: "#C9B4C2",
    footer: "#9E8A99",
    dot_colors: &[
        "#FFB7C5", "#A8E6CF", "#BDE0FE", "#FFD6A5", "#CDB4DB", "#FFC8DD", "#B9FBC0",
    ],
    series_colors: &["#FF9EBC", "#7FE0C4", "#93C5FD", "#FFC98B", "#C4A9F0"],
    number: "#3B2B36",
    label: "#FAEDF3",
    note: "#BBA3B2",
    track: "#3A3140",
    value: "#FAEDF3",
    rme: "#B49EAC",
    card: "#3A3243",
    card_opacity: 1.0,
    shadow: "#000000",
    shadow_opacity: 0.45,
    star: "#FFD166",
    sparkle: "#F4A7B9",
    kaomoji: "#E3B6C9",
};

pub fn theme(name: &str) -> Option<&'static Theme> {
    match name {
        "pastel" | "" => Some(&PASTEL),
        "dark" | "pastel-dark" => Some(&DARK),
        _ => None,
    }
}

pub struct RenderInput<'a> {
    pub title: &'a str,
    pub subtitle: &'a str,
    pub footer: &'a str,
    pub kaomoji: &'a str,
}

pub struct RenderOutput {
    pub svg: String,
    pub warnings: Vec<String>,
}

/// Every glyph used by the document, in first-use order, with its path.
///
/// Building the atlas before emitting lets outlines be extracted in parallel
/// while the document itself stays in strict reading order.
struct Atlas {
    index: HashMap<u16, usize>,
    paths: Vec<String>,
}

impl Atlas {
    fn build(font: &Font, texts: &[(&str, f32)]) -> (Self, Vec<String>) {
        let mut index: HashMap<u16, usize> = HashMap::new();
        let mut gids: Vec<u16> = Vec::new();
        let mut missing: Vec<char> = Vec::new();
        for (text, _) in texts {
            for ch in text.chars() {
                match font.glyph(ch) {
                    Some(glyph) => {
                        if let std::collections::hash_map::Entry::Vacant(slot) =
                            index.entry(glyph.gid)
                        {
                            slot.insert(gids.len());
                            gids.push(glyph.gid);
                        }
                    }
                    None => missing.push(ch),
                }
            }
        }
        missing.sort_unstable();
        missing.dedup();
        let paths = font.extract_paths(&gids);
        let warnings = if missing.is_empty() {
            Vec::new()
        } else {
            let chars: String = missing.iter().collect();
            vec![format!("font {:?} has no glyph for {chars:?}", font.family)]
        };
        (Self { index, paths }, warnings)
    }

    fn defs(&self) -> String {
        let mut out = String::with_capacity(self.paths.len() * 200);
        out.push_str("<defs>");
        for (id, path) in self.paths.iter().enumerate() {
            if path.is_empty() {
                continue;
            }
            out.push_str("<path id=\"g");
            out.push_str(&id.to_string());
            out.push_str("\" d=\"");
            out.push_str(path);
            out.push_str("\"/>");
        }
        out
    }
}

#[derive(Clone, Copy)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

pub fn render(font: &Font, plan: &LayoutPlan, input: &RenderInput, theme: &Theme) -> RenderOutput {
    let numbers: Vec<String> = (1..=plan.rows.len())
        .map(|index| index.to_string())
        .collect();
    let mut texts: Vec<(&str, f32)> = Vec::with_capacity(plan.rows.len() * 5 + 8);
    texts.push((input.title, TITLE_SIZE));
    texts.push((input.subtitle, SUBTITLE_SIZE));
    for item in &plan.legend {
        texts.push((item.text.as_str(), LEGEND_SIZE));
    }
    for section in &plan.sections {
        texts.push((section.title.as_str(), SECTION_SIZE));
    }
    for row in &plan.rows {
        texts.push((row.row.label.as_str(), LABEL_SIZE));
        if let Some(note) = row.row.note.as_deref() {
            texts.push((note, NOTE_SIZE));
        }
        if plan.multi_series {
            for value in &row.row.values {
                texts.push((value.text.as_str(), SERIES_VALUE_SIZE));
            }
        } else if let Some(value) = row.row.values.first() {
            texts.push((value.text.as_str(), VALUE_SIZE));
        }
        if let Some(rme) = row.row.rme_text.as_deref() {
            texts.push((rme, RME_SIZE));
        }
    }
    for number in &numbers {
        texts.push((number.as_str(), NUMBER_SIZE));
    }
    texts.push((input.footer, FOOTER_SIZE));

    let (atlas, warnings) = Atlas::build(font, &texts);

    let mut body = String::with_capacity(64 * 1024);

    let title_baseline = plan.title.y + font.ascent_px(TITLE_SIZE);
    push_text(
        &mut body,
        &atlas,
        font,
        input.title,
        plan.title.x,
        title_baseline,
        TITLE_SIZE,
        theme.title,
        Anchor::Start,
    );
    let subtitle_baseline = plan.subtitle.y + font.ascent_px(SUBTITLE_SIZE);
    push_text(
        &mut body,
        &atlas,
        font,
        input.subtitle,
        plan.subtitle.x,
        subtitle_baseline,
        SUBTITLE_SIZE,
        theme.subtitle,
        Anchor::Start,
    );

    for item in &plan.legend {
        body.push_str(&format!(
            "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"4\" fill=\"{color}\"/>",
            x = item.dot.0,
            y = item.dot.1,
            color = item.color,
        ));
        push_text(
            &mut body,
            &atlas,
            font,
            &item.text,
            item.label.x,
            item.label.y + font.ascent_px(LEGEND_SIZE),
            LEGEND_SIZE,
            theme.note,
            Anchor::Start,
        );
    }

    for section in &plan.sections {
        push_text(
            &mut body,
            &atlas,
            font,
            &section.title,
            section.rect.x,
            section.rect.y + font.ascent_px(SECTION_SIZE),
            SECTION_SIZE,
            theme.note,
            Anchor::Start,
        );
    }

    for (index, row) in plan.rows.iter().enumerate() {
        let card = row.card;
        body.push_str("<g><title>");
        body.push_str(&escape(&row.row.tooltip));
        body.push_str("</title>");
        body.push_str(&format!(
            "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"12\" \
             fill=\"{}\" fill-opacity=\"{}\"/>",
            theme.card,
            theme.card_opacity,
            x = card.x,
            y = card.y,
            w = card.w,
            h = card.h,
        ));
        let accent = theme.dot_colors[index % theme.dot_colors.len()];
        let pill = row.number_pill;
        number_cat(&mut body, pill, accent);
        // Centre on the glyph ink, not on the em box, so every index sits
        // optically centred however many digits it has.
        let number_ink = font.text_ink(&numbers[index], NUMBER_SIZE);
        push_text(
            &mut body,
            &atlas,
            font,
            &numbers[index],
            row.number_center.0 - number_ink.center_x(),
            row.number_center.1 + number_ink.center_up(),
            NUMBER_SIZE,
            theme.number,
            Anchor::Start,
        );

        // Stack label and note by their ink boxes, not their line boxes: the
        // font's full ascent/descent would push the pair against the card edge
        // and leave a visible gap between the two lines.
        let label_ink = font.text_ink(&row.row.label, LABEL_SIZE);
        let (label_baseline, note_baseline) = match row.row.note.as_deref() {
            Some(note) => {
                let note_ink = font.text_ink(note, NOTE_SIZE);
                // Space the two lines from their real ink edges, not from the
                // font's line boxes.
                let gap = (LABEL_SIZE * 0.36).max(4.0);
                let total =
                    (label_ink.top - label_ink.bottom) + gap + (note_ink.top - note_ink.bottom);
                let top = card.y + (card.h - total) / 2.0;
                let label_baseline = top + label_ink.top;
                let note_baseline = label_baseline - label_ink.bottom + gap + note_ink.top;
                (label_baseline, Some(note_baseline))
            }
            None => (card.y + card.h / 2.0 + label_ink.center_up(), None),
        };
        push_text(
            &mut body,
            &atlas,
            font,
            &row.row.label,
            row.label.x,
            label_baseline,
            LABEL_SIZE,
            theme.label,
            Anchor::Start,
        );
        if let (Some(note), Some(note_baseline)) = (row.row.note.as_deref(), note_baseline) {
            push_text(
                &mut body,
                &atlas,
                font,
                note,
                row.label.x,
                note_baseline,
                NOTE_SIZE,
                theme.note,
                Anchor::Start,
            );
        }

        for (series, bar) in row.bars.iter().enumerate() {
            let color = if plan.multi_series {
                plan.legend
                    .get(series)
                    .map(|item| item.color.as_str())
                    .unwrap_or(accent)
            } else {
                accent
            };
            body.push_str(&format!(
                "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"3\" fill=\"{}\"/>",
                theme.track,
                x = bar.track.x,
                y = bar.track.y,
                w = bar.track.w,
                h = bar.track.h,
            ));
            body.push_str(&format!(
                "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"3\" fill=\"{color}\"/>",
                x = bar.fill.x,
                y = bar.fill.y,
                w = bar.fill.w,
                h = bar.fill.h,
            ));
            if plan.multi_series
                && let Some(value) = row.row.values.get(series)
            {
                let baseline =
                    bar.track.y + bar.track.h / 2.0 + font.ascent_px(SERIES_VALUE_SIZE) / 2.0 - 1.5;
                push_text(
                    &mut body,
                    &atlas,
                    font,
                    &value.text,
                    row.value.x,
                    baseline,
                    SERIES_VALUE_SIZE,
                    theme.value,
                    Anchor::Start,
                );
            }
        }

        if !plan.multi_series {
            if let Some(value) = row.row.values.first() {
                let ink = font.text_ink(&value.text, VALUE_SIZE);
                push_text(
                    &mut body,
                    &atlas,
                    font,
                    &value.text,
                    row.value.x,
                    card.y + card.h / 2.0 + ink.center_up(),
                    VALUE_SIZE,
                    theme.value,
                    Anchor::Start,
                );
            }
            if let Some(rme) = row.row.rme_text.as_deref() {
                let ink = font.text_ink(rme, RME_SIZE);
                push_text(
                    &mut body,
                    &atlas,
                    font,
                    rme,
                    row.rme.x + row.rme.w - 14.0,
                    card.y + card.h / 2.0 + ink.center_up(),
                    RME_SIZE,
                    theme.rme,
                    Anchor::End,
                );
            }
        }
        body.push_str("</g>");
    }

    let footer_baseline = plan.footer.y + font.ascent_px(FOOTER_SIZE);
    push_text(
        &mut body,
        &atlas,
        font,
        input.footer,
        plan.footer.x,
        footer_baseline,
        FOOTER_SIZE,
        theme.footer,
        Anchor::Start,
    );

    // Emoji need a real font, so this single run stays as text. It sits on the
    // footer line so it can never collide with the pastel stars.
    if !input.kaomoji.is_empty() {
        body.push_str(&format!(
            "<text x=\"{x:.1}\" y=\"{y:.1}\" text-anchor=\"end\" \
             font-family=\"'Segoe UI Emoji','Apple Color Emoji','Noto Color Emoji',sans-serif\" \
             font-size=\"16\" fill=\"{fill}\">{emoji}</text>",
            x = plan.width - plan.padding,
            y = footer_baseline,
            fill = theme.kaomoji,
            emoji = escape(input.kaomoji),
        ));
    }

    let mut svg = String::with_capacity(atlas.paths.len() * 200 + body.len() + 2048);
    svg.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" \
         viewBox=\"0 0 {w:.0} {h:.0}\" role=\"img\" aria-label=\"{label}\">\
         <title>{label}</title><desc>{desc}</desc>",
        w = plan.width,
        h = plan.height,
        label = escape(input.title),
        desc = escape(input.subtitle),
    ));
    svg.push_str(&atlas.defs());
    svg.push_str(&format!(
        "<linearGradient id=\"bg\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\">\
         <stop offset=\"0\" stop-color=\"{}\"/><stop offset=\"0.55\" stop-color=\"{}\"/>\
         <stop offset=\"1\" stop-color=\"{}\"/></linearGradient>",
        theme.bg[0], theme.bg[1], theme.bg[2]
    ));
    svg.push_str(&format!(
        "<filter id=\"shadow\" x=\"-20%\" y=\"-20%\" width=\"140%\" height=\"140%\">\
         <feDropShadow dx=\"0\" dy=\"2\" stdDeviation=\"3\" flood-color=\"{}\" flood-opacity=\"{}\"/>\
         </filter></defs>",
        theme.shadow, theme.shadow_opacity
    ));
    svg.push_str(&format!(
        "<rect width=\"{w:.0}\" height=\"{h:.0}\" rx=\"24\" fill=\"url(#bg)\"/>",
        w = plan.width,
        h = plan.height
    ));
    svg.push_str(&decorations(plan, theme));
    svg.push_str(&body);
    svg.push_str("</svg>\n");
    RenderOutput { svg, warnings }
}

/// Pastel stars and dots, positioned relative to the card size.
fn decorations(plan: &LayoutPlan, theme: &Theme) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "<g filter=\"url(#shadow)\"><rect width=\"{w:.0}\" height=\"{h:.0}\" rx=\"24\" fill=\"transparent\"/></g>",
        w = plan.width,
        h = plan.height
    ));
    out.push_str(&format!(
        "<path d=\"M 830 34 l 4 8 9 1 -7 6 2 9 -8 -5 -8 5 2 -9 -7 -6 9 -1 z\" fill=\"{}\" fill-opacity=\"0.9\"/>",
        theme.star
    ));
    out.push_str(&format!(
        "<path d=\"M 872 62 l 3 6 7 1 -5 5 1 7 -6 -4 -6 4 1 -7 -5 -5 7 -1 z\" fill=\"{}\" fill-opacity=\"0.9\"/>",
        theme.star
    ));
    // Legend rows and section headings occupy the band the sparkles use, so
    // drop the decoration when either is present.
    let count = if plan.legend.is_empty() && plan.sections.is_empty() {
        ((plan.width - 140.0) / 118.0).floor().max(0.0) as usize
    } else {
        0
    };
    for index in 0..count {
        let cx = 70.0 + index as f32 * 118.0;
        let cy = 126.0 + if index % 2 == 0 { 0.0 } else { 8.0 };
        let r = if index % 2 == 0 { 3.0 } else { 2.0 };
        out.push_str(&format!(
            "<circle cx=\"{cx:.0}\" cy=\"{cy:.0}\" r=\"{r:.0}\" fill=\"{}\" fill-opacity=\"0.55\"/>",
            theme.sparkle
        ));
    }
    out
}

/// The row number sits on a solid cat in profile: body, head, ears, tail and two
/// legs. The body stretches with the digit count; everything else is fixed.
/// The row number sits on a cat: body, head with merged ears, tail and one paw.
/// Everything is drawn in a 30px-high design space and scaled to the badge, so
/// the same shape works for a 33px badge and a 59px one.
pub fn number_cat(svg: &mut String, pill: crate::layout::Rect, color: &str) {
    let scale = pill.h / 30.0;
    let w = pill.w / scale;
    let bx = (w - 21.0).max(14.0);
    svg.push_str(&format!(
        "<g transform=\"translate({x:.2} {y:.2}) scale({scale:.4})\">",
        x = pill.x,
        y = pill.y
    ));
    // Tail.
    svg.push_str(&format!(
        "<path d=\"M 6 24 C 2 24 0 19 3 16\" fill=\"none\" stroke=\"{color}\" \
         stroke-width=\"2.8\" stroke-linecap=\"round\"/>"
    ));
    // Body: flat back, a small dip, then a neck that climbs into the head.
    svg.push_str(&format!(
        "<path d=\"M 6 27 C 2 27 1 22 1 18 C 1 12 7 10 14 10 L {bx:.2} 10 \
         C {c1:.2} 10 {c2:.2} 8.6 {e:.2} 8 L {r:.2} 20 L {r:.2} 27 Z\" fill=\"{color}\"/>",
        c1 = bx + (w - 10.0 - bx) * 0.45,
        c2 = w - 13.0,
        e = w - 10.0,
        r = w - 4.0,
    ));
    // Head with merged ears, drawn at 0.9 of the reference outline.
    svg.push_str(&format!(
        "<g transform=\"translate({hx:.2} 10) scale(0.9) translate(-35 -10)\">\
         <path d=\"M 29 5.5 L 28 -0.5 L 34 3.5 L 40.5 0 L 41 5.5 C 43 8.5 43 15 38.5 16.5 \
         C 34 18 30 17 28.5 12 C 27.5 8.5 28 6.5 29 5.5 Z\" fill=\"{color}\"/></g>",
        hx = w - 9.0
    ));
    svg.push_str("</g>");
}

#[allow(clippy::too_many_arguments)]
fn push_text(
    body: &mut String,
    atlas: &Atlas,
    font: &Font,
    text: &str,
    x: f32,
    baseline: f32,
    size: f32,
    fill: &str,
    anchor: Anchor,
) {
    if text.is_empty() {
        return;
    }
    let width = font.text_width(text, size);
    let start = match anchor {
        Anchor::Start => x,
        Anchor::Middle => x - width / 2.0,
        Anchor::End => x - width,
    };
    let scale = size / font.units_per_em;
    body.push_str(&format!(
        "<g transform=\"translate({x:.2} {y:.2}) scale({s:.5} -{s:.5})\" fill=\"{fill}\">",
        x = start,
        y = baseline,
        s = scale,
    ));
    let mut cursor: f32 = 0.0;
    for ch in text.chars() {
        match font.glyph(ch) {
            Some(glyph) => {
                if let Some(id) = atlas.index.get(&glyph.gid) {
                    body.push_str("<use href=\"#g");
                    body.push_str(&id.to_string());
                    body.push_str("\" x=\"");
                    body.push_str(&format!("{cursor:.1}"));
                    body.push_str("\"/>");
                }
                cursor += glyph.advance;
            }
            None => cursor += font.advance_units(ch) + font.units_per_em * 0.5,
        }
    }
    body.push_str("</g>");
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}
