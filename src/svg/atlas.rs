//! Glyph atlas and text emission.

use hashbrown::HashMap;

use crate::hash::BuildIdentity;

use crate::font::Font;

use super::writer::Writer;

pub(crate) struct Atlas {
    /// `gid -> path id`; `u32::MAX` means the glyph is not in this atlas.
    index: Vec<u32>,
    pub(crate) paths: Vec<String>,
}

impl Atlas {
    pub(crate) fn build(font: &Font, texts: &[(&str, f32)]) -> (Self, Vec<String>) {
        let mut index: HashMap<u16, usize, BuildIdentity> = HashMap::default();
        let mut gids: Vec<u16> = Vec::new();
        let mut missing: Vec<char> = Vec::new();
        for (text, _) in texts {
            for ch in text.chars() {
                match font.glyph(ch) {
                    Some(glyph) => {
                        if let hashbrown::hash_map::Entry::Vacant(slot) = index.entry(glyph.gid) {
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
        let mut by_gid = vec![u32::MAX; 65_536];
        for (slot, gid) in gids.iter().enumerate() {
            by_gid[*gid as usize] = slot as u32;
        }
        let warnings = if missing.is_empty() {
            Vec::new()
        } else {
            let chars: String = missing.iter().collect();
            vec![format!("font {:?} has no glyph for {chars:?}", font.family)]
        };
        (
            Self {
                index: by_gid,
                paths,
            },
            warnings,
        )
    }

    /// Emits `<defs>`, one `<path id="gN">` per glyph, without a closing tag:
    /// the caller adds gradients and filter definitions before `</defs>`.
    pub(crate) fn write_defs(&self, out: &mut Writer) {
        out.open("defs").close();
        for (id, path) in self.paths.iter().enumerate() {
            if path.is_empty() {
                continue;
            }
            out.open("path")
                .attr_id("id", "g", id)
                .attr_raw("d", path)
                .empty();
        }
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
    out: &mut Writer,
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
    // Width is only needed to shift the origin; most runs are left-aligned.
    let start = match anchor {
        Anchor::Start => x,
        Anchor::Middle => x - font.text_width(text, size) / 2.0,
        Anchor::End => x - font.text_width(text, size),
    };
    let scale = size / font.units_per_em;
    out.open("g")
        .attr_fmt(
            "transform",
            format_args!("translate({start:.2} {baseline:.2}) scale({scale:.5} -{scale:.5})"),
        )
        .attr_raw("fill", fill)
        .close();
    let mut cursor: f32 = 0.0;
    for ch in text.chars() {
        match font.glyph(ch) {
            Some(glyph) => {
                let id = atlas.index[glyph.gid as usize];
                if id != u32::MAX {
                    out.open("use")
                        .attr_id("href", "#g", id as usize)
                        .attr_num("x", cursor, 1)
                        .empty();
                }
                cursor += glyph.advance;
            }
            None => cursor += font.advance_units(ch) + font.units_per_em * 0.5,
        }
    }
    out.end("g");
}
