//! Font file resolution: explicit path, directory conventions, then system
//! faces when `enable_system_font` is on.

use std::path::{Path, PathBuf};

use skrifa::string::StringId;
use skrifa::{FontRef, MetadataProvider};

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
    for dir in font_dirs() {
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

#[derive(Debug, Clone)]
struct FaceRef {
    path: PathBuf,
    index: u32,
}

/// Load the configured face, plus (optionally) the fixed guaranteed list.
///
/// Without `guaranteed` this is exactly [`resolve_font`] plus [`Font::load`],
/// so existing projects keep their old behaviour and bytes. `system` only
/// matters when the caller asked for it: it lets a family name resolve against
/// the operating system's installed fonts after the repository directories.
pub fn load_stack(
    explicit: Option<&std::path::Path>,
    families: &[String],
    guaranteed: bool,
) -> Result<FontStack, FontError> {
    load_stack_with_system(explicit, families, guaranteed, false)
}

/// Like [`load_stack`], but optionally resolves family names against installed
/// system fonts. `system = false` is byte-for-byte the old behaviour.
pub fn load_stack_with_system(
    explicit: Option<&std::path::Path>,
    families: &[String],
    guaranteed: bool,
    system: bool,
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
            [one] if !system => {
                // Preserve the historical single-family behaviour, including
                // the fallback to the first font on disk when the stem is
                // absent, so old configs keep their exact output.
                let path = resolve_font(None, Some(one))?;
                Ok(FontStack::single(Font::load(&path)?))
            }
            _ => {
                let mut faces: Vec<FaceRef> = Vec::new();
                for name in families {
                    if let Some(face) = named_face(name, system) {
                        push_unique(&mut faces, face);
                    }
                }
                let mut fonts: Vec<Font> = Vec::new();
                for face in &faces {
                    if let Ok(font) = Font::load_face(&face.path, face.index) {
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

    let mut faces: Vec<FaceRef> = Vec::new();
    if let Some(path) = explicit {
        push_unique(
            &mut faces,
            FaceRef {
                path: path.to_path_buf(),
                index: 0,
            },
        );
    } else {
        for name in families {
            if let Some(face) = named_face(name, system) {
                push_unique(&mut faces, face);
            }
        }
    }
    if faces.len() < MAX_FACES {
        for face in guaranteed_faces() {
            if faces.len() >= MAX_FACES {
                break;
            }
            push_unique(&mut faces, face);
        }
    }

    let mut fonts: Vec<Font> = Vec::new();
    for face in &faces {
        if let Ok(font) = Font::load_face(&face.path, face.index) {
            fonts.push(font);
        }
    }
    if fonts.is_empty() {
        return Err(FontError::Missing);
    }
    Ok(FontStack::new(fonts, 0))
}

fn named_face(name: &str, system: bool) -> Option<FaceRef> {
    family_path(name)
        .map(|path| FaceRef { path, index: 0 })
        .or_else(|| if system { system_face(name) } else { None })
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

fn guaranteed_faces() -> Vec<FaceRef> {
    let mut faces = Vec::new();
    let dirs = font_dirs();
    for stem in REPO_GUARANTEED {
        for dir in &dirs {
            for ext in ["ttf", "otf"] {
                let path = dir.join(format!("{stem}.{ext}"));
                if path.is_file() {
                    faces.push(FaceRef { path, index: 0 });
                }
            }
        }
    }
    for path in SYSTEM_FONTS {
        let path = PathBuf::from(path);
        if path.is_file() {
            faces.push(FaceRef { path, index: 0 });
        }
    }
    faces
}

fn push_unique(faces: &mut Vec<FaceRef>, face: FaceRef) {
    if !faces
        .iter()
        .any(|other| other.index == face.index && same_file(&other.path, &face.path))
    {
        faces.push(face);
    }
}

/// Find an installed face whose family name or file stem matches `name`.
fn system_face(name: &str) -> Option<FaceRef> {
    let wanted = normalize(name);
    if wanted.is_empty() {
        return None;
    }
    let files = system_files();
    // Cheap first pass: the file stem often matches even when the name table
    // uses a different spelling (segoeui.ttf, arial.ttf, ...).
    for path in &files {
        let stem = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        if normalize(&stem) == wanted
            && let Some(index) = matching_face(path, &wanted)
        {
            return Some(FaceRef {
                path: path.clone(),
                index,
            });
        }
    }
    for path in &files {
        if let Some(index) = matching_face(path, &wanted) {
            return Some(FaceRef {
                path: path.clone(),
                index,
            });
        }
    }
    None
}

fn matching_face(path: &Path, wanted: &str) -> Option<u32> {
    let data = std::fs::read(path).ok()?;
    let faces = if data.starts_with(b"ttcf") { 64 } else { 1 };
    for index in 0..faces {
        let Ok(font) = FontRef::from_index(&data, index) else {
            break;
        };
        let Some(family) = font
            .localized_strings(StringId::FAMILY_NAME)
            .next()
            .map(|name| name.to_string())
        else {
            continue;
        };
        if normalize(&family) == wanted {
            return Some(index);
        }
    }
    None
}

fn normalize(name: &str) -> String {
    name.chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn system_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for dir in system_dirs() {
        collect_font_files(&dir, &mut files, 0);
    }
    files.sort();
    files.dedup();
    files
}

fn collect_font_files(dir: &Path, files: &mut Vec<PathBuf>, depth: u8) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(|entry| entry.ok()).collect();
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            collect_font_files(&path, files, depth + 1);
        } else if kind.is_file() && is_font_file(&path) {
            files.push(path);
        }
    }
}

fn is_font_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| {
        ["ttf", "otf", "ttc", "otc"]
            .iter()
            .any(|wanted| ext.eq_ignore_ascii_case(wanted))
    })
}

fn system_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    #[cfg(target_os = "windows")]
    {
        let windir = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:\\Windows"));
        dirs.push(windir.join("Fonts"));
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(local).join("Microsoft/Windows/Fonts"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/System/Library/Fonts"));
        dirs.push(PathBuf::from("/System/Library/Fonts/Supplemental"));
        dirs.push(PathBuf::from("/Library/Fonts"));
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(home).join("Library/Fonts"));
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        dirs.push(PathBuf::from("/usr/share/fonts"));
        dirs.push(PathBuf::from("/usr/local/share/fonts"));
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            dirs.push(home.join(".local/share/fonts"));
            dirs.push(home.join(".fonts"));
        }
    }
    dirs
}

fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(dir) = std::env::var("BENCHCAT_FONT_DIR") {
        dirs.push(PathBuf::from(dir));
    }
    dirs.push(PathBuf::from("fonts"));
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

    #[test]
    fn system_name_normalization_ignores_case_and_punctuation() {
        assert_eq!(normalize("Noto Sans"), normalize("noto-sans"));
        assert_eq!(normalize("Segoe UI"), "segoeui");
    }
}
