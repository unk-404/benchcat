//! Card emission: rows, bars, values, legend and decorations.

use crate::font::Font;
use crate::layout::{
    FOOTER_SIZE, LABEL_SIZE, LEGEND_SIZE, LayoutPlan, NOTE_SIZE, NUMBER_SIZE, RME_SIZE,
    SECTION_SIZE, SERIES_VALUE_SIZE, SUBTITLE_SIZE, TITLE_SIZE, VALUE_SIZE,
};

use super::atlas::{Anchor, Atlas, escape, push_text};
use super::badge::number_cat;
use super::theme::Theme;

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
            for value in row.row.values.iter().filter(|value| value.present) {
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

        for bar in &row.bars {
            let series = bar.series;
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
