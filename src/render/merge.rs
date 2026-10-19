//! Union of ids across sources, row rules and bar fractions.

use std::collections::HashMap;

use crate::config::{Config, RowRule};
use crate::input::params_note;
use crate::layout::{RowPlan, SeriesValue};
use crate::model::{Benchmark, Direction, Unit, UnitKind};
use crate::rows::{self, Mapped};

use super::{RenderError, SeriesInput};

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
pub(crate) fn merge_rows(
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
