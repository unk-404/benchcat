//! Glyph atlas and text emission.

use hashbrown::HashMap;

use crate::font::FontStack;
use crate::hash::BuildIdentity;

use super::writer::Writer;

pub(crate) struct Atlas {
    /// `(face index, gid) -> path id`; faces are stacked for fallback, so the
    /// same gid in two faces is not the same glyph.
    index: HashMap<(u16, u16), u32, BuildIdentity>,
    pub(crate) paths: Vec<String>,
}

impl Atlas {
    pub(crate) fn build(stack: &FontStack, texts: &[(&str, f32)]) -> (Self, Vec<String>) {
        let mut index: HashMap<(u16, u16), u32, BuildIdentity> = HashMap::default();
        let mut unique: Vec<(usize, u16)> = Vec::new();
        let mut missing: Vec<char> = Vec::new();
        for (text, _) in texts {
            for ch in text.chars() {
                match stack.glyph(ch) {
                    Some((face, glyph)) => {
                        let key = (face as u16, glyph.gid);
                        if let hashbrown::hash_map::Entry::Vacant(slot) = index.entry(key) {
                            slot.insert(unique.len() as u32);
                            unique.push((face, glyph.gid));
                        }
                    }
                    None => missing.push(ch),
                }
            }
        }
        missing.sort_unstable();
        missing.dedup();

        let mut paths = vec![String::new(); unique.len()];
        for (face, font) in stack.fonts().iter().enumerate() {
            let slots: Vec<usize> = unique
                .iter()
                .enumerate()
                .filter(|(_, (owner, _))| *owner == face)
                .map(|(slot, _)| slot)
                .collect();
            if slots.is_empty() {
                continue;
            }
            let gids: Vec<u16> = slots.iter().map(|slot| unique[*slot].1).collect();
            let extracted = font.extract_paths(&gids);
            for (slot, path) in slots.into_iter().zip(extracted) {
                paths[slot] = path;
            }
        }

        let warnings = if missing.is_empty() {
            Vec::new()
        } else {
            let chars: String = missing.iter().collect();
            vec![format!(
                "font {:?} has no glyph for {chars:?}",
                stack.family()
            )]
        };
        (Self { index, paths }, warnings)
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
    stack: &FontStack,
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
    let start = match anchor {
        Anchor::Start => x,
        Anchor::Middle => x - stack.text_width(text, size) / 2.0,
        Anchor::End => x - stack.text_width(text, size),
    };

    // Runs drawn entirely by the primary face keep the exact old markup: one
    // translate+scale group for the whole run. Only runs that actually use a
    // fallback face switch scale per glyph.
    let all_primary = text
        .chars()
        .all(|ch| matches!(stack.glyph(ch), Some((0, _))));
    if stack.len() == 1 || all_primary {
        let font = stack.font(0);
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
            match stack.glyph(ch) {
                Some((_, glyph)) => {
                    if let Some(id) = atlas.index.get(&(0, glyph.gid)) {
                        out.open("use")
                            .attr_id("href", "#g", *id as usize)
                            .attr_num("x", cursor, 1)
                            .empty();
                    }
                    cursor += glyph.advance;
                }
                None => cursor += font.units_per_em * 0.5,
            }
        }
        out.end("g");
        return;
    }

    out.open("g")
        .attr_fmt(
            "transform",
            format_args!("translate({start:.2} {baseline:.2})"),
        )
        .attr_raw("fill", fill)
        .close();
    let mut cursor: f32 = 0.0;
    for ch in text.chars() {
        let Some((face, glyph)) = stack.glyph(ch) else {
            cursor += size * 0.5;
            continue;
        };
        let font = stack.font(face);
        let scale = size / font.units_per_em;
        if let Some(id) = atlas.index.get(&(face as u16, glyph.gid)) {
            out.open("g")
                .attr_fmt(
                    "transform",
                    format_args!("translate({cursor:.2} 0) scale({scale:.5} -{scale:.5})"),
                )
                .close();
            out.open("use").attr_id("href", "#g", *id as usize).empty();
            out.end("g");
        }
        cursor += glyph.advance * scale;
    }
    out.end("g");
}
