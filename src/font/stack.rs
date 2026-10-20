//! A primary face plus fixed fallback faces, selected per character.
//!
//! The fallback list is intentionally not configurable: `enable_guaranteed_font`
//! only turns it on. Repository faces come first, then a short list of system
//! faces; if nobody covers a character it is reported as missing exactly as
//! before.

use super::{Font, Glyph, Ink};

/// A stack of loaded faces. Index 0 is the primary face unless the caller
/// loaded a guaranteed face because the configured one was unavailable.
pub struct FontStack {
    fonts: Vec<Font>,
    primary: usize,
}

impl FontStack {
    pub fn new(fonts: Vec<Font>, primary: usize) -> Self {
        assert!(!fonts.is_empty(), "a font stack needs at least one face");
        assert!(primary < fonts.len(), "primary font index out of range");
        Self { fonts, primary }
    }

    pub fn single(font: Font) -> Self {
        Self {
            fonts: vec![font],
            primary: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }

    pub fn fonts(&self) -> &[Font] {
        &self.fonts
    }

    pub fn font(&self, index: usize) -> &Font {
        &self.fonts[index]
    }

    pub fn primary(&self) -> &Font {
        &self.fonts[self.primary]
    }

    /// Label for logs and the `{font}` placeholder.
    pub fn family(&self) -> &str {
        &self.primary().family
    }

    /// First face that can draw `ch`, with its index in the stack.
    pub fn glyph(&self, ch: char) -> Option<(usize, Glyph)> {
        self.fonts
            .iter()
            .enumerate()
            .find_map(|(index, font)| font.glyph(ch).map(|glyph| (index, glyph)))
    }

    pub fn has_glyph(&self, ch: char) -> bool {
        self.glyph(ch).is_some()
    }

    /// Advance width in pixels at `size`, using whichever face draws `ch`.
    pub fn advance(&self, ch: char, size: f32) -> f32 {
        match self.glyph(ch) {
            Some((index, glyph)) => glyph.advance * size / self.fonts[index].units_per_em,
            None => size * 0.5,
        }
    }

    pub fn advance_units(&self, ch: char) -> f32 {
        self.glyph(ch).map_or(0.0, |(_, glyph)| glyph.advance)
    }

    pub fn text_width(&self, text: &str, size: f32) -> f32 {
        text.chars().map(|ch| self.advance(ch, size)).sum()
    }

    /// Vertical metrics come from the primary face so the card keeps one
    /// baseline grid even when fallback faces have different metrics.
    pub fn ascent_px(&self, size: f32) -> f32 {
        self.primary().ascent * size / self.primary().units_per_em
    }

    pub fn line_height_px(&self, size: f32) -> f32 {
        let font = self.primary();
        let natural = (font.ascent - font.descent + font.leading) * size / font.units_per_em;
        natural.max(size * 1.15)
    }

    /// Ink box of `text`, with each fallback glyph measured in its own units.
    pub fn text_ink(&self, text: &str, size: f32) -> Ink {
        let mut cursor = 0.0f32;
        let mut left = f32::MAX;
        let mut right = f32::MIN;
        let mut top = f32::MIN;
        let mut bottom = f32::MAX;
        for ch in text.chars() {
            let Some((index, glyph)) = self.glyph(ch) else {
                cursor += size * 0.5;
                continue;
            };
            let font = &self.fonts[index];
            let scale = size / font.units_per_em;
            if let Some(bounds) = font.glyph_bounds(glyph.gid) {
                left = left.min(cursor + bounds.0 * scale);
                right = right.max(cursor + bounds.2 * scale);
                top = top.max(bounds.3 * scale);
                bottom = bottom.min(bounds.1 * scale);
            }
            cursor += glyph.advance * scale;
        }
        if left > right || top < bottom {
            return Ink::default();
        }
        Ink {
            left,
            right,
            top,
            bottom,
        }
    }

    pub fn missing(&self, text: &str) -> Vec<char> {
        let mut out: Vec<char> = text.chars().filter(|ch| !self.has_glyph(*ch)).collect();
        out.sort_unstable();
        out.dedup();
        out
    }
}

impl std::fmt::Debug for FontStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontStack")
            .field("primary", &self.family())
            .field("faces", &self.fonts.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn load(stem: &str) -> Option<Font> {
        let path = PathBuf::from(format!("fonts/{stem}.ttf"));
        if !path.is_file() {
            return None;
        }
        Some(Font::load(&path).expect("bundled font loads"))
    }

    #[test]
    fn falls_back_per_character() {
        let (Some(latin), Some(cjk)) = (load("quicksand"), load("zcool-kuaile")) else {
            return;
        };
        let stack = FontStack::new(vec![latin, cjk], 0);
        assert!(stack.glyph('A').is_some());
        let (index, _) = stack.glyph('中').expect("CJK fallback provides 中");
        assert_eq!(index, 1);
        assert!(stack.text_width("A中", 14.0) > 0.0);
        assert_eq!(stack.missing("A中"), Vec::<char>::new());
    }

    #[test]
    fn reports_characters_no_face_covers() {
        let Some(font) = load("quicksand") else {
            return;
        };
        let stack = FontStack::single(font);
        let missing = stack.missing("A\u{10FFFF}");
        assert!(!missing.is_empty());
        assert_eq!(stack.family(), "quicksand");
        assert_eq!(stack.len(), 1);
    }
}
