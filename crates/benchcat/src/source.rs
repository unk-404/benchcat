//! Source definition ABI for `sources/*.rs`.
//!
//! Each preset is ordinary Rust compiled directly into the crate by the
//! `benchcat-source-gen` macro. There is no runtime DSL, expression parser or
//! intermediate spec: `parse` is the parser that runs.

use std::path::Path;

use serde_json::Value;

use crate::input::Loaded;
use crate::model::{Benchmark, Direction, Stats};

pub type DetectFn = fn(&str) -> bool;
pub type ParseFn = fn(&str) -> Result<Loaded, String>;
pub type DetectDirFn = fn(&Path) -> bool;
pub type ParseDirFn = fn(&Path) -> Result<Loaded, String>;

pub struct SourceDef {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub extensions: &'static [&'static str],
    /// Content-based detection.
    pub detect: Option<DetectFn>,
    /// Single-document parser.
    pub parse: ParseFn,
    /// Directory detection (criterion etc.).
    pub detect_dir: Option<DetectDirFn>,
    /// Directory parser.
    pub parse_dir: Option<ParseDirFn>,
}

impl SourceDef {
    pub fn matches_alias(&self, value: &str) -> bool {
        self.name.eq_ignore_ascii_case(value)
            || self
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(value))
    }
}

/// A canonical row with direction inferred from the unit.
pub fn row(id: String, value: f64, unit: &str) -> Benchmark {
    let unit = normalize_unit(unit);
    let direction = direction_for(&unit);
    Benchmark {
        id: id.clone(),
        group: None,
        label: Some(id),
        note: None,
        unit,
        value,
        direction,
        params: Default::default(),
        metrics: Default::default(),
        stats: Stats::default(),
        deterministic: false,
        extra: Default::default(),
    }
}

pub fn direction_for(unit: &str) -> Direction {
    if unit.ends_with("/op") {
        Direction::Lower
    } else {
        Direction::Higher
    }
}

pub fn normalize_unit(unit: &str) -> String {
    let lower = unit.trim().to_ascii_lowercase().replace('μ', "µ");
    match lower.as_str() {
        "" => "ops/s".to_owned(),
        "ns" | "nanoseconds" => "ns/op".to_owned(),
        "us" | "µs" | "microseconds" => "us/op".to_owned(),
        "ms" | "milliseconds" => "ms/op".to_owned(),
        "s" | "sec" | "seconds" => "s/op".to_owned(),
        _ => unit.trim().to_owned(),
    }
}

pub fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(value) => value.as_f64(),
        Value::String(value) => value.trim().parse().ok(),
        _ => None,
    }
}

pub fn number_at(value: &Value, path: &str) -> Option<f64> {
    lookup(value, path).and_then(number)
}

pub fn string(value: &Value) -> Option<String> {
    value.as_str().map(ToOwned::to_owned)
}

pub fn string_at(value: &Value, path: &str) -> Option<String> {
    lookup(value, path).and_then(string)
}

pub fn lookup<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let path = path.trim().trim_start_matches('$').trim_start_matches('.');
    if path.is_empty() {
        return Some(value);
    }
    let mut current = value;
    for segment in path.split('.') {
        if segment.is_empty() {
            continue;
        }
        current = match current {
            Value::Object(map) => map.get(segment)?,
            Value::Array(items) => items.get(segment.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current)
}

pub fn time_scale(unit: &str) -> f64 {
    match unit.trim().to_ascii_lowercase().as_str() {
        "ns" | "nanoseconds" => 1.0,
        "us" | "µs" | "microseconds" => 1e3,
        "ms" | "milliseconds" => 1e6,
        "s" | "sec" | "seconds" => 1e9,
        _ => 1.0,
    }
}

pub fn percent(text: &str) -> f64 {
    text.split(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .find(|part| !part.is_empty())
        .and_then(|part| part.parse().ok())
        .unwrap_or(0.0)
}

pub fn last<'a>(text: &'a str, separator: &str) -> &'a str {
    text.rsplit_once(separator)
        .map(|(_, tail)| tail)
        .unwrap_or(text)
}

pub fn drop_last<'a>(text: &'a str, separator: &str) -> &'a str {
    text.rsplit_once(separator)
        .map(|(head, _)| head)
        .unwrap_or(text)
}

pub struct JsonRow<'a> {
    pub value: &'a Value,
    pub context: Vec<Value>,
}

pub fn json_rows<'a>(value: &'a Value, path: &str) -> Vec<JsonRow<'a>> {
    let path = path.trim().trim_start_matches('$').trim_start_matches('.');
    let mut out = Vec::new();
    if path.is_empty() {
        match value {
            Value::Array(items) => {
                for item in items {
                    out.push(JsonRow {
                        value: item,
                        context: Vec::new(),
                    });
                }
            }
            other => out.push(JsonRow {
                value: other,
                context: Vec::new(),
            }),
        }
        return out;
    }
    let segments: Vec<&str> = path
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect();
    walk_json(value, &segments, &mut Vec::new(), &mut out);
    out
}

fn walk_json<'a>(
    value: &'a Value,
    segments: &[&str],
    context: &mut Vec<Value>,
    out: &mut Vec<JsonRow<'a>>,
) {
    if segments.is_empty() {
        match value {
            Value::Array(items) => {
                for item in items {
                    out.push(JsonRow {
                        value: item,
                        context: context.clone(),
                    });
                }
            }
            other => out.push(JsonRow {
                value: other,
                context: context.clone(),
            }),
        }
        return;
    }
    let segment = segments[0];
    let rest = &segments[1..];
    if segment == "*" {
        match value {
            Value::Object(map) => {
                let mut keys: Vec<_> = map.keys().collect();
                keys.sort();
                for key in keys {
                    context.push(Value::String(key.clone()));
                    walk_json(&map[key], rest, context, out);
                    context.pop();
                }
            }
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    context.push(Value::from(index as u64));
                    walk_json(item, rest, context, out);
                    context.pop();
                }
            }
            _ => {}
        }
        return;
    }
    let next = match value {
        Value::Object(map) => map.get(segment),
        Value::Array(items) => segment
            .parse::<usize>()
            .ok()
            .and_then(|index| items.get(index)),
        _ => None,
    };
    if let Some(next) = next {
        walk_json(next, rest, context, out);
    }
}

pub fn exists(value: &Value, path: &str) -> bool {
    let path = path.trim().trim_start_matches('$').trim_start_matches('.');
    let segments: Vec<&str> = path
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect();
    walk_exists(value, &segments)
}

fn walk_exists(value: &Value, segments: &[&str]) -> bool {
    let Some((segment, rest)) = segments.split_first() else {
        return true;
    };
    if *segment == "*" {
        return match value {
            Value::Array(items) => items.iter().any(|item| walk_exists(item, rest)),
            Value::Object(map) => map.values().any(|item| walk_exists(item, rest)),
            _ => false,
        };
    }
    if let Value::Array(items) = value
        && segment.parse::<usize>().is_err()
    {
        return items.iter().any(|item| walk_exists(item, segments));
    }
    let next = match value {
        Value::Object(map) => map.get(*segment),
        Value::Array(items) => segment
            .parse::<usize>()
            .ok()
            .and_then(|index| items.get(index)),
        _ => None,
    };
    next.is_some_and(|next| walk_exists(next, rest))
}

pub fn first_str(values: Vec<Option<String>>) -> Option<String> {
    values.into_iter().flatten().find(|value| !value.is_empty())
}

pub fn first_num(values: Vec<Option<f64>>) -> Option<f64> {
    values.into_iter().flatten().next()
}

pub fn context_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

pub fn dedupe_max(rows: Vec<Benchmark>) -> Vec<Benchmark> {
    let mut order: Vec<String> = Vec::new();
    let mut best: hashbrown::HashMap<String, Benchmark> = Default::default();
    for row in rows {
        match best.get_mut(&row.id) {
            None => {
                order.push(row.id.clone());
                best.insert(row.id.clone(), row);
            }
            Some(existing) => {
                if row.value > existing.value {
                    *existing = row;
                }
            }
        }
    }
    order
        .into_iter()
        .filter_map(|id| best.remove(&id))
        .collect()
}

pub fn number_after(lines: &[&str], marker: &str) -> Option<f64> {
    for line in lines {
        if let Some((_, tail)) = line.split_once(marker) {
            if let Some(value) = first_number(tail) {
                return Some(value);
            }
        }
    }
    None
}

pub fn number_before(lines: &[&str], marker: &str) -> Option<f64> {
    for line in lines {
        if let Some((head, _)) = line.split_once(marker) {
            if let Some(value) = last_number(head) {
                return Some(value);
            }
        }
    }
    None
}

pub fn unit_after(lines: &[&str], marker: &str) -> Option<String> {
    for line in lines {
        if let Some((_, tail)) = line.split_once(marker) {
            for token in tail.split_whitespace() {
                let token = token.trim_matches(|ch: char| !ch.is_ascii_alphabetic() && ch != 'µ');
                if matches!(token, "ns" | "us" | "µs" | "ms" | "s") {
                    return Some(token.to_owned());
                }
            }
        }
    }
    None
}

fn first_number(text: &str) -> Option<f64> {
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            current.push(ch);
        } else if !current.is_empty() {
            if let Ok(value) = current.parse::<f64>() {
                return Some(value);
            }
            current.clear();
        }
    }
    current.parse::<f64>().ok()
}

fn last_number(text: &str) -> Option<f64> {
    let mut values = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            current.push(ch);
        } else if !current.is_empty() {
            if let Ok(value) = current.parse::<f64>() {
                values.push(value);
            }
            current.clear();
        }
    }
    if let Ok(value) = current.parse::<f64>() {
        values.push(value);
    }
    values.pop()
}
