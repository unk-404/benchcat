//! Card emission: rows, bars, values, legend and decorations.

use crate::font::Font;
use crate::layout::{
    FOOTER_SIZE, LABEL_SIZE, LEGEND_SIZE, LayoutPlan, NOTE_SIZE, NUMBER_SIZE, RME_SIZE,
    SECTION_SIZE, SERIES_VALUE_SIZE, SUBTITLE_SIZE, TITLE_SIZE, VALUE_SIZE,
};

use super::atlas::{Anchor, Atlas, push_text};
use super::badge::draw_cat;
use super::theme::Theme;
use super::writer::Writer;

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

    let mut out = Writer::with_capacity(plan.rows.len() * 3_200 + 65_536);
    out.raw("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.open("svg")
        .attr_raw("xmlns", "http://www.w3.org/2000/svg")
        .attr_num("width", plan.width, 0)
        .attr_num("height", plan.height, 0)
        .attr_fmt(
            "viewBox",
            format_args!("0 0 {:.0} {:.0}", plan.width, plan.height),
        )
        .attr_raw("role", "img")
        .attr("aria-label", input.title)
        .close();
    out.open("title").close().text(input.title).end("title");
    out.open("desc").close().text(input.subtitle).end("desc");

    atlas.write_defs(&mut out);
    out.open("linearGradient")
        .attr_raw("id", "bg")
        .attr_raw("x1", "0")
        .attr_raw("y1", "0")
        .attr_raw("x2", "1")
        .attr_raw("y2", "1")
        .close();
    for (offset, color) in [
        ("0", theme.bg[0]),
        ("0.55", theme.bg[1]),
        ("1", theme.bg[2]),
    ] {
        out.open("stop")
            .attr_raw("offset", offset)
            .attr_raw("stop-color", color)
            .empty();
    }
    out.end("linearGradient");
    out.open("filter")
        .attr_raw("id", "shadow")
        .attr_raw("x", "-20%")
        .attr_raw("y", "-20%")
        .attr_raw("width", "140%")
        .attr_raw("height", "140%")
        .close();
    out.open("feDropShadow")
        .attr_raw("dx", "0")
        .attr_raw("dy", "2")
        .attr_raw("stdDeviation", "3")
        .attr_raw("flood-color", theme.shadow)
        .attr_fmt("flood-opacity", format_args!("{}", theme.shadow_opacity))
        .empty();
    out.end("filter");
    out.end("defs");

    out.open("rect")
        .attr_num("width", plan.width, 0)
        .attr_num("height", plan.height, 0)
        .attr_raw("rx", "24")
        .attr_raw("fill", "url(#bg)")
        .empty();
    decorations(&mut out, plan, theme);

    let title_baseline = plan.title.y + font.ascent_px(TITLE_SIZE);
    push_text(
        &mut out,
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
        &mut out,
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
        out.open("circle")
            .attr_num("cx", item.dot.0, 1)
            .attr_num("cy", item.dot.1, 1)
            .attr_raw("r", "4")
            .attr_raw("fill", &item.color)
            .empty();
        push_text(
            &mut out,
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
            &mut out,
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
        out.open("g").close();
        out.open("title")
            .close()
            .text(&row.row.tooltip)
            .end("title");
        out.open("rect")
            .attr_num("x", card.x, 1)
            .attr_num("y", card.y, 1)
            .attr_num("width", card.w, 1)
            .attr_num("height", card.h, 1)
            .attr_raw("rx", "12")
            .attr_raw("fill", theme.card)
            .attr_fmt("fill-opacity", format_args!("{}", theme.card_opacity))
            .empty();

        let accent = theme.dot_colors[index % theme.dot_colors.len()];
        draw_cat(&mut out, row.number_pill, accent);
        // Centre on the glyph ink, not on the em box, so every index sits
        // optically centred however many digits it has.
        let number_ink = font.text_ink(&numbers[index], NUMBER_SIZE);
        push_text(
            &mut out,
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
            &mut out,
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
                &mut out,
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
            out.open("rect")
                .attr_num("x", bar.track.x, 1)
                .attr_num("y", bar.track.y, 1)
                .attr_num("width", bar.track.w, 1)
                .attr_num("height", bar.track.h, 1)
                .attr_raw("rx", "3")
                .attr_raw("fill", theme.track)
                .empty();
            out.open("rect")
                .attr_num("x", bar.fill.x, 1)
                .attr_num("y", bar.fill.y, 1)
                .attr_num("width", bar.fill.w, 1)
                .attr_num("height", bar.fill.h, 1)
                .attr_raw("rx", "3")
                .attr_raw("fill", color)
                .empty();
            if plan.multi_series
                && let Some(value) = row.row.values.get(series)
            {
                let baseline =
                    bar.track.y + bar.track.h / 2.0 + font.ascent_px(SERIES_VALUE_SIZE) / 2.0 - 1.5;
                push_text(
                    &mut out,
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
                    &mut out,
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
                    &mut out,
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
        out.end("g");
    }

    let footer_baseline = plan.footer.y + font.ascent_px(FOOTER_SIZE);
    push_text(
        &mut out,
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
        out.open("text")
            .attr_num("x", plan.width - plan.padding, 1)
            .attr_num("y", footer_baseline, 1)
            .attr_raw("text-anchor", "end")
            .attr_raw(
                "font-family",
                "'Segoe UI Emoji','Apple Color Emoji','Noto Color Emoji',sans-serif",
            )
            .attr_raw("font-size", "16")
            .attr_raw("fill", theme.kaomoji)
            .close()
            .text(input.kaomoji)
            .end("text");
    }

    out.raw("</svg>\n");
    RenderOutput {
        svg: out.finish(),
        warnings,
    }
}

/// Pastel stars and dots, positioned relative to the card size.
fn decorations(out: &mut Writer, plan: &LayoutPlan, theme: &Theme) {
    out.open("g").attr_raw("filter", "url(#shadow)").close();
    out.open("rect")
        .attr_num("width", plan.width, 0)
        .attr_num("height", plan.height, 0)
        .attr_raw("rx", "24")
        .attr_raw("fill", "transparent")
        .empty();
    out.end("g");
    out.open("path")
        .attr_raw(
            "d",
            "M 830 34 l 4 8 9 1 -7 6 2 9 -8 -5 -8 5 2 -9 -7 -6 9 -1 z",
        )
        .attr_raw("fill", theme.star)
        .attr_raw("fill-opacity", "0.9")
        .empty();
    out.open("path")
        .attr_raw(
            "d",
            "M 872 62 l 3 6 7 1 -5 5 1 7 -6 -4 -6 4 1 -7 -5 -5 7 -1 z",
        )
        .attr_raw("fill", theme.star)
        .attr_raw("fill-opacity", "0.9")
        .empty();
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
        out.open("circle")
            .attr_num("cx", cx, 0)
            .attr_num("cy", cy, 0)
            .attr_num("r", r, 0)
            .attr_raw("fill", theme.sparkle)
            .attr_raw("fill-opacity", "0.55")
            .empty();
    }
}
