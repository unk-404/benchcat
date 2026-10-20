//! Generated source registry.
//!
//! `sources!` expands to real `mod` declarations for every `sources/*.rs`
//! file and the `SOURCES` table. No runtime registration or parsing is
//! involved.

use std::path::Path;

use crate::source::SourceDef;
use benchcat_source_gen::sources;

sources!("../../sources");

pub fn find(name: &str) -> Option<&'static SourceDef> {
    SOURCES
        .iter()
        .copied()
        .find(|source| source.matches_alias(name))
}

pub fn detect(text: &str) -> Option<&'static SourceDef> {
    SOURCES
        .iter()
        .copied()
        .find(|source| source.detect.is_some_and(|detect| detect(text)))
}

pub fn find_dir(path: &Path) -> Option<&'static SourceDef> {
    SOURCES
        .iter()
        .copied()
        .find(|source| source.detect_dir.is_some_and(|detect| detect(path)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_sources_are_registered_with_unique_names() {
        assert!(SOURCES.len() >= 8);
        assert!(find("go").is_some());
        assert!(find("kotlinx").is_some());
        let mut names = std::collections::HashSet::new();
        for source in SOURCES.iter().copied() {
            assert!(
                names.insert(source.name),
                "duplicate source name {}",
                source.name
            );
        }
    }
}
