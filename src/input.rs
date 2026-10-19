//! Input loading: canonical JSON/JSONL, benchmark_test JSONL, and CSV.
//!
//! Every loader funnels into [`crate::model::Benchmark`], so the renderer never
//! needs to know which tool produced the numbers.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::model::{Benchmark, Direction, Meta, Report, Stats, json_scalar};

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
        if files.len() == before && path.is_dir() {
            return Err(InputError::Empty(path.display().to_string()));
        }
    }

    for file in &files {
        let text = std::fs::read_to_string(file).map_err(|source| InputError::Io {
            path: file.display().to_string(),
            source,
        })?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let parsed = parse_file(text, file, format)?;
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

fn parse_json(text: &str, name: &str) -> Result<Loaded, InputError> {
    let value: Value = serde_json::from_str(text).map_err(|err| InputError::Parse {
        path: name.to_owned(),
        format: "json",
        message: err.to_string(),
    })?;
    let mut loaded = Loaded::default();
    match value {
        Value::Array(items) => {
            for item in items {
                loaded.benchmarks.push(benchmark_from_value(&item, name)?);
            }
        }
        Value::Object(map) => {
            if map.contains_key("benchmarks") {
                let report: Report = serde_json::from_value(Value::Object(map)).map_err(|err| {
                    InputError::Parse {
                        path: name.to_owned(),
                        format: "json",
                        message: err.to_string(),
                    }
                })?;
                loaded.meta = report.meta;
                loaded.benchmarks = report.benchmarks;
            } else if map.contains_key("results") {
                // `hyperfine --export-json` (v1 has no schema_version).
                let rows =
                    crate::adapters::parse_hyperfine(&Value::Object(map)).map_err(|message| {
                        InputError::Parse {
                            path: name.to_owned(),
                            format: "hyperfine",
                            message,
                        }
                    })?;
                loaded.meta.tool = Some("hyperfine".to_owned());
                loaded.benchmarks = rows;
            } else if map.contains_key("throughput") {
                loaded
                    .benchmarks
                    .push(benchmark_test_entry(&Value::Object(map), name)?);
            } else {
                loaded
                    .benchmarks
                    .push(benchmark_from_value(&Value::Object(map), name)?);
            }
        }
        _ => {
            return Err(InputError::Parse {
                path: name.to_owned(),
                format: "json",
                message: "expected an object or an array of benchmark records".to_owned(),
            });
        }
    }
    Ok(loaded)
}

fn parse_jsonl(text: &str, name: &str) -> Result<Loaded, InputError> {
    let mut loaded = Loaded::default();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue; // benchmark_test interleaves progress output with JSON.
        }
        let value: Value = serde_json::from_str(line).map_err(|err| InputError::Parse {
            path: format!("{name}:{}", index + 1),
            format: "jsonl",
            message: err.to_string(),
        })?;
        loaded
            .benchmarks
            .push(benchmark_or_test(&value, name, index + 1)?);
    }
    Ok(loaded)
}

fn parse_benchmark_test(text: &str, name: &str) -> Result<Loaded, InputError> {
    let mut loaded = Loaded::default();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue;
        }
        let value: Value = serde_json::from_str(line).map_err(|err| InputError::Parse {
            path: format!("{name}:{}", index + 1),
            format: "benchmark_test",
            message: err.to_string(),
        })?;
        loaded.benchmarks.push(benchmark_test_entry(
            &value,
            &format!("{name}:{}", index + 1),
        )?);
    }
    loaded.meta.tool = Some("benchmark_test".to_owned());
    Ok(loaded)
}

fn benchmark_or_test(value: &Value, name: &str, line: usize) -> Result<Benchmark, InputError> {
    let located = format!("{name}:{line}");
    if value.get("throughput").is_some() {
        benchmark_test_entry(value, &located)
    } else {
        benchmark_from_value(value, &located)
    }
}

fn benchmark_from_value(value: &Value, name: &str) -> Result<Benchmark, InputError> {
    serde_json::from_value(value.clone()).map_err(|err| InputError::Parse {
        path: name.to_owned(),
        format: "json",
        message: err.to_string(),
    })
}

/// Convert one `benchmark_test` JSONL record.
///
/// ```json
/// {"name":"file.read.8MiB","throughput":{"value":1234.5,"unit":"ops/s"},
///  "statistics":{"relativeMarginOfError":1.2},"compiler":"jit"}
/// ```
fn benchmark_test_entry(value: &Value, name: &str) -> Result<Benchmark, InputError> {
    let id = value
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| InputError::Parse {
            path: name.to_owned(),
            format: "benchmark_test",
            message: "missing `name`".to_owned(),
        })?
        .to_owned();
    let throughput = value.get("throughput").ok_or_else(|| InputError::Parse {
        path: name.to_owned(),
        format: "benchmark_test",
        message: "missing `throughput`".to_owned(),
    })?;
    let measured = throughput
        .get("value")
        .and_then(Value::as_f64)
        .ok_or_else(|| InputError::Parse {
            path: name.to_owned(),
            format: "benchmark_test",
            message: "missing `throughput.value`".to_owned(),
        })?;
    let unit = throughput
        .get("unit")
        .and_then(Value::as_str)
        .unwrap_or("ops/s")
        .to_owned();
    let rme = value
        .get("statistics")
        .and_then(|stats| stats.get("relativeMarginOfError"))
        .and_then(Value::as_f64);

    let mut extra = std::collections::BTreeMap::new();
    if let Some(extra_map) = value.as_object() {
        for (key, item) in extra_map {
            if !matches!(key.as_str(), "name" | "throughput" | "statistics") {
                extra.insert(key.clone(), item.clone());
            }
        }
    }

    Ok(Benchmark {
        id: id.clone(),
        group: None,
        label: Some(id.clone()),
        note: None,
        unit,
        value: measured,
        direction: Direction::Higher,
        params: Default::default(),
        metrics: Default::default(),
        stats: Stats {
            rme_pct: rme,
            ..Stats::default()
        },
        deterministic: false,
        extra,
    })
}

fn parse_csv(text: &str, name: &str) -> Result<Loaded, InputError> {
    let delimiter = if text.lines().next().is_some_and(|line| line.contains('\t')) {
        b'\t'
    } else {
        b','
    };
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|err| InputError::Parse {
            path: name.to_owned(),
            format: "csv",
            message: err.to_string(),
        })?
        .clone();
    let index_of = |candidates: &[&str]| -> Option<usize> {
        headers.iter().position(|header| {
            let header = header.trim().to_ascii_lowercase();
            candidates.iter().any(|candidate| header == *candidate)
        })
    };
    let id_col = index_of(&["id", "name", "benchmark"]);
    let value_col =
        index_of(&["value", "ops", "throughput", "mean"]).ok_or_else(|| InputError::Parse {
            path: name.to_owned(),
            format: "csv",
            message: format!(
                "missing a value column; headers were: {}",
                headers.iter().collect::<Vec<_>>().join(", ")
            ),
        })?;
    let label_col = index_of(&["label", "display"]);
    let note_col = index_of(&["note", "notes", "detail"]);
    let group_col = index_of(&["group", "section"]);
    let unit_col = index_of(&["unit", "units"]);
    let rme_col = index_of(&["rme_pct", "rme", "rme%", "margin"]);

    let mut loaded = Loaded::default();
    for (row_index, record) in reader.records().enumerate() {
        let record = record.map_err(|err| InputError::Parse {
            path: format!("{name}:{}", row_index + 2),
            format: "csv",
            message: err.to_string(),
        })?;
        let cell = |index: Option<usize>| -> Option<&str> {
            index
                .and_then(|index| record.get(index))
                .map(str::trim)
                .filter(|value| !value.is_empty())
        };
        let id = cell(id_col)
            .or_else(|| cell(label_col))
            .map(str::to_owned)
            .unwrap_or_else(|| format!("row-{}", row_index + 1));
        let value: f64 = cell(Some(value_col))
            .and_then(|raw| raw.parse().ok())
            .ok_or_else(|| InputError::Parse {
                path: format!("{name}:{}", row_index + 2),
                format: "csv",
                message: format!("`{}` is not a number", cell(Some(value_col)).unwrap_or("")),
            })?;
        loaded.benchmarks.push(Benchmark {
            label: cell(label_col).map(str::to_owned),
            note: cell(note_col).map(str::to_owned),
            group: cell(group_col).map(str::to_owned),
            unit: cell(unit_col).unwrap_or("ops/s").to_owned(),
            value,
            direction: Direction::Higher,
            stats: Stats {
                rme_pct: cell(rme_col).and_then(|raw| raw.parse().ok()),
                ..Stats::default()
            },
            id,
            params: Default::default(),
            metrics: Default::default(),
            deterministic: false,
            extra: Default::default(),
        });
    }
    Ok(loaded)
}

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

fn is_jit(benchmark: &Benchmark) -> bool {
    benchmark
        .extra
        .get("compiler")
        .and_then(Value::as_str)
        .is_some_and(|compiler| compiler.eq_ignore_ascii_case("jit"))
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
