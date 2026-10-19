//! Importers for third-party benchmark outputs.
//!
//! Both loaders map into the canonical [`Benchmark`] model; the renderer never
//! knows which tool produced a number.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::model::{Benchmark, Direction, Stats};

/// Walk `root` for criterion's `new/estimates.json` files and convert them.
///
/// Criterion's JSON is documented as a private implementation detail, so every
/// field is read defensively: a missing key drops the row instead of failing
/// the whole run, and the caller reports how many records were skipped.
pub fn load_criterion(root: &Path) -> Result<Vec<Benchmark>, String> {
    let mut estimates = Vec::new();
    walk(root, &mut estimates)?;
    let mut out = Vec::new();
    for path in estimates {
        let text = std::fs::read_to_string(&path)
            .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        let value: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(_) => continue,
        };
        // `.../<bench>/new/estimates.json`
        let bench_dir = path
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| format!("unexpected criterion layout: {}", path.display()))?;
        let manifest = std::fs::read_to_string(bench_dir.join("benchmark.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok());
        let relative = bench_dir.strip_prefix(root).unwrap_or(bench_dir);
        if let Some(benchmark) = parse_criterion(&value, manifest.as_ref(), relative) {
            out.push(benchmark);
        }
    }
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|err| format!("cannot read {}: {err}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // `report` holds criterion's own SVGs/HTML; never ingest those.
            if path
                .file_name()
                .is_some_and(|name| name == "report" || name == "base" || name == "change")
            {
                continue;
            }
            walk(&path, out)?;
        } else if path
            .file_name()
            .is_some_and(|name| name == "estimates.json")
            && path
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name == "new" || name == "base")
        {
            out.push(path);
        }
    }
    Ok(())
}

/// `estimates.json` is nanoseconds per iteration, with confidence intervals.
fn parse_criterion(
    estimates: &Value,
    manifest: Option<&Value>,
    relative: &Path,
) -> Option<Benchmark> {
    let mean = estimates.get("mean")?;
    let point = mean.get("point_estimate")?.as_f64()?;
    if point <= 0.0 {
        return None;
    }
    let std_dev = estimates
        .get("std_dev")
        .and_then(|value| value.get("point_estimate"))
        .and_then(Value::as_f64);
    let median = estimates
        .get("median")
        .and_then(|value| value.get("point_estimate"))
        .and_then(Value::as_f64);
    let rme = mean.get("confidence_interval").and_then(|interval| {
        let low = interval.get("lower_bound")?.as_f64()?;
        let high = interval.get("upper_bound")?.as_f64()?;
        Some((high - low) / 2.0 / point * 100.0)
    });

    let manifest = manifest.cloned().unwrap_or(Value::Null);
    let full_id = manifest
        .get("full_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| {
            relative
                .to_string_lossy()
                .replace(['\\', '/'], "/")
                .trim_matches('/')
                .to_owned()
        });
    let function_id = manifest
        .get("function_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| full_id.rsplit('/').next().unwrap_or(&full_id).to_owned());
    let group_id = manifest
        .get("group_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let value_str = manifest
        .get("value_str")
        .and_then(Value::as_str)
        .map(str::to_owned);

    let throughput = manifest.get("throughput");
    let (unit, value, direction) = match throughput {
        Some(throughput) if throughput.get("Bytes").is_some() => {
            let bytes = throughput.get("Bytes")?.as_f64()?;
            ("B/s".to_owned(), bytes * 1e9 / point, Direction::Higher)
        }
        Some(throughput) if throughput.get("Elements").is_some() => {
            let elements = throughput.get("Elements")?.as_f64()?;
            (
                "ops/s".to_owned(),
                elements * 1e9 / point,
                Direction::Higher,
            )
        }
        _ => ("ns/op".to_owned(), point, Direction::Lower),
    };

    let mut extra = std::collections::BTreeMap::new();
    extra.insert("tool".to_owned(), Value::String("criterion".to_owned()));
    if let Some(group) = &group_id {
        extra.insert("criterion_group".to_owned(), Value::String(group.clone()));
    }
    extra.insert(
        "criterion_full_id".to_owned(),
        Value::String(full_id.clone()),
    );

    Some(Benchmark {
        id: full_id,
        group: group_id,
        label: Some(function_id),
        note: value_str.or_else(|| Some(relative.to_string_lossy().replace(['\\', '/'], " / "))),
        unit,
        value,
        direction,
        params: Default::default(),
        metrics: Default::default(),
        stats: Stats {
            mean: Some(point),
            median,
            min: None,
            max: None,
            stddev: std_dev,
            samples: None,
            rme_pct: rme,
        },
        deterministic: false,
        extra,
    })
}

/// `hyperfine --export-json` v2 (and the legacy v1 shape).
pub fn parse_hyperfine(value: &Value) -> Result<Vec<Benchmark>, String> {
    let results = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| "hyperfine JSON has no `results` array".to_owned())?;
    let primary = value
        .get("metadata")
        .and_then(|metadata| metadata.get("primary_metric"))
        .and_then(Value::as_str)
        .unwrap_or("time_wall_clock");

    let mut out = Vec::new();
    for result in results {
        let command = result
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let name = result
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| command.clone());

        // v2 nests the numbers under `summary`, v1 puts them at the top level.
        let summary = result
            .get("summary")
            .and_then(|summary| summary.get(primary));
        let number = |key: &str| -> Option<f64> {
            summary
                .and_then(|summary| summary.get(key))
                .and_then(Value::as_f64)
                .or_else(|| result.get(key).and_then(Value::as_f64))
        };
        let Some(mean) = number("mean") else {
            continue;
        };

        let mut metrics = std::collections::BTreeMap::new();
        if let Some(peak) = summary
            .and_then(|summary| summary.get("max"))
            .and_then(Value::as_f64)
            .filter(|_| primary == "memory_peak_resident")
        {
            metrics.insert("peak_bytes".to_owned(), peak);
        }
        if let Some(peak) = result.get("memory_peak_resident").and_then(Value::as_f64) {
            metrics.insert("peak_bytes".to_owned(), peak);
        }

        let mut extra = std::collections::BTreeMap::new();
        extra.insert("tool".to_owned(), Value::String("hyperfine".to_owned()));
        if !command.is_empty() {
            extra.insert("command".to_owned(), Value::String(command.clone()));
        }
        let note = if command.is_empty() || name == command {
            None
        } else {
            Some(command)
        };

        out.push(Benchmark {
            id: name.clone(),
            group: None,
            label: Some(name),
            note,
            unit: "s".to_owned(),
            value: mean,
            direction: Direction::Lower,
            params: Default::default(),
            metrics,
            stats: Stats {
                mean: Some(mean),
                median: number("median"),
                min: number("min"),
                max: number("max"),
                stddev: number("stddev"),
                samples: summary
                    .and_then(|summary| summary.get("count"))
                    .and_then(Value::as_u64)
                    .map(|count| count as u32),
                rme_pct: None,
            },
            deterministic: false,
            extra,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn criterion_time_benchmark_is_ns_per_op() {
        let estimates: Value = serde_json::from_str(
            r#"{
                "mean": {
                    "confidence_interval": {"confidence_level": 0.95, "lower_bound": 980.0, "upper_bound": 1020.0},
                    "point_estimate": 1000.0,
                    "standard_error": 10.0
                },
                "std_dev": {"point_estimate": 40.0},
                "median": {"point_estimate": 995.0}
            }"#,
        )
        .unwrap();
        let manifest: Value = serde_json::from_str(
            r#"{"group_id":"io","function_id":"read","full_id":"io/read","throughput":null}"#,
        )
        .unwrap();
        let benchmark = parse_criterion(&estimates, Some(&manifest), Path::new("io/read")).unwrap();
        assert_eq!(benchmark.unit, "ns/op");
        assert_eq!(benchmark.direction, Direction::Lower);
        assert_eq!(benchmark.id, "io/read");
        assert_eq!(benchmark.label.as_deref(), Some("read"));
        assert!((benchmark.stats.rme_pct.unwrap() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn criterion_throughput_becomes_bytes_per_second() {
        let estimates: Value = serde_json::from_str(
            r#"{"mean":{"point_estimate":2000000.0,"confidence_interval":{"lower_bound":1900000.0,"upper_bound":2100000.0}}}"#,
        )
        .unwrap();
        let manifest: Value = serde_json::from_str(
            r#"{"group_id":"io","function_id":"read","full_id":"io/read","throughput":{"Bytes":8388608}}"#,
        )
        .unwrap();
        let benchmark = parse_criterion(&estimates, Some(&manifest), Path::new("io/read")).unwrap();
        assert_eq!(benchmark.unit, "B/s");
        assert_eq!(benchmark.direction, Direction::Higher);
        // 8 MiB in 2 ms => ~4.19 GB/s
        assert!((benchmark.value - 4_194_304_000.0).abs() < 1.0);
    }

    #[test]
    fn hyperfine_v2_uses_summary_mean() {
        let value: Value = serde_json::from_str(
            r#"{
                "schema_version": 2,
                "metadata": {"primary_metric": "time_wall_clock"},
                "results": [{
                    "command": "rg pattern",
                    "name": "search",
                    "summary": {
                        "time_wall_clock": {"unit": "second", "count": 10, "mean": 0.042, "stddev": 0.002, "median": 0.041, "min": 0.039, "max": 0.048}
                    }
                }]
            }"#,
        )
        .unwrap();
        let rows = parse_hyperfine(&value).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "search");
        assert_eq!(rows[0].unit, "s");
        assert_eq!(rows[0].stats.samples, Some(10));
        assert!((rows[0].value - 0.042).abs() < 1e-12);
    }

    #[test]
    fn hyperfine_v1_flat_shape_still_parses() {
        let value: Value = serde_json::from_str(
            r#"{"results":[{"command":"sleep 0.01","mean":0.011,"stddev":0.0004,"median":0.011,"min":0.010,"max":0.013}]}"#,
        )
        .unwrap();
        let rows = parse_hyperfine(&value).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "sleep 0.01");
        assert!((rows[0].value - 0.011).abs() < 1e-12);
    }
}
