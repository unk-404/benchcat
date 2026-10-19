//! Glyph atlas, text emission and escaping.

use std::collections::HashMap;

use crate::font::Font;

pub(crate) struct Atlas {
    index: HashMap<u16, usize>,
    pub(crate) paths: Vec<String>,
}

impl Atlas {
    pub(crate) fn build(font: &Font, texts: &[(&str, f32)]) -> (Self, Vec<String>) {
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

    pub(crate) fn defs(&self) -> String {
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_text(
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

pub(crate) fn escape(text: &str) -> String {
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
