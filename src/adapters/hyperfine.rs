//! hyperfine `--export-json` importer.

use serde_json::Value;

use crate::model::{Benchmark, Direction, Stats};

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
