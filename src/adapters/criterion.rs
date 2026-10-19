//! criterion `target/criterion` importer.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::model::{Benchmark, Direction, Stats};

pub fn load_criterion(root: &Path) -> Result<Vec<Benchmark>, String> {
    let mut estimates = Vec::new();
    walk(root, &mut estimates)?;
    // `read_dir` order differs per filesystem; the row order must not.
    estimates.sort();
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
pub(crate) fn parse_criterion(
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
