//! `[[row]]` mapping: rename rows, override units, and transform values with
//! regex captures, without writing any code.

use crate::small_map::SmallMap;

use serde_json::Value;

use crate::config::RowRule;
use crate::expr::{self, Expr};
use crate::model::{Benchmark, json_scalar};

pub struct Mapped {
    pub label: Option<String>,
    pub note: Option<String>,
    pub unit: String,
    pub value: f64,
    pub group: Option<String>,
}

/// Match `benchmark` against one rule. `id` is an exact match; `match` is a
/// regex whose capture groups are available as `$1`… in templates and values.
pub fn match_rule(rule: &RowRule, benchmark: &Benchmark) -> Result<Option<Vec<String>>, String> {
    if let Some(pattern) = &rule.pattern {
        let regex = regex::Regex::new(pattern)
            .map_err(|err| format!("invalid row match {pattern:?}: {err}"))?;
        let Some(captures) = regex.captures(&benchmark.id) else {
            return Ok(None);
        };
        let groups = (1..captures.len())
            .map(|index| {
                captures
                    .get(index)
                    .map(|group| group.as_str().to_owned())
                    .unwrap_or_default()
            })
            .collect();
        return Ok(Some(groups));
    }
    if let Some(id) = &rule.id {
        return Ok(if id == &benchmark.id {
            Some(Vec::new())
        } else {
            None
        });
    }
    Err("row rule needs either `id` or `match`".to_owned())
}

pub fn apply(rule: &RowRule, benchmark: &Benchmark, captures: &[String]) -> Result<Mapped, String> {
    let label = rule
        .label
        .as_ref()
        .map(|template| substitute(template, captures, &benchmark.params));
    let note = rule.note.as_ref().map(|template| {
        if template.is_empty() {
            String::new()
        } else {
            substitute(template, captures, &benchmark.params)
        }
    });
    let value = match &rule.value {
        Some(source) => {
            let expression: Expr =
                expr::parse(source).map_err(|err| format!("row value {source:?}: {err}"))?;
            expression.eval(benchmark.value, captures)
        }
        None => benchmark.value,
    };
    Ok(Mapped {
        label,
        note,
        unit: rule.unit.clone().unwrap_or_else(|| benchmark.unit.clone()),
        value,
        group: rule.group.clone().or_else(|| benchmark.group.clone()),
    })
}

/// `$1` → first capture, `{workers}` → a structured parameter.
fn substitute(template: &str, captures: &[String], params: &SmallMap<String, Value>) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'$' => {
                let start = index + 1;
                let mut end = start;
                while end < bytes.len() && (bytes[end] as char).is_ascii_digit() {
                    end += 1;
                }
                if end > start {
                    let capture: usize = template[start..end].parse().unwrap_or(0);
                    if let Some(value) = captures.get(capture.saturating_sub(1)) {
                        out.push_str(value);
                    }
                    index = end;
                    continue;
                }
                out.push('$');
                index += 1;
            }
            b'{' => {
                if let Some(offset) = template[index + 1..].find('}') {
                    let end = index + 1 + offset;
                    let key = &template[index + 1..end];
                    match params.get(key) {
                        Some(value) => out.push_str(&json_scalar(value)),
                        None => out.push_str(&template[index..=end]),
                    }
                    index = end + 1;
                    continue;
                }
                out.push('{');
                index += 1;
            }
            _ => {
                let ch = template[index..].chars().next().expect("in bounds");
                out.push(ch);
                index += ch.len_utf8();
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Direction, Stats};

    fn benchmark(id: &str, value: f64, unit: &str) -> Benchmark {
        Benchmark {
            id: id.to_owned(),
            group: None,
            label: None,
            note: None,
            unit: unit.to_owned(),
            value,
            direction: Direction::Higher,
            params: SmallMap::new(),
            metrics: SmallMap::new(),
            stats: Stats::default(),
            deterministic: false,
            extra: SmallMap::new(),
        }
    }

    fn pool_rule() -> RowRule {
        RowRule {
            id: None,
            pattern: Some(r"^pool\.read\.random4K\.w(\d+)$".to_owned()),
            label: Some("Pool read · $1 workers".to_owned()),
            note: Some("4 KiB · batch 32".to_owned()),
            unit: Some("IOPS".to_owned()),
            value: Some("value * $1 * 32".to_owned()),
            group: Some("Pool".to_owned()),
        }
    }

    #[test]
    fn regex_captures_drive_label_and_value() {
        let bench = benchmark("pool.read.random4K.w8", 1000.0, "ops/s");
        let captures = match_rule(&pool_rule(), &bench).unwrap().unwrap();
        assert_eq!(captures, vec!["8"]);
        let mapped = apply(&pool_rule(), &bench, &captures).unwrap();
        assert_eq!(mapped.label.as_deref(), Some("Pool read · 8 workers"));
        assert_eq!(mapped.unit, "IOPS");
        assert!((mapped.value - 256_000.0).abs() < 1e-9);
    }

    #[test]
    fn exact_id_rule_matches_only_that_row() {
        let rule = RowRule {
            id: Some("file.read.8MiB".to_owned()),
            pattern: None,
            label: Some("Sequential read".to_owned()),
            note: None,
            unit: None,
            value: None,
            group: Some("File I/O".to_owned()),
        };
        let yes = benchmark("file.read.8MiB", 1.0, "GiB/s");
        let no = benchmark("file.write.8MiB", 1.0, "GiB/s");
        assert!(match_rule(&rule, &yes).unwrap().is_some());
        assert!(match_rule(&rule, &no).unwrap().is_none());
        let mapped = apply(&rule, &yes, &[]).unwrap();
        assert_eq!(mapped.group.as_deref(), Some("File I/O"));
    }

    #[test]
    fn rule_without_group_keeps_the_input_group() {
        let mut bench = benchmark("x", 1.0, "ops/s");
        bench.group = Some("From input".to_owned());
        let rule = RowRule {
            id: Some("x".to_owned()),
            pattern: None,
            label: Some("renamed".to_owned()),
            note: None,
            unit: None,
            value: None,
            group: None,
        };
        let mapped = apply(&rule, &bench, &[]).unwrap();
        assert_eq!(mapped.group.as_deref(), Some("From input"));

        let mut override_rule = rule;
        override_rule.group = Some("From rule".to_owned());
        let mapped = apply(&override_rule, &bench, &[]).unwrap();
        assert_eq!(mapped.group.as_deref(), Some("From rule"));
    }

    #[test]
    fn params_are_available_in_templates() {
        let mut bench = benchmark("x", 1.0, "ops/s");
        bench.params.insert("workers".to_owned(), Value::from(4));
        let rule = RowRule {
            id: Some("x".to_owned()),
            pattern: None,
            label: Some("read · {workers}w".to_owned()),
            note: None,
            unit: None,
            value: None,
            group: None,
        };
        let mapped = apply(&rule, &bench, &[]).unwrap();
        assert_eq!(mapped.label.as_deref(), Some("read · 4w"));
    }
}
