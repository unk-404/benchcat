//! V2 AST and parser: table variants (Markdown/CSV) with canonical records.

use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "compiler/v2/grammar.pest"]
pub(crate) struct V2Parser;

pub(crate) struct Source {
    pub name: String,
    pub aliases: Vec<String>,
    pub extensions: Vec<String>,
    pub inputs: Vec<Input>,
    pub rows: RowFields,
    pub params: Vec<(String, Expr)>,
    pub metrics: Vec<(String, Expr)>,
    pub stats: Vec<(String, Expr)>,
}

pub(crate) struct Input {
    pub kind: String,
    pub accept_header: String,
    pub record: Vec<(String, Expr)>,
}

pub(crate) struct RowFields {
    pub id: Expr,
    pub value: Expr,
    pub unit: Expr,
    pub label: Option<Expr>,
    pub group: Option<Expr>,
    pub note: Option<Expr>,
    pub direction: Option<Expr>,
    pub filter: Option<Expr>,
}

#[derive(Clone, Debug)]
pub(crate) enum Expr {
    Num(f64),
    Str(String),
    Bool(bool),
    Var(String),
    Call(String, Vec<Expr>),
    Coalesce(Box<Expr>, Box<Expr>),
    Pipe(Box<Expr>, Box<Expr>),
}

pub(crate) fn parse(text: &str) -> Result<Source, String> {
    let parsed = V2Parser::parse(Rule::source, text)
        .map_err(|error| format!("{error}"))?
        .next()
        .ok_or_else(|| "expected source block".to_owned())?;
    let mut inner = parsed.into_inner();
    let name = inner
        .next()
        .ok_or_else(|| "missing source name".to_owned())?
        .as_str()
        .to_owned();
    let mut aliases = Vec::new();
    let mut extensions = Vec::new();
    let mut inputs = Vec::new();
    let mut rows = None;
    let mut params = Vec::new();
    let mut metrics = Vec::new();
    let mut stats = Vec::new();
    let mut has_inputs = false;
    let mut has_rows = false;
    let mut has_params = false;
    let mut has_metrics = false;
    let mut has_stats = false;
    for pair in inner {
        match pair.as_rule() {
            Rule::aliases => aliases = strings(pair)?,
            Rule::extensions => extensions = strings(pair)?,
            Rule::item => {
                let item = pair.into_inner().next().unwrap();
                match item.as_rule() {
                    Rule::inputs => {
                        if has_inputs {
                            return Err("duplicate inputs block".to_owned());
                        }
                        inputs = parse_inputs(item)?;
                        has_inputs = true;
                    }
                    Rule::rows => {
                        if has_rows {
                            return Err("duplicate rows block".to_owned());
                        }
                        rows = Some(parse_rows(item)?);
                        has_rows = true;
                    }
                    Rule::params => {
                        if has_params {
                            return Err("duplicate params block".to_owned());
                        }
                        params = pairs(item)?;
                        has_params = true;
                    }
                    Rule::metrics => {
                        if has_metrics {
                            return Err("duplicate metrics block".to_owned());
                        }
                        metrics = pairs(item)?;
                        has_metrics = true;
                    }
                    Rule::stats => {
                        if has_stats {
                            return Err("duplicate stats block".to_owned());
                        }
                        stats = pairs(item)?;
                        has_stats = true;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    let rows = rows.ok_or_else(|| "missing rows block".to_owned())?;
    if inputs.is_empty() {
        return Err("missing inputs block".to_owned());
    }
    Ok(Source {
        name,
        aliases,
        extensions,
        inputs,
        rows,
        params,
        metrics,
        stats,
    })
}

fn parse_inputs(pair: pest::iterators::Pair<Rule>) -> Result<Vec<Input>, String> {
    let mut out = Vec::new();
    for variant in pair.into_inner() {
        if variant.as_rule() != Rule::input_variant {
            continue;
        }
        let mut inner = variant.into_inner();
        let kind = inner
            .next()
            .ok_or("missing variant kind")?
            .as_str()
            .to_owned();
        let accept = inner.next().ok_or("missing accept")?;
        let accept_header = string(accept.into_inner().next().ok_or("missing accept header")?)?;
        let record_pair = inner.next().ok_or("missing record")?;
        let record = pairs(record_pair)?;
        out.push(Input {
            kind,
            accept_header,
            record,
        });
    }
    Ok(out)
}

fn parse_rows(pair: pest::iterators::Pair<Rule>) -> Result<RowFields, String> {
    let mut id = None;
    let mut value = None;
    let mut unit = None;
    let mut label = None;
    let mut group = None;
    let mut note = None;
    let mut direction = None;
    let mut filter = None;
    for field in pair.into_inner() {
        if field.as_rule() != Rule::row_field {
            continue;
        }
        let mut parts = field.into_inner();
        let name = parts.next().unwrap().as_str();
        let expr = parse_expr(parts.next().unwrap());
        match name {
            "id" => id = Some(expr),
            "value" => value = Some(expr),
            "unit" => unit = Some(expr),
            "label" => label = Some(expr),
            "group" => group = Some(expr),
            "note" => note = Some(expr),
            "direction" => direction = Some(expr),
            "where" => filter = Some(expr),
            _ => {}
        }
    }
    Ok(RowFields {
        id: id.ok_or("rows.id is required")?,
        value: value.ok_or("rows.value is required")?,
        unit: unit.ok_or("rows.unit is required")?,
        label,
        group,
        note,
        direction,
        filter,
    })
}

fn pairs(pair: pest::iterators::Pair<Rule>) -> Result<Vec<(String, Expr)>, String> {
    let mut out = Vec::new();
    for item in pair.into_inner() {
        if item.as_rule() != Rule::pair {
            continue;
        }
        let mut parts = item.into_inner();
        let name = parts.next().unwrap().as_str().to_owned();
        out.push((name, parse_expr(parts.next().unwrap())));
    }
    Ok(out)
}

fn parse_expr(pair: pest::iterators::Pair<Rule>) -> Expr {
    match pair.as_rule() {
        Rule::expr => {
            let mut inner = pair.into_inner();
            let mut expr = parse_expr(inner.next().unwrap());
            while let Some(_pipe) = inner.next() {
                let rhs = parse_expr(inner.next().unwrap());
                expr = Expr::Pipe(Box::new(expr), Box::new(rhs));
            }
            expr
        }
        Rule::coalesce => {
            let mut inner = pair.into_inner();
            let mut expr = parse_expr(inner.next().unwrap());
            while inner.next().is_some() {
                let rhs = parse_expr(inner.next().unwrap());
                expr = Expr::Coalesce(Box::new(expr), Box::new(rhs));
            }
            expr
        }
        Rule::primary => parse_expr(pair.into_inner().next().unwrap()),
        Rule::call => {
            let mut inner = pair.into_inner();
            let name = inner.next().unwrap().as_str().to_owned();
            Expr::Call(name, inner.map(parse_expr).collect())
        }
        Rule::string => Expr::Str(string(pair).unwrap_or_default()),
        Rule::number => Expr::Num(pair.as_str().parse().unwrap_or(0.0)),
        Rule::boolean => Expr::Bool(pair.as_str() == "true"),
        _ => Expr::Var(pair.as_str().to_owned()),
    }
}

fn strings(pair: pest::iterators::Pair<Rule>) -> Result<Vec<String>, String> {
    let array = pair
        .into_inner()
        .find(|pair| pair.as_rule() == Rule::string_array)
        .ok_or_else(|| "expected string array".to_owned())?;
    array
        .into_inner()
        .filter(|pair| pair.as_rule() == Rule::string)
        .map(string)
        .collect()
}

fn string(pair: pest::iterators::Pair<Rule>) -> Result<String, String> {
    let raw = pair.as_str();
    if !raw.starts_with('"') || !raw.ends_with('"') {
        return Err("expected string".to_owned());
    }
    let inner = &raw[1..raw.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => return Err("unterminated escape".to_owned()),
        }
    }
    Ok(out)
}
