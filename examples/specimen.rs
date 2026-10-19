//! Type specimen cards: the same pastel card language as the benchmark chart,
//! but filled with lettering instead of measurements.
//!
//! Usage:
//!   cargo run --example specimen              # fonts/*.ttf -> assets/fonts/*.svg
//!   cargo run --example specimen -- --png     # plus previews in target/previews
//!
//! Missing glyphs are never hidden: each one becomes a pastel cat, sized by a
//! style chosen from the ink of the glyphs that row *can* render. That keeps the
//! cats from crowding dense CJK sample lines.

use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};

use bench_svg::Font;
use bench_svg::svg::PASTEL;

const OUT_DIR: &str = "assets/fonts";
const PREVIEW_DIR: &str = "target/previews";
const WIDTH: f32 = 920.0;
const PADDING: f32 = 22.0;
const MARGIN: f32 = PADDING + 14.0;
const ROW_HEIGHT: f32 = 54.0;
const ROW_GAP: f32 = 6.0;
const HEADER_HEIGHT: f32 = 144.0;

const LABEL_SIZE: f32 = 9.5;
const SAMPLE_SIZE: f32 = 16.0;

const CAT_FILL: &str = "#FFD6E4";
const CAT_STROKE: &str = "#E7A2BC";
const WARN_FILL: &str = "#FFF5F9";

/// Samples ordered from basic Latin coverage to script-specific blocks. The
/// optional third field names the script in English, so a row stays readable
/// even when none of its own characters can be drawn.
const SAMPLES: [(&str, Option<&str>, &str); 9] = [
    ("Uppercase", None, "ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
    ("Lowercase", None, "abcdefghijklmnopqrstuvwxyz"),
    ("Digits", None, "0123456789 1.24 82.4k ±1.21%"),
    ("Punctuation", None, r##"!"#$%&'()*+,-./:;<=>?@[\]^_`{|}~"##),
    (
        "Pangram",
        None,
        "The quick brown fox jumps over the lazy dog",
    ),
    ("Symbols", None, "± × · → ← ↑ ↓ ✓ ✗ № § ¶ ¥ € £"),
    (
        "日本語",
        Some("Japanese"),
        "順次読み込み・ランダム書き込み（1 GiB）",
    ),
    ("中文", Some("Chinese"), "顺序读写与随机读取（1 GiB）"),
    ("한국어", Some("Korean"), "순차 읽기 및 임의 쓰기"),
];

fn main() -> Result<(), Box<dyn Error>> {
    let with_png = std::env::args().any(|arg| arg == "--png");
    std::fs::create_dir_all(OUT_DIR)?;
    if with_png {
        std::fs::create_dir_all(PREVIEW_DIR)?;
    }

    let mut fonts: Vec<PathBuf> = std::fs::read_dir("fonts")?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension().is_some_and(|extension| {
                extension.eq_ignore_ascii_case("ttf") || extension.eq_ignore_ascii_case("otf")
            })
        })
        .collect();
    fonts.sort();
    if fonts.is_empty() {
        return Err("no fonts found in ./fonts".into());
    }

    for path in fonts {
        let font = Font::load(&path)?;
        let svg = specimen(&font);
        let stem = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "font".to_owned());
        let out = Path::new(OUT_DIR).join(format!("{stem}.svg"));
        std::fs::write(&out, &svg)?;
        println!("{:<40} {:>8} bytes", out.display(), svg.len());
        if with_png {
            let png = Path::new(PREVIEW_DIR).join(format!("font-{stem}.png"));
            rasterise(&svg, &png, 2.0)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Cat placeholders
// ---------------------------------------------------------------------------

/// Reference box for a missing glyph, in em units, taken from the font's own
/// ideographic ink, so a cat sits exactly on the baseline of the characters
/// around it.
struct InkRef {
    top: f32,
    bottom: f32,
    width: f32,
}

impl InkRef {
    fn for_font(font: &Font) -> Self {
        for probe in ['序', '中', '日', '国', '漢'] {
            if !font.has_glyph(probe) {
                continue;
            }
            let ink = font.text_ink(&probe.to_string(), 100.0);
            if ink.top > ink.bottom {
                return Self {
                    top: ink.top / 100.0,
                    bottom: ink.bottom / 100.0,
                    width: (ink.right - ink.left) / 100.0,
                };
            }
        }
        Self {
            top: 0.86,
            bottom: -0.02,
            width: 0.88,
        }
    }
}

enum Face {
    /// Two dot eyes, triangle nose, `w` mouth.
    Happy,
    /// Closed eyes, no mouth.
    Sleepy,
    /// One dot eye, one closed eye.
    Wink,
}

/// One cat style. The size is a property of the style, not of the instance:
/// a row that needs smaller cats asks for a different style.
struct CatStyle {
    scale: f32,
    face: Face,
}

const CAT_STYLES: [CatStyle; 3] = [
    CatStyle {
        scale: 0.98,
        face: Face::Happy,
    },
    CatStyle {
        scale: 0.88,
        face: Face::Sleepy,
    },
    CatStyle {
        scale: 0.78,
        face: Face::Wink,
    },
];

/// Pick a cat style from the ink of the glyphs this row can actually draw.
/// Dense scripts (CJK) get the smallest cat so it cannot crowd the characters.
fn cat_style_for(font: &Font, text: &str) -> usize {
    let mut widths: Vec<f32> = text
        .chars()
        .filter(|ch| !ch.is_whitespace() && font.has_glyph(*ch))
        .take(24)
        .map(|ch| {
            let ink = font.text_ink(&ch.to_string(), SAMPLE_SIZE);
            (ink.right - ink.left) / SAMPLE_SIZE
        })
        .filter(|width| *width > 0.0)
        .collect();
    if widths.is_empty() {
        return 0;
    }
    widths.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = widths[widths.len() / 2];
    if median >= 0.72 {
        2
    } else if median >= 0.48 {
        1
    } else {
        0
    }
}

/// Draw one cat inside the reference ink box, scaled by the style.
fn draw_cat(svg: &mut String, x: f32, baseline: f32, size: f32, ink: &InkRef, style: &CatStyle) {
    let w = ink.width * size * style.scale;
    let h = (ink.top - ink.bottom) * size * style.scale;
    let top = baseline - h;
    let head_y = top + h * 0.22;
    let head_h = h * 0.78;
    let rx = head_h * 0.42;
    let x = x + (size * 0.95 - w) / 2.0;

    // Ears first so the head overlaps their base.
    for (a, b, c, d, e, f) in [
        (
            0.10,
            head_y + head_h * 0.44,
            0.20,
            top,
            0.46,
            head_y + head_h * 0.28,
        ),
        (
            0.90,
            head_y + head_h * 0.44,
            0.80,
            top,
            0.54,
            head_y + head_h * 0.28,
        ),
    ] {
        svg.push_str(&format!(
            "<path d=\"M {x1:.1} {y1:.1} L {x2:.1} {y2:.1} L {x3:.1} {y3:.1} Z\" \
             fill=\"{CAT_FILL}\" stroke=\"{CAT_STROKE}\" stroke-width=\"0.7\" \
             stroke-linejoin=\"round\"/>",
            x1 = x + w * a,
            y1 = b,
            x2 = x + w * c,
            y2 = d,
            x3 = x + w * e,
            y3 = f,
        ));
    }
    svg.push_str(&format!(
        "<rect x=\"{x:.1}\" y=\"{head_y:.1}\" width=\"{w:.1}\" height=\"{head_h:.1}\" \
         rx=\"{rx:.1}\" fill=\"{CAT_FILL}\" stroke=\"{CAT_STROKE}\" stroke-width=\"0.7\"/>"
    ));

    let eye_y = head_y + head_h * 0.40;
    let eye_r = size * 0.042;
    let left_eye = x + w * 0.33;
    let right_eye = x + w * 0.67;
    match style.face {
        Face::Happy => {
            svg.push_str(&format!(
                "<circle cx=\"{left_eye:.1}\" cy=\"{eye_y:.1}\" r=\"{eye_r:.1}\" fill=\"{CAT_STROKE}\"/>\
                 <circle cx=\"{right_eye:.1}\" cy=\"{eye_y:.1}\" r=\"{eye_r:.1}\" fill=\"{CAT_STROKE}\"/>"
            ));
            svg.push_str(&mouth(x, w, head_y, head_h, size));
        }
        Face::Sleepy => {
            svg.push_str(&closed_eye(left_eye, eye_y, w));
            svg.push_str(&closed_eye(right_eye, eye_y, w));
            svg.push_str(&nose(x, w, head_y, head_h, size));
        }
        Face::Wink => {
            svg.push_str(&format!(
                "<circle cx=\"{left_eye:.1}\" cy=\"{eye_y:.1}\" r=\"{eye_r:.1}\" fill=\"{CAT_STROKE}\"/>"
            ));
            svg.push_str(&closed_eye(right_eye, eye_y, w));
            svg.push_str(&mouth(x, w, head_y, head_h, size));
        }
    }
}

fn closed_eye(cx: f32, cy: f32, w: f32) -> String {
    format!(
        "<path d=\"M {:.1} {cy:.1} q {:.1} {:.1} {:.1} 0\" fill=\"none\" stroke=\"{CAT_STROKE}\" \
         stroke-width=\"0.9\" stroke-linecap=\"round\"/>",
        cx - w * 0.09,
        w * 0.09,
        -w * 0.06,
        w * 0.18,
    )
}

fn nose(x: f32, w: f32, head_y: f32, head_h: f32, size: f32) -> String {
    let nose_y = head_y + head_h * 0.56;
    format!(
        "<path d=\"M {a:.1} {b:.1} L {c:.1} {b:.1} L {d:.1} {e:.1} Z\" fill=\"{CAT_STROKE}\"/>",
        a = x + w * 0.45,
        b = nose_y,
        c = x + w * 0.55,
        d = x + w * 0.5,
        e = nose_y + size * 0.05,
    )
}

fn mouth(x: f32, w: f32, head_y: f32, head_h: f32, size: f32) -> String {
    format!(
        "<path d=\"M {a:.1} {b:.1} q {c:.1} {d:.1} {e:.1} 0 q {f:.1} {g:.1} {h:.1} 0\" \
         fill=\"none\" stroke=\"{CAT_STROKE}\" stroke-width=\"0.7\" stroke-linecap=\"round\"/>",
        a = x + w * 0.38,
        b = head_y + head_h * 0.70,
        c = w * 0.06,
        d = size * 0.05,
        e = w * 0.12,
        f = w * 0.06,
        g = -size * 0.05,
        h = w * 0.12,
    )
}

// ---------------------------------------------------------------------------
// Specimen
// ---------------------------------------------------------------------------

struct Row {
    label: &'static str,
    script: Option<&'static str>,
    text: &'static str,
    missing: usize,
    cat: usize,
}

impl Row {
    fn coverage(&self) -> f32 {
        let total = self.text.chars().filter(|ch| !ch.is_whitespace()).count();
        if total == 0 {
            1.0
        } else {
            1.0 - self.missing as f32 / total as f32
        }
    }
}

fn rows(font: &Font) -> Vec<Row> {
    SAMPLES
        .iter()
        .map(|(label, script, text)| {
            let missing = text
                .chars()
                .filter(|ch| !ch.is_whitespace() && !font.has_glyph(*ch))
                .count();
            Row {
                label,
                script: *script,
                text,
                missing,
                cat: cat_style_for(font, text),
            }
        })
        .collect()
}

fn specimen(font: &Font) -> String {
    let rows = rows(font);
    let ink = InkRef::for_font(font);
    let hero = "AaBbCc 0123";
    let title = font.family.clone();
    let subtitle = format!(
        "{} glyphs · {} codepoints mapped · upem {} · ascent {:.0} · descent {:.0}",
        font.glyph_count,
        font.char_count(),
        font.units_per_em,
        font.ascent,
        font.descent,
    );
    let footer = "Type specimen · missing glyphs become pastel cats · bench-svg";

    let rows_height = rows.len() as f32 * (ROW_HEIGHT + ROW_GAP);
    let height = HEADER_HEIGHT + rows_height + 38.0;

    // One atlas for the whole card: each glyph is written once and reused.
    let mut texts: Vec<&str> = vec![title.as_str(), subtitle.as_str(), hero, footer];
    for row in &rows {
        texts.push(row.label);
        texts.push(row.text);
        if let Some(script) = row.script {
            texts.push(script);
        }
    }
    let mut order: Vec<u16> = Vec::new();
    let mut index: HashMap<u16, usize> = HashMap::new();
    for value in &texts {
        for ch in value.chars() {
            if let Some(glyph) = font.glyph(ch)
                && let std::collections::hash_map::Entry::Vacant(slot) = index.entry(glyph.gid)
            {
                slot.insert(order.len());
                order.push(glyph.gid);
            }
        }
    }
    let paths = font.extract_paths(&order);

    let mut svg = String::with_capacity(64 * 1024);
    svg.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{WIDTH:.0}\" height=\"{height:.0}\" \
         viewBox=\"0 0 {WIDTH:.0} {height:.0}\" role=\"img\" aria-label=\"{label}\">\
         <title>{label}</title><desc>{subtitle}</desc>",
        label = escape(&title),
        subtitle = escape(&subtitle),
    ));
    svg.push_str("<defs>");
    for (id, data) in paths.iter().enumerate() {
        if data.is_empty() {
            continue;
        }
        svg.push_str(&format!("<path id=\"s{id}\" d=\"{data}\"/>"));
    }
    svg.push_str(&format!(
        "<linearGradient id=\"bg\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\">\
         <stop offset=\"0\" stop-color=\"{}\"/><stop offset=\"0.55\" stop-color=\"{}\"/>\
         <stop offset=\"1\" stop-color=\"{}\"/></linearGradient>",
        PASTEL.bg[0], PASTEL.bg[1], PASTEL.bg[2]
    ));
    svg.push_str(&format!(
        "<filter id=\"shadow\" x=\"-20%\" y=\"-20%\" width=\"140%\" height=\"140%\">\
         <feDropShadow dx=\"0\" dy=\"2\" stdDeviation=\"3\" flood-color=\"{}\" flood-opacity=\"{}\"/>\
         </filter></defs>",
        PASTEL.shadow, PASTEL.shadow_opacity
    ));
    svg.push_str(&format!(
        "<rect width=\"{WIDTH:.0}\" height=\"{height:.0}\" rx=\"24\" fill=\"url(#bg)\"/>"
    ));
    svg.push_str(&format!(
        "<path d=\"M 830 34 l 4 8 9 1 -7 6 2 9 -8 -5 -8 5 2 -9 -7 -6 9 -1 z\" fill=\"{}\" fill-opacity=\"0.9\"/>",
        PASTEL.star
    ));
    svg.push_str(&format!(
        "<path d=\"M 872 62 l 3 6 7 1 -5 5 1 7 -6 -4 -6 4 1 -7 -5 -5 7 -1 z\" fill=\"{}\" fill-opacity=\"0.9\" transform=\"translate(0 -16)\"/>",
        PASTEL.star
    ));

    let title_baseline = PADDING + font.ascent_px(26.0);
    text_run(
        &mut svg,
        font,
        &paths,
        &index,
        &title,
        MARGIN,
        title_baseline,
        26.0,
        PASTEL.title,
        Align::Start,
        None,
    );
    text_run(
        &mut svg,
        font,
        &paths,
        &index,
        &subtitle,
        MARGIN,
        title_baseline + font.line_height_px(12.0) + 5.0,
        12.0,
        PASTEL.subtitle,
        Align::Start,
        None,
    );

    // Hero sample on a soft card of its own.
    let (hero_top, hero_height) = (80.0, 46.0);
    svg.push_str(&format!(
        "<rect x=\"{x:.1}\" y=\"{hero_top:.1}\" width=\"{w:.1}\" height=\"{hero_height:.1}\" \
         rx=\"14\" fill=\"{}\" fill-opacity=\"0.55\" filter=\"url(#shadow)\"/>",
        PASTEL.card,
        x = PADDING,
        w = WIDTH - PADDING * 2.0,
    ));
    text_run(
        &mut svg,
        font,
        &paths,
        &index,
        hero,
        MARGIN,
        hero_top + hero_height / 2.0 + font.text_ink(hero, 30.0).center_up(),
        30.0,
        PASTEL.title,
        Align::Start,
        None,
    );

    // Label and sample form one block, centred in the row so the two lines sit
    // closer together and both keep a comfortable margin from the card edges.
    let label_line = font.line_height_px(LABEL_SIZE);
    let sample_line = font.line_height_px(SAMPLE_SIZE);
    let block_gap = 2.0;

    for (row_index, row) in rows.iter().enumerate() {
        let y = HEADER_HEIGHT + row_index as f32 * (ROW_HEIGHT + ROW_GAP);
        let empty = row.coverage() <= 0.0;
        if empty {
            svg.push_str(&format!(
                "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"12\" \
                 fill=\"{WARN_FILL}\" stroke=\"{CAT_STROKE}\" stroke-width=\"1\" stroke-dasharray=\"4 3\"/>",
                x = PADDING,
                y = y,
                w = WIDTH - PADDING * 2.0,
                h = ROW_HEIGHT,
            ));
        } else {
            svg.push_str(&format!(
                "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"12\" \
                 fill=\"{}\" fill-opacity=\"{}\"/>",
                PASTEL.card,
                PASTEL.card_opacity,
                x = PADDING,
                y = y,
                w = WIDTH - PADDING * 2.0,
                h = ROW_HEIGHT,
            ));
        }

        let block_top = y + (ROW_HEIGHT - (label_line + block_gap + sample_line)) / 2.0;
        let label_baseline = block_top + font.ascent_px(LABEL_SIZE);
        let sample_baseline = block_top + label_line + block_gap + font.ascent_px(SAMPLE_SIZE);

        // If the native label cannot be drawn at all, lead with the English
        // script name instead of leaving a bare "· N missing".
        let native_label = row
            .label
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .any(|ch| font.has_glyph(ch));
        let base = if native_label {
            row.label
        } else {
            row.script.unwrap_or(row.label)
        };
        let label = match (empty, row.missing) {
            (true, _) => format!("{base} · no glyphs ( =^.^= )"),
            (false, 0) => base.to_owned(),
            (false, missing) => format!("{base} · {missing} missing"),
        };
        text_run(
            &mut svg,
            font,
            &paths,
            &index,
            &label,
            MARGIN,
            label_baseline,
            LABEL_SIZE,
            if empty { CAT_STROKE } else { PASTEL.note },
            Align::Start,
            None,
        );
        if let Some(script) = row.script
            && native_label
        {
            text_run(
                &mut svg,
                font,
                &paths,
                &index,
                script,
                WIDTH - MARGIN,
                label_baseline,
                LABEL_SIZE,
                if empty { CAT_STROKE } else { PASTEL.note },
                Align::End,
                None,
            );
        }

        let sample = clamp_to_width(
            font,
            row.text,
            SAMPLE_SIZE,
            WIDTH - MARGIN * 2.0,
            Some(&ink),
        );
        text_run(
            &mut svg,
            font,
            &paths,
            &index,
            &sample,
            MARGIN,
            sample_baseline,
            SAMPLE_SIZE,
            if empty { CAT_STROKE } else { PASTEL.label },
            Align::Start,
            Some((&ink, &CAT_STYLES[row.cat])),
        );
    }

    text_run(
        &mut svg,
        font,
        &paths,
        &index,
        footer,
        MARGIN,
        height - PADDING - 2.0,
        10.5,
        PASTEL.footer,
        Align::Start,
        None,
    );
    svg.push_str("</svg>\n");
    svg
}

#[derive(Clone, Copy, PartialEq)]
enum Align {
    Start,
    End,
}

/// Truncate a sample that would overflow the card, adding an ellipsis.
fn clamp_to_width(
    font: &Font,
    text: &str,
    size: f32,
    max_width: f32,
    ink: Option<&InkRef>,
) -> String {
    let width_of = |ch: char| -> f32 {
        if font.has_glyph(ch) {
            font.advance(ch, size)
        } else {
            match ink {
                Some(_) => size * 0.95,
                None => font.advance(ch, size),
            }
        }
    };
    if text.chars().map(width_of).sum::<f32>() <= max_width {
        return text.to_owned();
    }
    let mut out = String::new();
    let mut width = 0.0f32;
    for ch in text.chars() {
        let advance = width_of(ch);
        if width + advance > max_width - size * 0.7 {
            break;
        }
        width += advance;
        out.push(ch);
    }
    out.push('…');
    out
}

/// Emit a run of glyphs, drawing a pastel cat for every missing glyph.
#[allow(clippy::too_many_arguments)]
fn text_run(
    svg: &mut String,
    font: &Font,
    paths: &[String],
    index: &HashMap<u16, usize>,
    value: &str,
    x: f32,
    baseline: f32,
    size: f32,
    fill: &str,
    align: Align,
    cats: Option<(&InkRef, &CatStyle)>,
) {
    if value.is_empty() {
        return;
    }
    // Align by ink, not by advance, so every line shares one optical edge.
    let metrics = font.text_ink(value, size);
    let scale = size / font.units_per_em;
    let mut page_x = match align {
        Align::Start => x - metrics.left,
        Align::End => x - metrics.right,
    };
    let mut run_open = false;
    let mut run_units = 0.0f32;

    for ch in value.chars() {
        match font.glyph(ch) {
            Some(glyph) => {
                let has_path = index
                    .get(&glyph.gid)
                    .is_some_and(|id| !paths[*id].is_empty());
                if has_path {
                    if !run_open {
                        svg.push_str(&format!(
                            "<g transform=\"translate({page_x:.2} {baseline:.2}) \
                             scale({scale:.5} -{scale:.5})\" fill=\"{fill}\">"
                        ));
                        run_open = true;
                        run_units = 0.0;
                    }
                    let id = index[&glyph.gid];
                    svg.push_str(&format!("<use href=\"#s{id}\" x=\"{run_units:.1}\"/>"));
                }
                run_units += glyph.advance;
                page_x += glyph.advance * scale;
            }
            None => {
                if run_open {
                    svg.push_str("</g>");
                    run_open = false;
                }
                match cats {
                    Some((ink, style)) => {
                        draw_cat(svg, page_x, baseline, size, ink, style);
                        page_x += size * 0.95;
                    }
                    None => page_x += size * 0.5,
                }
            }
        }
    }
    if run_open {
        svg.push_str("</g>");
    }
}

fn rasterise(svg: &str, path: &Path, scale: f32) -> Result<(), Box<dyn Error>> {
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_str(svg, &options)?;
    let size = tree.size();
    let width = (size.width() * scale).ceil() as u32;
    let height = (size.height() * scale).ceil() as u32;
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(width, height).ok_or("cannot allocate pixmap")?;
    pixmap.fill(resvg::tiny_skia::Color::WHITE);
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.save_png(path)?;
    Ok(())
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
