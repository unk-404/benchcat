//! Font file resolution: explicit path, then directory conventions.

use std::path::{Path, PathBuf};

use super::{Font, FontError, FontStack};

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
    if let Ok(dir) = std::env::var("BENCHCAT_FONT_DIR") {
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

/// Faces tried after a custom face, in this order, when
/// `enable_guaranteed_font` is on. Fixed on purpose: repository fonts first,
/// then a short system list. Loading stops at [`MAX_FACES`] faces.
const REPO_GUARANTEED: &[&str] = &[
    "mplus-rounded",
    "zen-maru-gothic",
    "zcool-kuaile",
    "quicksand",
];
const MAX_FACES: usize = 4;

#[cfg(target_os = "windows")]
const SYSTEM_FONTS: &[&str] = &[
    "C:\\Windows\\Fonts\\segoeui.ttf",
    "C:\\Windows\\Fonts\\arial.ttf",
    "C:\\Windows\\Fonts\\malgun.ttf",
];

#[cfg(target_os = "macos")]
const SYSTEM_FONTS: &[&str] = &[
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "/Library/Fonts/Arial.ttf",
];

#[cfg(all(unix, not(target_os = "macos")))]
const SYSTEM_FONTS: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
];

#[cfg(not(any(unix, target_os = "windows")))]
const SYSTEM_FONTS: &[&str] = &[];

/// Load the configured face, plus (optionally) the fixed guaranteed list.
///
/// Without `guaranteed` this is exactly [`resolve_font`] plus [`Font::load`],
/// so existing projects keep their old behaviour and bytes.
pub fn load_stack(
    explicit: Option<&std::path::Path>,
    families: &[String],
    guaranteed: bool,
) -> Result<FontStack, FontError> {
    if !guaranteed {
        if explicit.is_some() {
            let path = resolve_font(explicit, None)?;
            return Ok(FontStack::single(Font::load(&path)?));
        }
        return match families {
            [] => {
                let path = resolve_font(None, None)?;
                Ok(FontStack::single(Font::load(&path)?))
            }
            [one] => {
                // Preserve the historical single-family behaviour, including
                // the fallback to the first font on disk when the stem is
                // absent, so old configs keep their exact output.
                let path = resolve_font(None, Some(one))?;
                Ok(FontStack::single(Font::load(&path)?))
            }
            many => {
                let mut fonts: Vec<Font> = Vec::new();
                for name in many {
                    if let Some(path) = family_path(name)
                        && let Ok(font) = Font::load(&path)
                        && !fonts
                            .iter()
                            .any(|loaded| same_file(&loaded.path, &font.path))
                    {
                        fonts.push(font);
                    }
                }
                if fonts.is_empty() {
                    let path = resolve_font(None, None)?;
                    return Ok(FontStack::single(Font::load(&path)?));
                }
                Ok(FontStack::new(fonts, 0))
            }
        };
    }

    let mut fonts: Vec<Font> = Vec::new();
    if let Some(path) = explicit {
        if let Ok(font) = Font::load(path) {
            fonts.push(font);
        }
    } else {
        for name in families {
            if let Some(path) = family_path(name)
                && let Ok(font) = Font::load(&path)
                && !fonts
                    .iter()
                    .any(|loaded| same_file(&loaded.path, &font.path))
            {
                fonts.push(font);
            }
        }
    }

    if fonts.len() < MAX_FACES {
        for path in guaranteed_paths() {
            if fonts.iter().any(|font| same_file(&font.path, &path)) {
                continue;
            }
            if let Ok(font) = Font::load(&path) {
                fonts.push(font);
            }
            if fonts.len() >= MAX_FACES {
                break;
            }
        }
    }

    if fonts.is_empty() {
        return Err(FontError::Missing);
    }
    Ok(FontStack::new(fonts, 0))
}

fn family_path(name: &str) -> Option<std::path::PathBuf> {
    for dir in font_dirs() {
        for ext in ["ttf", "otf"] {
            let path = dir.join(format!("{name}.{ext}"));
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

fn guaranteed_paths() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    let dirs = font_dirs();
    for stem in REPO_GUARANTEED {
        for dir in &dirs {
            for ext in ["ttf", "otf"] {
                let path = dir.join(format!("{stem}.{ext}"));
                if path.is_file() {
                    paths.push(path);
                }
            }
        }
    }
    for path in SYSTEM_FONTS {
        let path = std::path::PathBuf::from(path);
        if path.is_file() {
            paths.push(path);
        }
    }
    paths
}

fn font_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(dir) = std::env::var("BENCHCAT_FONT_DIR") {
        dirs.push(std::path::PathBuf::from(dir));
    }
    dirs.push(std::path::PathBuf::from("fonts"));
    dirs
}

fn same_file(left: &std::path::Path, right: &std::path::Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn have(stem: &str) -> bool {
        std::path::Path::new(&format!("fonts/{stem}.ttf")).is_file()
    }

    #[test]
    fn list_preserves_order_and_falls_back_per_character() {
        if !have("quicksand") || !have("zcool-kuaile") {
            return;
        }
        let stack = load_stack(
            None,
            &["quicksand".to_owned(), "zcool-kuaile".to_owned()],
            false,
        )
        .expect("stack loads");
        assert_eq!(stack.len(), 2);
        assert_eq!(stack.family(), "quicksand");
        assert!(stack.glyph('中').is_some());
    }

    #[test]
    fn guaranteed_faces_follow_the_custom_list() {
        if !have("quicksand") {
            return;
        }
        let stack = load_stack(None, &["quicksand".to_owned()], true).expect("stack loads");
        assert!(stack.len() >= 2);
        assert_eq!(stack.family(), "quicksand");
    }
}
