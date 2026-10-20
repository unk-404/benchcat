//! Units: parsing, SI base normalisation and display formatting.

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
