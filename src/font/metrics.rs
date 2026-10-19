//! Text measurement on top of the cached glyph metrics.

use super::{Font, Ink};

impl Font {
    /// Width of `text` at `size`, in pixels.
    pub fn text_width(&self, text: &str, size: f32) -> f32 {
        text.chars().map(|ch| self.advance(ch, size)).sum()
    }

    /// Distance from the top of the em box down to the baseline, in pixels.
    pub fn ascent_px(&self, size: f32) -> f32 {
        self.ascent * size / self.units_per_em
    }

    /// Recommended line height in pixels at `size`.
    pub fn line_height_px(&self, size: f32) -> f32 {
        let natural = (self.ascent - self.descent + self.leading) * size / self.units_per_em;
        natural.max(size * 1.15)
    }

    /// Ink bounding box of `text` at `size`, in pixels, relative to a pen that
    /// starts on the baseline (y grows upwards). This is the font's real edge,
    /// not the em box, so optical centring can use it.
    pub fn text_ink(&self, text: &str, size: f32) -> Ink {
        let scale = size / self.units_per_em;
        let mut cursor = 0.0f32;
        let mut left = f32::MAX;
        let mut right = f32::MIN;
        let mut top = f32::MIN;
        let mut bottom = f32::MAX;
        for ch in text.chars() {
            let Some(glyph) = self.glyph(ch) else {
                continue;
            };
            if let Some(bounds) = self.glyph_bounds(glyph.gid) {
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
}
