//! Document parsers for canonical JSON, JSONL, benchmark_test and CSV.

use rayon::prelude::*;
use serde_json::Value;

use crate::model::{Benchmark, Direction, Report, Stats};

use super::{InputError, Loaded};

pub(super) fn parse_json(text: &str, name: &str) -> Result<Loaded, InputError> {
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

const PARALLEL_LINES: usize = 2_000;
const LINE_CHUNK: usize = 1_024;

pub(super) fn parse_jsonl(text: &str, name: &str) -> Result<Loaded, InputError> {
    let lines: Vec<&str> = text.lines().collect();
    let benchmarks = if lines.len() >= PARALLEL_LINES {
        // Long files dominate load time; parse line blocks in parallel and keep
        // block/line order so the result is identical to the sequential pass.
        let chunks: Result<Vec<Vec<Benchmark>>, InputError> = lines
            .par_chunks(LINE_CHUNK)
            .enumerate()
            .map(|(chunk, block)| {
                let mut out = Vec::with_capacity(block.len());
                for (offset, line) in block.iter().enumerate() {
                    let index = chunk * LINE_CHUNK + offset + 1;
                    if let Some(benchmark) = jsonl_line(line, name, index)? {
                        out.push(benchmark);
                    }
                }
                Ok(out)
            })
            .collect();
        chunks?.into_iter().flatten().collect()
    } else {
        let mut out = Vec::with_capacity(lines.len());
        for (index, line) in lines.iter().enumerate() {
            if let Some(benchmark) = jsonl_line(line, name, index + 1)? {
                out.push(benchmark);
            }
        }
        out
    };
    Ok(Loaded {
        benchmarks,
        ..Loaded::default()
    })
}

fn jsonl_line(line: &str, name: &str, index: usize) -> Result<Option<Benchmark>, InputError> {
    let line = line.trim();
    if !line.starts_with('{') {
        return Ok(None); // benchmark_test interleaves progress output with JSON.
    }
    // Canonical records go straight into `Benchmark`; only benchmark_test needs
    // the `Value` detour for `name`/`throughput`/`statistics`.
    if line.contains("\"throughput\"") {
        let value: Value = serde_json::from_str(line).map_err(|err| InputError::Parse {
            path: format!("{name}:{index}"),
            format: "jsonl",
            message: err.to_string(),
        })?;
        return benchmark_test_entry(&value, &format!("{name}:{index}")).map(Some);
    }
    serde_json::from_str::<Benchmark>(line)
        .map(Some)
        .map_err(|err| InputError::Parse {
            path: format!("{name}:{index}"),
            format: "jsonl",
            message: err.to_string(),
        })
}

pub(super) fn parse_benchmark_test(text: &str, name: &str) -> Result<Loaded, InputError> {
    let lines: Vec<&str> = text.lines().collect();
    let benchmarks = if lines.len() >= PARALLEL_LINES {
        let chunks: Result<Vec<Vec<Benchmark>>, InputError> = lines
            .par_chunks(LINE_CHUNK)
            .enumerate()
            .map(|(chunk, block)| {
                let mut out = Vec::with_capacity(block.len());
                for (offset, line) in block.iter().enumerate() {
                    let index = chunk * LINE_CHUNK + offset + 1;
                    if let Some(benchmark) = benchmark_test_line(line, name, index)? {
                        out.push(benchmark);
                    }
                }
                Ok(out)
            })
            .collect();
        chunks?.into_iter().flatten().collect()
    } else {
        let mut out = Vec::with_capacity(lines.len());
        for (index, line) in lines.iter().enumerate() {
            if let Some(benchmark) = benchmark_test_line(line, name, index + 1)? {
                out.push(benchmark);
            }
        }
        out
    };
    let mut loaded = Loaded {
        benchmarks,
        ..Loaded::default()
    };
    loaded.meta.tool = Some("benchmark_test".to_owned());
    Ok(loaded)
}

fn benchmark_test_line(
    line: &str,
    name: &str,
    index: usize,
) -> Result<Option<Benchmark>, InputError> {
    let line = line.trim();
    if !line.starts_with('{') {
        return Ok(None);
    }
    let value: Value = serde_json::from_str(line).map_err(|err| InputError::Parse {
        path: format!("{name}:{index}"),
        format: "benchmark_test",
        message: err.to_string(),
    })?;
    benchmark_test_entry(&value, &format!("{name}:{index}")).map(Some)
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

    let mut extra = crate::small_map::SmallMap::new();
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

pub(super) fn parse_csv(text: &str, name: &str) -> Result<Loaded, InputError> {
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

pub(super) fn is_jit(benchmark: &Benchmark) -> bool {
    benchmark
        .extra
        .get("compiler")
        .and_then(Value::as_str)
        .is_some_and(|compiler| compiler.eq_ignore_ascii_case("jit"))
}
