//! Renderer configuration (`bench-svg.toml`).

use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub render: Render,
    /// Typeface selection; `family` resolves against `./fonts`.
    pub font: FontSettings,
    /// Explicit row mapping; rules are applied in order. Written as `[[row]]`.
    #[serde(rename = "row")]
    pub rows: Vec<RowRule>,
    /// Extra measurement sources; two or more turn on the comparison layout.
    pub series: Vec<Series>,
}

/// One `[[row]]` entry: match a benchmark, then rename, re-unit or rescale it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RowRule {
    /// Exact benchmark id.
    #[serde(default)]
    pub id: Option<String>,
    /// Regex matched against the id; groups become `$1`… in templates.
    #[serde(default, rename = "match")]
    pub pattern: Option<String>,
    /// Display label template, e.g. `"Pool read · $1 workers"`.
    #[serde(default)]
    pub label: Option<String>,
    /// Note template; an empty string clears the note.
    #[serde(default)]
    pub note: Option<String>,
    /// Unit override, applied after the value expression.
    #[serde(default)]
    pub unit: Option<String>,
    /// Arithmetic expression over `value` and `$1`…, e.g. `"value * $1 * 32"`.
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FontSettings {
    /// Font family name, i.e. the file stem inside `./fonts`.
    pub family: Option<String>,
}

/// One comparison source: a platform, a language, a branch, ...
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Series {
    /// Legend text, for example `Linux x64`.
    pub label: String,
    /// File or directory to load.
    pub input: String,
    /// Optional per-source format override.
    #[serde(default)]
    pub format: Option<String>,
    /// Optional per-source bar colour.
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Render {
    /// Large heading; defaults to `benchmarks`.
    pub title: Option<String>,
    /// Small line under the heading; defaults to provenance.
    pub subtitle: Option<String>,
    /// Footer line; defaults to a generated summary.
    pub footer: Option<String>,
    /// Canvas width in pixels.
    pub width: f32,
    /// Keep the card at least this tall, for consistent side-by-side cards.
    pub min_height: f32,
    /// Height of one row card.
    pub card_height: f32,
    /// Vertical gap between row cards.
    pub row_gap: f32,
    /// Inner padding of the card.
    pub padding: f32,
    /// Visual theme name.
    pub theme: String,
    /// Decorative kaomoji; empty string removes it.
    pub kaomoji: String,
    /// Only render the first N rows.
    pub max_rows: Option<usize>,
    /// Drop benchmarks that no `[[row]]` rule matched.
    pub only_matched: bool,
}

impl Default for Render {
    fn default() -> Self {
        Self {
            title: None,
            subtitle: None,
            footer: None,
            width: 920.0,
            min_height: 0.0,
            card_height: 38.0,
            row_gap: 10.0,
            padding: 22.0,
            theme: "pastel".to_owned(),
            kaomoji: "(◕‿◕)♡".to_owned(),
            max_rows: None,
            only_matched: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read config {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("invalid config {path}: {source}")]
    Toml {
        path: String,
        source: toml::de::Error,
    },
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.display().to_string(),
            source,
        })?;
        toml::from_str(&text).map_err(|source| ConfigError::Toml {
            path: path.display().to_string(),
            source,
        })
    }

    /// Load `path` when given, otherwise the conventional `bench-svg.toml`.
    pub fn discover(path: Option<&Path>) -> Result<Option<Self>, ConfigError> {
        if let Some(path) = path {
            return Self::load(path).map(Some);
        }
        let conventional = Path::new("bench-svg.toml");
        if conventional.is_file() {
            return Self::load(conventional).map(Some);
        }
        Ok(None)
    }
}
