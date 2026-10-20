//! Glyph outline extraction (single and parallel).

use rayon::prelude::*;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::model::glyph::outline::ControlBoundsPen;
use skrifa::{FontRef, GlyphId, MetadataProvider};

use super::{Font, GLYPH_CHUNK, GlyphBounds, PARALLEL_GLYPH_THRESHOLD};

impl Font {
    /// SVG path for one glyph, cached. Use [`Font::extract_paths`] in bulk.
    pub fn path(&self, gid: u16) -> String {
        if let Some(path) = self.paths.get(&gid).map(|path| path.clone()) {
            return path;
        }
        let font = self.font();
        let path = outline_path(&font, gid);
        self.paths.insert(gid, path.clone());
        path
    }

    /// Outline bounds in font units, cached per glyph.
    pub(super) fn glyph_bounds(&self, gid: u16) -> Option<GlyphBounds> {
        if let Some(cached) = self.bounds.get(&gid).map(|bounds| *bounds) {
            return cached;
        }
        let font = self.font();
        let bounds = font
            .outline_glyphs()
            .get(GlyphId::new(u32::from(gid)))
            .and_then(|glyph| {
                let mut pen = ControlBoundsPen::new();
                let settings = DrawSettings::unhinted(Size::unscaled(), LocationRef::default());
                glyph.draw(settings, &mut pen).ok()?;
                pen.bounding_box()
                    .map(|bounds| (bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max))
            });
        self.bounds.insert(gid, bounds);
        bounds
    }

    /// Extract many glyph outlines, in parallel when it is worth the overhead.
    ///
    /// Results are returned in the same order as `gids`, so the emitted document
    /// is byte-identical regardless of how many threads ran.
    pub fn extract_paths(&self, gids: &[u16]) -> Vec<String> {
        if gids.len() < PARALLEL_GLYPH_THRESHOLD {
            let font = self.font();
            return gids.iter().map(|gid| outline_path(&font, *gid)).collect();
        }
        let data = self.data.as_slice();
        let chunks: Vec<&[u16]> = gids.chunks(GLYPH_CHUNK).collect();
        let parts: Vec<Vec<String>> = chunks
            .par_iter()
            .map(|chunk| {
                let font = FontRef::new(data).expect("font was validated during load");
                chunk.iter().map(|gid| outline_path(&font, *gid)).collect()
            })
            .collect();
        parts.into_iter().flatten().collect()
    }
}

/// Draw one glyph into compact SVG path data, in font units with y up.
fn outline_path(font: &FontRef<'_>, gid: u16) -> String {
    let mut pen = PathPen::default();
    if let Some(outline) = font.outline_glyphs().get(GlyphId::new(u32::from(gid))) {
        let settings = DrawSettings::unhinted(Size::unscaled(), LocationRef::default());
        let _ = outline.draw(settings, &mut pen);
    }
    pen.finish()
}

/// Collects outline commands into compact SVG path data.
#[derive(Default)]
struct PathPen {
    d: String,
    started: bool,
}

impl PathPen {
    fn finish(self) -> String {
        self.d
    }

    fn coord(&mut self, value: f32) -> String {
        let rounded = (f64::from(value) * 10.0).round() / 10.0;
        if rounded == rounded.trunc() {
            format!("{}", rounded as i64)
        } else {
            let text = format!("{rounded:.1}");
            text.trim_end_matches('0').trim_end_matches('.').to_owned()
        }
    }

    fn point(&mut self, x: f32, y: f32) {
        if self.started {
            self.d.push(' ');
        }
        self.started = true;
        let x = self.coord(x);
        let y = self.coord(y);
        self.d.push_str(&x);
        self.d.push(' ');
        self.d.push_str(&y);
    }
}

impl OutlinePen for PathPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.d.push('M');
        self.started = false;
        self.point(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.d.push('L');
        self.started = false;
        self.point(x, y);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.d.push('Q');
        self.started = false;
        self.point(cx0, cy0);
        self.point(x, y);
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.d.push('C');
        self.started = false;
        self.point(cx0, cy0);
        self.point(cx1, cy1);
        self.point(x, y);
    }

    fn close(&mut self) {
        self.d.push('Z');
        self.started = false;
    }
}
