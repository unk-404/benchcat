//! Canonical benchmark data model shared by every input format and the renderer.

use std::collections::BTreeMap;

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
    pub params: BTreeMap<String, serde_json::Value>,
    /// Secondary numbers that must not drive the bar (`max_rss_bytes`, `instr`, ...).
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
    #[serde(default)]
    pub stats: Stats,
    /// Instruction-count style metrics: gate on a narrow tolerance, not on noise.
    #[serde(default)]
    pub deterministic: bool,
    /// Unknown fields preserved for templates and diagnostics.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
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

/// A whole canonical document (`bench-svg/report@1`).
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
            schema: Some("bench-svg/report@1".to_owned()),
            meta: Meta::default(),
            benchmarks,
        }
    }
}

/// Dimensional family of a unit, used for cross-tool normalisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnitKind {
    BytesPerSec,
    OpsPerSec,
    DurationPerOp,
    Count,
    Ratio,
    Raw,
}

/// A parsed unit plus the factor that converts a value to the SI base unit.
#[derive(Debug, Clone)]
pub struct Unit {
    pub symbol: String,
    pub kind: UnitKind,
    /// `base = value * scale`.
    pub scale: f64,
}

impl Unit {
    pub fn parse(symbol: &str) -> Self {
        let raw = symbol.trim();
        let lower = raw.to_ascii_lowercase().replace('µ', "u").replace(' ', "");
        let (kind, scale) = match lower.as_str() {
            "b/s" | "byte/s" | "bytes/s" => (UnitKind::BytesPerSec, 1.0),
            "kib/s" => (UnitKind::BytesPerSec, 1024.0),
            "mib/s" => (UnitKind::BytesPerSec, 1024.0 * 1024.0),
            "gib/s" => (UnitKind::BytesPerSec, 1024.0 * 1024.0 * 1024.0),
            "tib/s" => (UnitKind::BytesPerSec, 1024.0 * 1024.0 * 1024.0 * 1024.0),
            "kb/s" => (UnitKind::BytesPerSec, 1000.0),
            "mb/s" => (UnitKind::BytesPerSec, 1_000_000.0),
            "gb/s" => (UnitKind::BytesPerSec, 1_000_000_000.0),
            "ops/s" | "op/s" | "iops" | "req/s" | "requests/s" => (UnitKind::OpsPerSec, 1.0),
            "kops/s" | "kiops" => (UnitKind::OpsPerSec, 1_000.0),
            "mops/s" | "miops" => (UnitKind::OpsPerSec, 1_000_000.0),
            "ns/op" | "ns" | "nanos/op" => (UnitKind::DurationPerOp, 1e-9),
            "us/op" | "us" | "micros/op" => (UnitKind::DurationPerOp, 1e-6),
            "ms/op" | "ms" => (UnitKind::DurationPerOp, 1e-3),
            "s/op" | "s" | "sec/op" => (UnitKind::DurationPerOp, 1.0),
            "instr" | "instructions" | "count" | "ops" => (UnitKind::Count, 1.0),
            "x" | "ratio" => (UnitKind::Ratio, 1.0),
            _ => (UnitKind::Raw, 1.0),
        };
        Self {
            symbol: raw.to_owned(),
            kind,
            scale,
        }
    }

    pub fn base(&self, value: f64) -> f64 {
        value * self.scale
    }

    /// Format a value written in this unit.
    pub fn format(&self, value: f64) -> String {
        format_base(self.kind, self.base(value), self.symbol.as_str())
    }
}

/// Format a value already expressed in the SI base unit of `kind`.
pub fn format_base(kind: UnitKind, base: f64, raw_symbol: &str) -> String {
    match kind {
        UnitKind::BytesPerSec => {
            const KIB: f64 = 1024.0;
            const MIB: f64 = 1024.0 * 1024.0;
            const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
            const TIB: f64 = 1024.0 * 1024.0 * 1024.0 * 1024.0;
            if base >= TIB {
                format!("{:.2} TiB/s", base / TIB)
            } else if base >= GIB {
                format!("{:.2} GiB/s", base / GIB)
            } else if base >= MIB {
                format!("{:.2} MiB/s", base / MIB)
            } else if base >= KIB {
                format!("{:.1} KiB/s", base / KIB)
            } else {
                format!("{base:.0} B/s")
            }
        }
        UnitKind::OpsPerSec => {
            let suffix = if raw_symbol.to_ascii_lowercase().contains("iops") {
                "IOPS"
            } else {
                "ops/s"
            };
            if base >= 1e9 {
                format!("{:.2}G {suffix}", base / 1e9)
            } else if base >= 1e6 {
                format!("{:.2}M {suffix}", base / 1e6)
            } else if base >= 1e3 {
                format!("{:.1}k {suffix}", base / 1e3)
            } else {
                format!("{base:.0} {suffix}")
            }
        }
        UnitKind::DurationPerOp => {
            if base >= 1.0 {
                format!("{base:.2} s")
            } else if base >= 1e-3 {
                format!("{:.2} ms", base * 1e3)
            } else if base >= 1e-6 {
                format!("{:.2} µs", base * 1e6)
            } else {
                format!("{:.0} ns", base * 1e9)
            }
        }
        UnitKind::Count => {
            if base >= 1e9 {
                if raw_symbol.is_empty() {
                    format!("{:.2}G", base / 1e9)
                } else {
                    format!("{:.2}G {raw_symbol}", base / 1e9)
                }
            } else if base >= 1e6 {
                if raw_symbol.is_empty() {
                    format!("{:.2}M", base / 1e6)
                } else {
                    format!("{:.2}M {raw_symbol}", base / 1e6)
                }
            } else if base >= 1e3 {
                if raw_symbol.is_empty() {
                    format!("{:.1}k", base / 1e3)
                } else {
                    format!("{:.1}k {raw_symbol}", base / 1e3)
                }
            } else if raw_symbol.is_empty() {
                format!("{base:.0}")
            } else {
                format!("{base:.0} {raw_symbol}")
            }
        }
        UnitKind::Ratio => format!("{base:.2}×"),
        UnitKind::Raw => {
            if raw_symbol.is_empty() {
                format!("{base:.2}")
            } else {
                format!("{base:.2} {raw_symbol}")
            }
        }
    }
}

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
