//! Font file resolution: explicit path, then directory conventions.

use std::path::{Path, PathBuf};

use super::FontError;

/// Resolve a font file from explicit paths and conventional locations.
pub fn resolve_font(explicit: Option<&Path>, family: Option<&str>) -> Result<PathBuf, FontError> {
    if let Some(path) = explicit {
        if path.is_file() {
            return Ok(path.to_path_buf());
        }
        return Err(FontError::Io {
            path: path.display().to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "font file not found"),
        });
    }
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = std::env::var("BENCH_SVG_FONT_DIR") {
        dirs.push(PathBuf::from(dir));
    }
    dirs.push(PathBuf::from("fonts"));
    for dir in dirs {
        // An explicit family must match exactly; without one, prefer the
        // documented default before falling back to whatever is on disk.
        if let Some(name) = family {
            for ext in ["ttf", "otf"] {
                let candidate = dir.join(format!("{name}.{ext}"));
                if candidate.is_file() {
                    return Ok(candidate);
                }
            }
        } else {
            for ext in ["ttf", "otf"] {
                let candidate = dir.join(format!("mplus-rounded.{ext}"));
                if candidate.is_file() {
                    return Ok(candidate);
                }
            }
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let mut files: Vec<PathBuf> = entries
                .filter_map(|entry| entry.ok().map(|e| e.path()))
                .filter(|path| {
                    path.extension().is_some_and(|ext| {
                        ext.eq_ignore_ascii_case("ttf") || ext.eq_ignore_ascii_case("otf")
                    })
                })
                .collect();
            files.sort();
            if let Some(first) = files.into_iter().next() {
                return Ok(first);
            }
        }
    }
    Err(FontError::Missing)
}
