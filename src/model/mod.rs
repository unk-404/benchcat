//! Canonical benchmark data model shared by every input format and the renderer.

use crate::small_map::SmallMap;

use serde::{Deserialize, Serialize};

/// Whether a larger or smaller value represents better performance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    #[default]
    #[serde(alias = "higher-is-better", alias = "higher_is_better", alias = "max")]
    Higher,
    #[serde(alias = "lower-is-better", alias = "lower_is_better", alias = "min")]
    Lower,
}

/// Distribution statistics captured by the benchmark tool.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Stats {
    #[serde(default)]
    pub mean: Option<f64>,
    #[serde(default)]
    pub median: Option<f64>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub stddev: Option<f64>,
    #[serde(default)]
    pub samples: Option<u32>,
    #[serde(
        default,
        alias = "relative_margin_of_error",
        alias = "relativeMarginOfError",
        alias = "rme"
    )]
    pub rme_pct: Option<f64>,
}

/// One measured operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Benchmark {
    /// Stable machine key used to line up runs, platforms and sources.
    pub id: String,
    /// Optional section heading; rows keep their input order within a group.
    #[serde(default)]
    pub group: Option<String>,
    /// Display name; defaults to [`Benchmark::id`].
    #[serde(default)]
    pub label: Option<String>,
    /// Secondary one-line annotation.
    #[serde(default)]
    pub note: Option<String>,
    /// Unit as written by the producing tool, for example `B/s` or `ns/op`.
    #[serde(default = "default_unit")]
    pub unit: String,
    /// Value expressed in [`Benchmark::unit`].
    pub value: f64,
    #[serde(default)]
    pub direction: Direction,
    /// Structured parameters (`workers`, `batch`, `block_size`, ...).
    #[serde(default)]
    pub params: SmallMap<String, serde_json::Value>,
    /// Secondary numbers that must not drive the bar (`max_rss_bytes`, `instr`, ...).
    #[serde(default)]
    pub metrics: SmallMap<String, f64>,
    #[serde(default)]
    pub stats: Stats,
    /// Instruction-count style metrics: gate on a narrow tolerance, not on noise.
    #[serde(default)]
    pub deterministic: bool,
    /// Unknown fields preserved for templates and diagnostics.
    #[serde(flatten)]
    pub extra: SmallMap<String, serde_json::Value>,
}

fn default_unit() -> String {
    "ops/s".to_owned()
}

impl Benchmark {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.id)
    }

    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }
}

/// Provenance for the whole report.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meta {
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default, alias = "toolVersion")]
    pub tool_version: Option<String>,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub platform: Platform,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Platform {
    #[serde(default)]
    pub os: Option<String>,
    #[serde(default, alias = "osVersion")]
    pub os_version: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
    #[serde(default)]
    pub cpu: Option<String>,
    #[serde(default)]
    pub cores: Option<u32>,
    #[serde(default, alias = "ramGiB", alias = "ram_gib")]
    pub ram_gib: Option<u32>,
    #[serde(default)]
    pub runner: Option<String>,
}

impl Platform {
    /// Short human readable label, for example `Windows · x86_64`.
    pub fn summary(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(os) = &self.os {
            parts.push(os.clone());
        }
        if let Some(arch) = &self.arch {
            parts.push(arch.clone());
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" · "))
        }
    }
}

/// A whole canonical document (`benchcat/report@1`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub meta: Meta,
    #[serde(default)]
    pub benchmarks: Vec<Benchmark>,
}

impl Report {
    pub fn new(benchmarks: Vec<Benchmark>) -> Self {
        Self {
            schema: Some("benchcat/report@1".to_owned()),
            meta: Meta::default(),
            benchmarks,
        }
    }
}

mod unit;

pub use unit::{Unit, UnitKind, format_base};

/// Human readable rendering of a JSON scalar, for `params` templates.
pub fn json_scalar(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_units_and_normalises_to_base() {
        assert_eq!(Unit::parse("MiB/s").base(2.0), 2.0 * 1024.0 * 1024.0);
        assert!((Unit::parse("ns/op").base(1000.0) - 1e-6).abs() < 1e-18);
        assert_eq!(Unit::parse("IOPS").kind, UnitKind::OpsPerSec);
    }

    #[test]
    fn formats_bytes_per_second() {
        let unit = Unit::parse("B/s");
        assert_eq!(unit.format(2.0 * 1024.0 * 1024.0 * 1024.0), "2.00 GiB/s");
    }

    #[test]
    fn formats_duration_per_op() {
        let unit = Unit::parse("ns/op");
        assert_eq!(unit.format(1_240.0), "1.24 µs");
    }
}
