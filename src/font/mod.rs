//! Runtime TrueType parsing: metrics, advances and glyph outlines.
//!
//! The font is loaded from disk (never embedded), so a project can ship any
//! TrueType/OpenType face it likes. Measurement and drawing always read the
//! same file, which keeps layout and glyphs self-consistent by construction.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, MetadataProvider};

/// Below this many distinct glyphs the thread pool costs more than it saves.
const PARALLEL_GLYPH_THRESHOLD: usize = 96;
/// Glyphs per worker chunk; each chunk builds one `FontRef` and reuses it.
const GLYPH_CHUNK: usize = 64;

/// Outline bounds in font units: `(x_min, y_min, x_max, y_max)`.
type GlyphBounds = (f32, f32, f32, f32);

#[derive(Debug, thiserror::Error)]
pub enum FontError {
    #[error("cannot read font {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("cannot parse font {path}: {reason}")]
    Parse { path: String, reason: String },
    #[error("no font configured: pass --font-file or place a .ttf next to the config")]
    Missing,
}

/// A mapped character and everything measured once at load time.
#[derive(Debug, Clone, Copy)]
pub struct Glyph {
    pub gid: u16,
    /// Advance width in font units.
    pub advance: f32,
}

/// Ink rectangle of a text run, in pixels relative to the baseline.
///
/// `top` is positive above the baseline, `bottom` negative below it, so the
/// optical centre of the run sits at `baseline - (top + bottom) / 2`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ink {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Ink {
    /// Vertical offset of the ink centre above the baseline.
    pub fn center_up(&self) -> f32 {
        (self.top + self.bottom) / 2.0
    }

    /// Horizontal offset of the ink centre from the pen start.
    pub fn center_x(&self) -> f32 {
        (self.left + self.right) / 2.0
    }
}

/// A loaded font. Measurement is lock-free; outlines are extracted in parallel.
pub struct Font {
    data: Vec<u8>,
    pub family: String,
    pub path: PathBuf,
    pub units_per_em: f32,
    pub ascent: f32,
    pub descent: f32,
    pub leading: f32,
    /// Number of glyphs in the face, from the font's own metrics.
    pub glyph_count: u16,
    glyphs: HashMap<char, Glyph>,
    paths: Mutex<HashMap<u16, String>>,
    bounds: Mutex<HashMap<u16, Option<GlyphBounds>>>,
}

impl std::fmt::Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Font")
            .field("family", &self.family)
            .field("path", &self.path)
            .field("units_per_em", &self.units_per_em)
            .field("ascent", &self.ascent)
            .field("descent", &self.descent)
            .field("glyphs", &self.glyphs.len())
            .finish()
    }
}

impl Font {
    pub fn load(path: &Path) -> Result<Self, FontError> {
        let data = std::fs::read(path).map_err(|source| FontError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_bytes(data, path.to_path_buf())
    }

    pub fn from_bytes(data: Vec<u8>, path: PathBuf) -> Result<Self, FontError> {
        let font = FontRef::new(&data).map_err(|source| FontError::Parse {
            path: path.display().to_string(),
            reason: source.to_string(),
        })?;
        let metrics = font.metrics(Size::unscaled(), LocationRef::default());
        let metrics_h = font.glyph_metrics(Size::unscaled(), LocationRef::default());
        let mut glyphs: HashMap<char, Glyph> = HashMap::new();
        for (codepoint, gid) in font.charmap().mappings() {
            let Some(ch) = char::from_u32(codepoint) else {
                continue;
            };
            let advance = metrics_h.advance_width(gid).unwrap_or(0.0);
            glyphs.entry(ch).or_insert(Glyph {
                gid: (gid.to_u32() & 0xFFFF) as u16,
                advance,
            });
        }
        let family = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "font".to_owned());
        Ok(Self {
            data,
            family,
            path,
            units_per_em: f32::from(metrics.units_per_em),
            ascent: metrics.ascent,
            descent: metrics.descent,
            leading: metrics.leading,
            glyph_count: metrics.glyph_count,
            glyphs,
            paths: Mutex::new(HashMap::new()),
            bounds: Mutex::new(HashMap::new()),
        })
    }

    fn font(&self) -> FontRef<'_> {
        FontRef::new(&self.data).expect("font was validated during load")
    }

    /// Glyph for `ch`, or `None` when the face has no glyph for it.
    pub fn glyph(&self, ch: char) -> Option<Glyph> {
        self.glyphs.get(&ch).copied()
    }

    pub fn has_glyph(&self, ch: char) -> bool {
        self.glyphs.contains_key(&ch)
    }

    /// Number of codepoints the face maps to a glyph.
    pub fn char_count(&self) -> usize {
        self.glyphs.len()
    }

    /// Advance width in font units.
    pub fn advance_units(&self, ch: char) -> f32 {
        self.glyphs.get(&ch).map_or(0.0, |glyph| glyph.advance)
    }

    /// Advance width in pixels at `size`.
    pub fn advance(&self, ch: char, size: f32) -> f32 {
        match self.glyphs.get(&ch) {
            Some(glyph) => glyph.advance * size / self.units_per_em,
            None => size * 0.5,
        }
    }
    /// Characters used by `text` that this face cannot draw.
    pub fn missing(&self, text: &str) -> Vec<char> {
        let mut out: Vec<char> = text.chars().filter(|ch| !self.has_glyph(*ch)).collect();
        out.sort_unstable();
        out.dedup();
        out
    }
}

mod metrics;
mod outline;
mod resolve;

pub use resolve::resolve_font;

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_font() -> Option<Font> {
        let path = PathBuf::from("fonts/mplus-rounded.ttf");
        if !path.is_file() {
            return None;
        }
        Some(Font::load(&path).expect("sample font loads"))
    }

    #[test]
    fn loads_metrics_and_advances() {
        let Some(font) = sample_font() else { return };
        assert!(font.units_per_em > 0.0);
        assert!(font.ascent > 0.0);
        assert!(font.text_width("Benchmark", 14.0) > 0.0);
    }

    #[test]
    fn produces_outline_paths() {
        let Some(font) = sample_font() else { return };
        let glyph = font.glyph('A').expect("A exists");
        let path = font.path(glyph.gid);
        assert!(path.starts_with('M'), "unexpected path: {path}");
        assert!(path.ends_with('Z'));
    }

    #[test]
    fn bulk_extraction_matches_cached_paths() {
        let Some(font) = sample_font() else { return };
        let gids: Vec<u16> = "Benchmark 0123456789 MiB/s"
            .chars()
            .filter_map(|ch| font.glyph(ch))
            .map(|glyph| glyph.gid)
            .collect();
        let bulk = font.extract_paths(&gids);
        for (gid, path) in gids.iter().zip(bulk.iter()) {
            assert_eq!(*path, font.path(*gid));
        }
    }
}
