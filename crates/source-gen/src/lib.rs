//! Versioned build-time compiler for `sources/*.oops`.
//!
//! Each `.oops` file declares `@version N`; the matching compiler under
//! `compiler/vN` parses it and emits direct Rust. No runtime interpreter.

use std::path::Path;
use std::path::PathBuf;

use proc_macro::TokenStream;
use quote::{format_ident, quote};

mod compiler;

#[proc_macro]
pub fn sources(input: TokenStream) -> TokenStream {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| expand(input))) {
        Ok(tokens) => tokens,
        Err(_) => compile_error("sources!: internal error while compiling .oops presets"),
    }
}

fn expand(input: TokenStream) -> TokenStream {
    let raw = input.to_string();
    let relative = raw.trim().trim_matches('"');
    if relative.is_empty() || relative.contains('"') {
        return compile_error("sources!: expected a path literal such as \"../../sources\"");
    }

    let compile_manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        candidates.push(Path::new(&manifest).join(relative));
    }
    candidates.push(compile_manifest.join(relative));
    candidates.push(compile_manifest.join("..").join("..").join("sources"));
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(relative));
        candidates.push(cwd.join("sources"));
    }
    let dir = candidates
        .iter()
        .find(|path| path.is_dir())
        .cloned()
        .unwrap_or_else(|| compile_manifest.join("..").join("..").join("sources"));

    let mut files: Vec<String> = match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                (name.ends_with(".oops") && !name.starts_with('_')).then_some(name)
            })
            .collect(),
        Err(error) => {
            return compile_error(&format!("sources!: cannot read {}: {error}", dir.display()));
        }
    };
    files.sort();
    if files.is_empty() {
        return compile_error(&format!("sources!: no .oops files in {}", dir.display()));
    }

    let mut compiled = Vec::new();
    for file in &files {
        let stem = file.trim_end_matches(".oops");
        if !is_identifier(stem) {
            return compile_error(&format!(
                "sources!: {file}: file name must be a valid Rust identifier"
            ));
        }
        let path = dir.join(file);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                return compile_error(&format!(
                    "sources!: cannot read {}: {error}",
                    path.display()
                ));
            }
        };
        let (version, body) = match split_version(&text) {
            Ok(parts) => parts,
            Err(message) => return compile_error(&format!("sources!: {file}: {message}")),
        };
        let tokens = match compiler::compile(version, body) {
            Ok(tokens) => tokens,
            Err(message) => return compile_error(&format!("sources!: {file}: {message}")),
        };
        compiled.push((format_ident!("{stem}"), tokens));
    }

    let modules = compiled.iter().map(|(ident, tokens)| {
        quote! {
            pub mod #ident {
                #tokens
            }
        }
    });
    let entries = compiled
        .iter()
        .map(|(ident, _)| quote! { &self::#ident::SOURCE });
    let output = quote! {
        #(#modules)*
        pub static SOURCES: &[&crate::source::SourceDef] = &[#(#entries),*];
    };
    if std::env::var("OOPS_DUMP").is_ok() {
        let _ = std::fs::write(
            std::env::temp_dir().join("oops-expanded.rs"),
            output.to_string(),
        );
    }
    output.into()
}

fn split_version(text: &str) -> Result<(u32, &str), String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        offset += line.len();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        let Some(value) = trimmed.strip_prefix("@version ") else {
            return Err("first directive must be `@version N`".to_owned());
        };
        let version = value
            .trim()
            .parse::<u32>()
            .map_err(|error| format!("invalid @version: {error}"))?;
        return Ok((version, &text[offset..]));
    }
    Err("missing @version".to_owned())
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(ch) if ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn compile_error(message: &str) -> TokenStream {
    format!("compile_error!({message:?});").parse().unwrap()
}
