//! Enforces the versioning contract in `docs/spec/compiler-versioning.md`.

use std::path::Path;

fn collect(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn v1_does_not_reference_later_compilers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect(&root.join("src/compiler/v1"), &mut files);
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(
            !text.contains("v2"),
            "frozen v1 compiler references v2: {}",
            file.display()
        );
    }
}

#[test]
fn every_oops_declares_a_supported_version() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sources = root.join("sources");
    let mut files = Vec::new();
    collect(&sources, &mut files);
    let mut seen = 0;
    for file in files {
        if file.extension().and_then(|ext| ext.to_str()) != Some("oops") {
            continue;
        }
        let text = std::fs::read_to_string(&file).unwrap();
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let first = text
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty() && !line.starts_with("//"))
            .unwrap_or_default();
        assert!(
            first == "@version 1" || first == "@version 2",
            "{} must start with a supported @version, got {first:?}",
            file.display()
        );
        seen += 1;
    }
    assert!(seen >= 1);
}

#[test]
fn compiler_directories_match_supported_versions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/compiler");
    let mut versions = Vec::new();
    for entry in std::fs::read_dir(root).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(version) = name.strip_prefix('v')
            && version.chars().all(|ch| ch.is_ascii_digit())
        {
            versions.push(version.parse::<u32>().unwrap());
        }
    }
    versions.sort_unstable();
    assert_eq!(versions, vec![1, 2]);
}
