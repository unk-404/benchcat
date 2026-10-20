//! Renderer configuration (`benchcat.toml`).

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
    /// Section the row belongs to; rows with the same group share a heading.
    #[serde(default)]
    pub group: Option<String>,
}

/// One or more family stems, in fallback order.
///
/// Accepts a single string (`"quicksand"`), a comma-separated string
/// (`"quicksand, zcool-kuaile"`) or a list (`["quicksand", "zcool-kuaile"]`),
/// so existing single-family configs keep working unchanged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FontFamilies(pub Vec<String>);

impl FontFamilies {
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'de> Deserialize<'de> for FontFamilies {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            One(String),
            Many(Vec<String>),
        }

        let raw = Raw::deserialize(deserializer)?;
        let values = match raw {
            Raw::One(text) => split_families(&text),
            Raw::Many(items) => items.iter().flat_map(|item| split_families(item)).collect(),
        };
        Ok(Self(values))
    }
}

fn split_families(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FontSettings {
    /// Family stems in fallback order: string, comma-separated string or list.
    pub family: Option<FontFamilies>,
    /// Try the fixed guaranteed-font list when the configured face is missing,
    /// unreadable, or lacks glyphs. Off by default so existing cards stay
    /// byte-identical.
    pub enable_guaranteed_font: bool,
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
    /// Multi-source bar reference: the row (default), each source, or the whole card.
    pub bar_scale: BarScale,
    /// Multi-source bar shape: `linear` keeps ratios honest, `log` compresses ranges.
    pub bar_curve: BarCurve,
    /// Multi-source rows that lack a source: skip the entry, or draw an empty track.
    pub missing: Missing,
}

/// Where a multi-source row's bars take their 100% from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BarScale {
    /// Best value inside the row (default).
    #[default]
    Row,
    /// Best value of each source across all rows.
    Source,
    /// Best value of the whole card per unit family.
    Global,
}

/// How a value becomes a bar length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BarCurve {
    /// `value / reference`; differences stay visible.
    #[default]
    Linear,
    /// `log1p(value) / log1p(reference)`; compresses wide ranges.
    Log,
}

/// Rendering choice for a source absent from a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Missing {
    /// Draw nothing; the row shrinks to its present bars.
    #[default]
    Skip,
    /// Draw an empty track and an em dash.
    Placeholder,
}

impl Default for Render {
    fn default() -> Self {
        Self {
            title: None,
            subtitle: None,
            footer: None,
            width: 920.0,
            min_height: 0.0,
            card_height: 42.0,
            row_gap: 10.0,
            padding: 22.0,
            theme: "pastel".to_owned(),
            kaomoji: "(◕‿◕)♡".to_owned(),
            max_rows: None,
            only_matched: false,
            bar_scale: BarScale::Row,
            bar_curve: BarCurve::Linear,
            missing: Missing::Skip,
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
    #[error("invalid config {path}: {message}")]
    Invalid { path: String, message: String },
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let display = path.display().to_string();
        // Layered sources: the TOML file, then `BENCHCAT_*` environment
        // overrides (`BENCHCAT_RENDER__TITLE`, `BENCHCAT_FONT__FAMILY`, ...).
        // Only the `toml` feature is enabled: json/yaml/ini/ron/json5, async
        // and case conversion are not used by this tool.
        let settings = config::Config::builder()
            .add_source(config::File::from(path.to_path_buf()))
            .add_source(
                config::Environment::with_prefix("BENCHCAT")
                    .prefix_separator("_")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()
            .map_err(|err| ConfigError::Invalid {
                path: display.clone(),
                message: err.to_string(),
            })?;
        settings
            .try_deserialize()
            .map_err(|err| ConfigError::Invalid {
                path: display,
                message: err.to_string(),
            })
    }

    /// Load `path` when given, otherwise the conventional `benchcat.toml`.
    pub fn discover(path: Option<&Path>) -> Result<Option<Self>, ConfigError> {
        if let Some(path) = path {
            return Self::load(path).map(Some);
        }
        let conventional = Path::new("benchcat.toml");
        if conventional.is_file() {
            return Self::load(conventional).map(Some);
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn font_family_accepts_string_comma_and_list() {
        let one: Config = serde_json::from_str(r#"{"font":{"family":"quicksand"}}"#).unwrap();
        assert_eq!(one.font.family.unwrap().0, ["quicksand"]);

        let comma: Config =
            serde_json::from_str(r#"{"font":{"family":"quicksand, zcool-kuaile"}}"#).unwrap();
        assert_eq!(comma.font.family.unwrap().0, ["quicksand", "zcool-kuaile"]);

        let list: Config =
            serde_json::from_str(r#"{"font":{"family":["quicksand","zcool-kuaile"]}}"#).unwrap();
        assert_eq!(list.font.family.unwrap().0, ["quicksand", "zcool-kuaile"]);
        assert!(!list.font.enable_guaranteed_font);
    }

    #[test]
    fn guaranteed_font_is_opt_in_but_parses() {
        let default: Config = serde_json::from_str("{}").unwrap();
        assert!(!default.font.enable_guaranteed_font);

        let on: Config =
            serde_json::from_str(r#"{"font":{"enable_guaranteed_font":true}}"#).unwrap();
        assert!(on.font.enable_guaranteed_font);
    }
}
