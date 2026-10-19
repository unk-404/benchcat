//! Orchestration: canonical models in, one SVG string out.

use std::collections::HashMap;
use std::path::Path;

use crate::config::{Config, RowRule};
use crate::font::Font;
use crate::input::params_note;
use crate::layout::{LayoutOptions, LegendItem, RowPlan, SeriesValue, layout};
use crate::model::{Benchmark, Direction, Meta, Unit, UnitKind};
use crate::rows::{self, Mapped};
use crate::svg::{self, RenderInput};
use crate::template::{self, Context as TemplateContext};

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error(transparent)]
    Layout(#[from] crate::layout::LayoutError),
    #[error("theme {0:?} is unknown; expected \"pastel\" or \"dark\"")]
    UnknownTheme(String),
    #[error("{0}")]
    Rule(String),
}

pub struct Rendered {
    pub svg: String,
    pub warnings: Vec<String>,
    pub rows: usize,
}

/// One source of measurements: a platform, a language, a branch, ...
pub struct SeriesInput<'a> {
    pub label: Option<String>,
    pub color: Option<String>,
    pub benchmarks: &'a [Benchmark],
}

pub struct RenderRequest<'a> {
    pub config: &'a Config,
    pub meta: &'a Meta,
    pub series: &'a [SeriesInput<'a>],
    pub font: &'a Font,
}

pub fn render(request: RenderRequest<'_>) -> Result<Rendered, RenderError> {
    let render = &request.config.render;
    let theme =
        svg::theme(&render.theme).ok_or_else(|| RenderError::UnknownTheme(render.theme.clone()))?;
    let multi_series = request.series.len() > 1;

    let legend: Vec<LegendItem> = if multi_series {
        request
            .series
            .iter()
            .enumerate()
            .map(|(index, series)| LegendItem {
                label: series
                    .label
                    .clone()
                    .unwrap_or_else(|| format!("source {}", index + 1)),
                color: series.color.clone().unwrap_or_else(|| {
                    theme.series_colors[index % theme.series_colors.len()].to_owned()
                }),
            })
            .collect()
    } else {
        Vec::new()
    };

    let rows = merge_rows(request.series, multi_series, request.config)?;
    let rows = match render.max_rows {
        Some(limit) if rows.len() > limit => &rows[..limit],
        _ => &rows[..],
    };

    // `{placeholder}` expansion keeps provenance out of the committed config.
    let context = TemplateContext {
        meta: request.meta,
        rows: rows.len(),
        font: &request.font.family,
    };
    let mut warnings: Vec<String> = Vec::new();
    let title_source = render
        .title
        .clone()
        .unwrap_or_else(|| "benchmarks".to_owned());
    let (title, title_warnings) = template::expand(&title_source, &context);
    warnings.extend(title_warnings);
    let subtitle_source = render
        .subtitle
        .clone()
        .unwrap_or_else(|| default_subtitle(request.meta));
    let (subtitle, subtitle_warnings) = template::expand(&subtitle_source, &context);
    warnings.extend(subtitle_warnings);
    let footer_source = render
        .footer
        .clone()
        .unwrap_or_else(|| default_footer(request.meta, rows.len()));
    let (footer, footer_warnings) = template::expand(&footer_source, &context);
    warnings.extend(footer_warnings);

    let options = LayoutOptions {
        width: render.width,
        min_height: render.min_height,
        card_height: render.card_height,
        row_gap: render.row_gap,
        padding: render.padding,
        title: title.clone(),
        subtitle: subtitle.clone(),
        footer: footer.clone(),
        legend,
    };
    let plan = layout(request.font, rows, &options)?;
    let output = svg::render(
        request.font,
        &plan,
        &RenderInput {
            title: &title,
            subtitle: &subtitle,
            footer: &footer,
            kaomoji: &render.kaomoji,
        },
        theme,
    );

    Ok(Rendered {
        svg: output.svg,
        warnings: {
            warnings.extend(output.warnings);
            warnings
        },
        rows: rows.len(),
    })
}

#[derive(Clone)]
struct Slot {
    text: String,
    base: f64,
    rme: Option<f64>,
    kind: UnitKind,
    direction: Direction,
}

/// Label, note and section for a row, as first seen across sources.
type RowMeta = (Option<String>, Option<String>, Option<String>);

/// Union of benchmark ids across sources, in first-seen order, with one slot
/// per source. Missing rows stay `None` so they render as an empty track.
fn merge_rows(
    series: &[SeriesInput<'_>],
    multi_series: bool,
    config: &Config,
) -> Result<Vec<RowPlan>, RenderError> {
    let rules = &config.rows;
    let mut order: Vec<String> = Vec::new();
    let mut slots: HashMap<String, Vec<Option<Slot>>> = HashMap::new();
    let mut labels: HashMap<String, RowMeta> = HashMap::new();
    let mut keys: HashMap<String, usize> = HashMap::new();

    for (index, source) in series.iter().enumerate() {
        for benchmark in source.benchmarks {
            let (mapped, rule_index) = map_benchmark(benchmark, rules)?;
            let entry = slots.entry(benchmark.id.clone()).or_insert_with(|| {
                order.push(benchmark.id.clone());
                keys.insert(benchmark.id.clone(), rule_index);
                vec![None; series.len()]
            });
            let unit = Unit::parse(&mapped.unit);
            entry[index] = Some(Slot {
                text: unit.format(mapped.value),
                base: unit.base(mapped.value).max(0.0),
                rme: benchmark.stats.rme_pct,
                kind: unit.kind,
                direction: benchmark.direction,
            });
            let label = mapped.label.clone().or_else(|| benchmark.label.clone());
            let note = match mapped.note.clone() {
                Some(text) if text.is_empty() => None,
                Some(text) => Some(text),
                None => benchmark.note.clone().or_else(|| params_note(benchmark)),
            };
            labels
                .entry(benchmark.id.clone())
                .or_insert((label, note, mapped.group.clone()));
        }
    }

    // Bars are only comparable inside one dimensional family (bytes/s with
    // bytes/s, ns/op with ns/op). Each row is scaled against the best value of
    // its own family, and "lower is better" metrics are inverted so a longer
    // bar always means better performance.
    let mut best: HashMap<UnitKind, f64> = HashMap::new();
    for entries in slots.values() {
        for slot in entries.iter().flatten() {
            let candidate = best.entry(slot.kind).or_insert(slot.base);
            match slot.direction {
                Direction::Higher => *candidate = candidate.max(slot.base),
                Direction::Lower => *candidate = candidate.min(slot.base),
            }
        }
    }

    let mut rows: Vec<(usize, RowPlan)> = Vec::with_capacity(order.len());
    for id in order {
        let key = keys.get(&id).copied().unwrap_or(rules.len());
        let Some(entries) = slots.remove(&id) else {
            continue;
        };
        let values: Vec<SeriesValue> = entries
            .iter()
            .map(|slot| match slot {
                Some(slot) => {
                    let reference = if multi_series {
                        entries
                            .iter()
                            .flatten()
                            .filter(|other| other.kind == slot.kind)
                            .map(|other| other.base)
                            .fold(None::<f64>, |acc, value| {
                                Some(match (acc, slot.direction) {
                                    (None, Direction::Higher) => value,
                                    (None, Direction::Lower) => value,
                                    (Some(acc), Direction::Higher) => acc.max(value),
                                    (Some(acc), Direction::Lower) => acc.min(value),
                                })
                            })
                            .unwrap_or(slot.base)
                    } else {
                        *best.get(&slot.kind).unwrap_or(&slot.base)
                    };
                    SeriesValue {
                        text: slot.text.clone(),
                        fraction: bar_fraction(slot.base, reference, slot.direction),
                    }
                }
                None => SeriesValue {
                    text: "—".to_owned(),
                    fraction: 0.0,
                },
            })
            .collect();
        let (label, note, group) = labels.remove(&id).unwrap_or((None, None, None));
        let tooltip = {
            let mut parts = vec![label.clone().unwrap_or_else(|| id.clone())];
            for (index, slot) in entries.iter().enumerate() {
                if let Some(slot) = slot {
                    let name = series
                        .get(index)
                        .and_then(|source| source.label.clone())
                        .unwrap_or_else(|| format!("source {}", index + 1));
                    parts.push(format!("{name} {}", slot.text));
                }
            }
            parts.join(" · ")
        };
        rows.push((
            key,
            RowPlan {
                label: label.unwrap_or_else(|| id.clone()),
                note,
                group,
                values,
                rme_text: if multi_series {
                    None
                } else {
                    entries
                        .first()
                        .and_then(|slot| slot.as_ref())
                        .and_then(|slot| slot.rme)
                        .map(|rme| format!("±{rme:.2}%"))
                },
                tooltip,
            },
        ));
    }

    // Rules define the row order; unmatched rows keep their input order at the
    // end unless `only_matched` drops them.
    rows.sort_by_key(|(key, _)| *key);
    let rows = rows
        .into_iter()
        .filter(|(key, _)| {
            !(config.render.only_matched && !rules.is_empty() && *key >= rules.len())
        })
        .map(|(_, row)| row)
        .collect();
    Ok(rows)
}

fn map_benchmark(benchmark: &Benchmark, rules: &[RowRule]) -> Result<(Mapped, usize), RenderError> {
    for (index, rule) in rules.iter().enumerate() {
        if let Some(captures) = rows::match_rule(rule, benchmark).map_err(RenderError::Rule)? {
            let mapped = rows::apply(rule, benchmark, &captures).map_err(RenderError::Rule)?;
            return Ok((mapped, index));
        }
    }
    Ok((
        Mapped {
            label: benchmark.label.clone(),
            note: benchmark.note.clone(),
            unit: benchmark.unit.clone(),
            value: benchmark.value,
            group: benchmark.group.clone(),
        },
        rules.len(),
    ))
}

/// Log-compressed 0..1 bar length relative to `reference`.
fn bar_fraction(base: f64, reference: f64, direction: Direction) -> f32 {
    if base <= 0.0 || reference <= 0.0 {
        return 0.0;
    }
    let fraction = match direction {
        Direction::Higher => libm::log1p(base) / libm::log1p(reference),
        // Lower is better: the best (smallest) value gets the longest bar.
        Direction::Lower => libm::log1p(reference) / libm::log1p(base),
    };
    fraction.clamp(0.0, 1.0) as f32
}

fn default_subtitle(meta: &Meta) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(platform) = meta.platform.summary() {
        parts.push(platform);
    }
    if let Some(tool) = &meta.tool {
        let mut label = tool.clone();
        if let Some(version) = &meta.tool_version {
            label.push(' ');
            label.push_str(version);
        }
        parts.push(label);
    }
    if let Some(commit) = &meta.commit {
        parts.push(format!("commit {}", short_commit(commit)));
    }
    if parts.is_empty() {
        "bench-svg".to_owned()
    } else {
        parts.join(" · ")
    }
}

fn default_footer(meta: &Meta, rows: usize) -> String {
    let mut text = format!(
        "Generated from {rows} benchmark {}",
        if rows == 1 { "row" } else { "rows" }
    );
    if let Some(tool) = &meta.tool {
        text.push_str(" · ");
        text.push_str(tool);
    }
    if let Some(commit) = &meta.commit {
        text.push_str(" · ");
        text.push_str(short_commit(commit));
    }
    text.push_str(" · bars are relative to the best value in each unit");
    text
}

fn short_commit(commit: &str) -> &str {
    if commit.len() > 7 {
        &commit[..7]
    } else {
        commit
    }
}

/// Load the font, preferring an explicit file, then a family name, then `fonts/`.
pub fn load_font(
    explicit: Option<&Path>,
    family: Option<&str>,
) -> Result<Font, crate::font::FontError> {
    let path = crate::font::resolve_font(explicit, family)?;
    Font::load(&path)
}
