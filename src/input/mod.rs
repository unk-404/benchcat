//! Input loading: canonical JSON/JSONL, benchmark_test JSONL, and CSV.
//!
//! Every loader funnels into [`crate::model::Benchmark`], so the renderer never
//! needs to know which tool produced the numbers.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::model::{Benchmark, Meta, json_scalar};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    #[default]
    Auto,
    Json,
    Jsonl,
    Csv,
    /// The JSONL emitted by Dart's `benchmark_test` package.
    BenchmarkTest,
    /// A criterion output directory (`target/criterion`).
    Criterion,
    /// `hyperfine --export-json` output.
    Hyperfine,
}

impl Format {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" | "" => Some(Self::Auto),
            "json" | "canonical" | "bench-svg" => Some(Self::Json),
            "jsonl" | "ndjson" => Some(Self::Jsonl),
            "csv" => Some(Self::Csv),
            "benchmark_test" | "benchmark-test" | "dart" => Some(Self::BenchmarkTest),
            "criterion" | "cargo-criterion" => Some(Self::Criterion),
            "hyperfine" => Some(Self::Hyperfine),
            _ => None,
        }
    }

    pub const fn all() -> &'static [&'static str] {
        &[
            "auto",
            "json",
            "jsonl",
            "csv",
            "benchmark_test",
            "criterion",
            "hyperfine",
        ]
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InputError {
    #[error("input path does not exist: {0}")]
    Missing(String),
    #[error("cannot read {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("{path}: invalid {format} input: {message}")]
    Parse {
        path: String,
        format: &'static str,
        message: String,
    },
    #[error("no benchmark records found in: {0}")]
    Empty(String),
    #[error("unknown format {0:?}; expected one of {1:?}")]
    UnknownFormat(String, &'static [&'static str]),
}

#[derive(Debug, Default)]
pub struct Loaded {
    pub benchmarks: Vec<Benchmark>,
    pub meta: Meta,
    pub sources: Vec<String>,
}

/// Expand directories, then parse every file with the requested (or detected) format.
pub fn load(paths: &[PathBuf], format: Format) -> Result<Loaded, InputError> {
    let mut loaded = Loaded::default();
    let mut files = Vec::new();

    for path in paths {
        // A criterion output directory is a tree of estimates, not a document.
        if format == Format::Criterion || (format == Format::Auto && is_criterion_dir(path)) {
            let records =
                crate::adapters::load_criterion(path).map_err(|message| InputError::Parse {
                    path: path.display().to_string(),
                    format: "criterion",
                    message,
                })?;
            loaded
                .meta
                .tool
                .get_or_insert_with(|| "criterion".to_owned());
            loaded.benchmarks.extend(records);
            loaded.sources.push(path.display().to_string());
            continue;
        }
        let before = files.len();
        collect_files(path, &mut files)?;
        // Directory order is filesystem-dependent; sort so every platform reads
        // the same sequence.
        files[before..].sort();
        if files.len() == before && path.is_dir() {
            return Err(InputError::Empty(path.display().to_string()));
        }
    }

    let total_bytes: u64 = files
        .iter()
        .filter_map(|file| std::fs::metadata(file).ok())
        .map(|meta| meta.len())
        .sum();
    let parsed: Vec<Result<Loaded, InputError>> = if files.len() >= 2 && total_bytes >= 128 * 1024 {
        // Files are independent; merging below keeps the argv order, so the
        // output is unchanged.
        files
            .par_iter()
            .map(|file| parse_path(file, format))
            .collect()
    } else {
        files.iter().map(|file| parse_path(file, format)).collect()
    };
    for (file, parsed) in files.iter().zip(parsed) {
        let parsed = parsed?;
        if loaded.meta.tool.is_none() {
            loaded.meta = parsed.meta;
        }
        loaded.benchmarks.extend(parsed.benchmarks);
        loaded.sources.push(file.display().to_string());
    }

    loaded.benchmarks = dedupe(loaded.benchmarks);
    if loaded.benchmarks.is_empty() {
        return Err(InputError::Empty(
            paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
    Ok(loaded)
}

fn parse_path(file: &Path, format: Format) -> Result<Loaded, InputError> {
    let text = std::fs::read_to_string(file).map_err(|source| InputError::Io {
        path: file.display().to_string(),
        source,
    })?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    parse_file(text, file, format)
}

/// True when `path` looks like a criterion output tree, i.e. it contains at
/// least one `.../new/estimates.json`.
fn is_criterion_dir(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    fn probe(dir: &Path, depth: usize) -> bool {
        if depth > 6 {
            return false;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = path.file_name().unwrap_or_default();
            if name == "report" || name == "base" || name == "change" {
                continue;
            }
            if name == "new" && path.join("estimates.json").is_file() {
                return true;
            }
            if probe(&path, depth + 1) {
                return true;
            }
        }
        false
    }
    probe(path, 0)
}

fn collect_files(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), InputError> {
    if path.is_file() {
        out.push(path.to_path_buf());
        return Ok(());
    }
    if path.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(path)
            .map_err(|source| InputError::Io {
                path: path.display().to_string(),
                source,
            })?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .collect();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                // criterion writes its own HTML/SVG reports; never ingest them.
                if entry.file_name().is_some_and(|name| name == "report") {
                    continue;
                }
                collect_files(&entry, out)?;
            } else if is_data_file(&entry) {
                out.push(entry);
            }
        }
        return Ok(());
    }
    Err(InputError::Missing(path.display().to_string()))
}

fn is_data_file(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => matches!(
            ext.to_ascii_lowercase().as_str(),
            "json" | "jsonl" | "ndjson" | "csv" | "tsv"
        ),
        None => false,
    }
}

fn parse_file(text: &str, path: &Path, format: Format) -> Result<Loaded, InputError> {
    let name = path.display().to_string();
    let detected = match format {
        Format::Auto => detect(text, path),
        other => other,
    };
    let mut loaded = match detected {
        Format::Json => parse_json(text, &name)?,
        Format::Jsonl => parse_jsonl(text, &name)?,
        Format::Csv => parse_csv(text, &name)?,
        Format::BenchmarkTest => parse_benchmark_test(text, &name)?,
        Format::Hyperfine => parse_json(text, &name)?,
        Format::Criterion => {
            return Err(InputError::Parse {
                path: name,
                format: "criterion",
                message: "criterion input must be a directory tree, not a single file".to_owned(),
            });
        }
        Format::Auto => unreachable!("auto is resolved above"),
    };
    if loaded.meta.tool.is_none() {
        loaded.meta.tool = Some(match detected {
            Format::BenchmarkTest => "benchmark_test".to_owned(),
            Format::Csv => "csv".to_owned(),
            _ => "bench-svg".to_owned(),
        });
    }
    Ok(loaded)
}

fn detect(text: &str, path: &Path) -> Format {
    if matches!(
        path.extension().and_then(|ext| ext.to_str()).map(str::to_ascii_lowercase),
        Some(ext) if ext == "csv" || ext == "tsv"
    ) {
        return Format::Csv;
    }
    // Byte-order marks are common when a file is produced by PowerShell or
    // Excel; they must not change how the document is detected.
    let trimmed = text.strip_prefix('\u{feff}').unwrap_or(text).trim_start();
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        // A whole-document JSON object, or a stream of JSONL objects.
        if trimmed.starts_with('{') && looks_like_jsonl(trimmed) {
            return if trimmed.contains("\"throughput\"") {
                Format::BenchmarkTest
            } else {
                Format::Jsonl
            };
        }
        return Format::Json;
    }
    Format::Csv
}

/// Two leading lines that are both complete objects mean JSONL, not a padded
/// JSON document.
fn looks_like_jsonl(text: &str) -> bool {
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let Some(first) = lines.next() else {
        return false;
    };
    if !(first.starts_with('{') && first.ends_with('}')) {
        return false;
    }
    lines
        .next()
        .is_some_and(|second| second.starts_with('{') && second.ends_with('}'))
}

mod parsers;

use parsers::{is_jit, parse_benchmark_test, parse_csv, parse_json, parse_jsonl};

/// Keep one record per id. When a file contains JIT and AOT runs of the same
/// benchmark, prefer the JIT number, matching the Dart renderer's behaviour.
fn dedupe(benchmarks: Vec<Benchmark>) -> Vec<Benchmark> {
    let mut order: Vec<String> = Vec::new();
    let mut slots: HashMap<String, Benchmark> = HashMap::new();
    for benchmark in benchmarks {
        match slots.get(&benchmark.id) {
            None => {
                order.push(benchmark.id.clone());
                slots.insert(benchmark.id.clone(), benchmark);
            }
            Some(existing) => {
                let existing_is_jit = is_jit(existing);
                let new_is_jit = is_jit(&benchmark);
                if !existing_is_jit && new_is_jit {
                    slots.insert(benchmark.id.clone(), benchmark);
                }
            }
        }
    }
    order
        .into_iter()
        .filter_map(|id| slots.remove(&id))
        .collect()
}
/// Build the `note` line for a row from structured parameters.
pub fn params_note(benchmark: &Benchmark) -> Option<String> {
    if !benchmark.params.is_empty() {
        let mut parts: Vec<String> = benchmark
            .params
            .iter()
            .map(|(key, value)| format!("{key} {}", json_scalar(value)))
            .collect();
        parts.sort();
        return Some(parts.join(" · "));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_benchmark_test_jsonl_with_bom() {
        let text = "\u{feff}{\"name\":\"a\",\"throughput\":{\"value\":1,\"unit\":\"ops/s\"}}\n\
                    {\"name\":\"b\",\"throughput\":{\"value\":2,\"unit\":\"ops/s\"}}\n";
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        assert_eq!(detect(text, Path::new("x.jsonl")), Format::BenchmarkTest);
    }

    #[test]
    fn detects_pretty_printed_json_document() {
        let text = "{\n  \"schema\": \"bench-svg/report@1\",\n  \"benchmarks\": []\n}";
        assert_eq!(detect(text, Path::new("x.json")), Format::Json);
    }

    #[test]
    fn detects_csv_by_extension() {
        assert_eq!(detect("id,value\n", Path::new("x.csv")), Format::Csv);
    }

    #[test]
    fn parses_benchmark_test_and_skips_progress_output() {
        let text = "progress 10%\n\
                    {\"name\":\"file.read\",\"throughput\":{\"value\":2.5,\"unit\":\"GiB/s\"},\
                     \"statistics\":{\"relativeMarginOfError\":1.5},\"compiler\":\"jit\"}\n";
        let loaded = parse_benchmark_test(text, "mem").expect("parses");
        assert_eq!(loaded.benchmarks.len(), 1);
        assert_eq!(loaded.benchmarks[0].id, "file.read");
        assert_eq!(loaded.benchmarks[0].unit, "GiB/s");
        assert_eq!(loaded.benchmarks[0].stats.rme_pct, Some(1.5));
    }

    #[test]
    fn prefers_jit_records_when_both_exist() {
        let text = "{\"name\":\"x\",\"throughput\":{\"value\":1,\"unit\":\"ops/s\"},\"compiler\":\"aot\"}\n\
                    {\"name\":\"x\",\"throughput\":{\"value\":9,\"unit\":\"ops/s\"},\"compiler\":\"jit\"}\n";
        let loaded = parse_benchmark_test(text, "mem").expect("parses");
        let deduped = dedupe(loaded.benchmarks);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].value, 9.0);
    }
}
