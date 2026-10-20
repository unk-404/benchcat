//! V1 AST and parser. Frozen once released.

use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "compiler/v1/grammar.pest"]
pub(crate) struct V1Parser;

pub(crate) struct Source {
    pub name: String,
    pub aliases: Vec<String>,
    pub extensions: Vec<String>,
    pub detect: Detect,
    pub parse: Parse,
    pub dedupe: Option<String>,
}

pub(crate) enum Detect {
    Text(String),
    Prefix(String),
    Contains(String),
    Any(Vec<String>),
    Json(String),
    Csv(String),
    Toml(String),
    Directory(String),
}

pub(crate) enum Parse {
    Line {
        name: NameMode,
        extract: Vec<(String, ExtractKind, String)>,
        fields: Fields,
    },
    Records {
        start: String,
        name: NameMode,
        extract: Vec<(String, ExtractKind, String)>,
        fields: Fields,
    },
    Columns {
        split: String,
        require_first: String,
        columns: Vec<(String, usize)>,
        fields: Fields,
    },
    Text {
        rows: String,
        fields: Fields,
    },
    Json {
        rows: String,
        fields: Fields,
    },
    JsonLines {
        rows: String,
        fields: Fields,
    },
    Csv {
        fields: Fields,
    },
    Toml {
        rows: String,
        fields: Fields,
    },
    Directory {
        adapter: String,
    },
}

pub(crate) enum NameMode {
    Rest,
    Before(String),
    After(String),
}

pub(crate) enum ExtractKind {
    Before,
    After,
    UnitAfter,
}

pub(crate) struct Fields {
    pub id: Expr,
    pub label: Option<Expr>,
    pub group: Option<Expr>,
    pub note: Option<Expr>,
    pub value: Expr,
    pub unit: Expr,
    pub direction: Option<Expr>,
    pub params: Vec<(String, Expr)>,
    pub metrics: Vec<(String, Expr)>,
    pub stats: Vec<(String, Expr)>,
}

#[derive(Clone, Debug)]
pub(crate) enum Expr {
    Num(f64),
    Str(String),
    Bool(bool),
    Var(String),
    Call(String, Vec<Expr>),
    Binary(char, Box<Expr>, Box<Expr>),
}

pub(crate) fn parse(text: &str) -> Result<Source, String> {
    let parsed = V1Parser::parse(Rule::source, text)
        .map_err(|error| format!("{error}"))?
        .next()
        .ok_or_else(|| "expected source block".to_owned())?;
    let mut inner = parsed.into_inner();
    let name = inner
        .next()
        .ok_or_else(|| "missing source name".to_owned())?
        .as_str()
        .to_owned();
    let mut source = Source {
        name,
        aliases: Vec::new(),
        extensions: Vec::new(),
        detect: Detect::Text(String::new()),
        parse: Parse::Csv {
            fields: empty_fields(),
        },
        dedupe: None,
    };
    let mut has_detect = false;
    let mut has_parse = false;
    for item in inner {
        if item.as_rule() != Rule::item {
            continue;
        }
        let child = item.into_inner().next().unwrap();
        match child.as_rule() {
            Rule::aliases => source.aliases = strings(child)?,
            Rule::extensions => source.extensions = strings(child)?,
            Rule::detect => {
                if has_detect {
                    return Err("duplicate detect".to_owned());
                }
                source.detect = parse_detect(child)?;
                has_detect = true;
            }
            Rule::parse => {
                if has_parse {
                    return Err("duplicate parse".to_owned());
                }
                source.parse = parse_parse(child)?;
                has_parse = true;
            }
            Rule::dedupe => source.dedupe = Some(ident_value(child)?),
            other => return Err(format!("unexpected item {other:?}")),
        }
    }
    if !has_detect {
        return Err("missing detect".to_owned());
    }
    if !has_parse {
        return Err("missing parse".to_owned());
    }
    Ok(source)
}

fn empty_fields() -> Fields {
    Fields {
        id: Expr::Str(String::new()),
        label: None,
        group: None,
        note: None,
        value: Expr::Num(0.0),
        unit: Expr::Str(String::new()),
        direction: None,
        params: Vec::new(),
        metrics: Vec::new(),
        stats: Vec::new(),
    }
}

fn parse_detect(pair: pest::iterators::Pair<Rule>) -> Result<Detect, String> {
    let child = pair.into_inner().next().unwrap();
    let rule = child.as_rule();
    let mut inner = child.into_inner();
    match rule {
        Rule::text_detect => Ok(Detect::Text(regex(inner.next().unwrap())?)),
        Rule::prefix_detect => Ok(Detect::Prefix(string(inner.next().unwrap())?)),
        Rule::contains_detect => Ok(Detect::Contains(string(inner.next().unwrap())?)),
        Rule::any_detect => {
            let array = inner.next().ok_or("missing any array")?;
            Ok(Detect::Any(
                array
                    .into_inner()
                    .filter(|pair| pair.as_rule() == Rule::string)
                    .map(string)
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        }
        Rule::json_detect => Ok(Detect::Json(string(inner.next().unwrap())?)),
        Rule::csv_detect => Ok(Detect::Csv(string(inner.next().unwrap())?)),
        Rule::toml_detect => Ok(Detect::Toml(string(inner.next().unwrap())?)),
        Rule::directory_detect => Ok(Detect::Directory(inner.next().unwrap().as_str().to_owned())),
        other => Err(format!("unexpected detect {other:?}")),
    }
}

fn parse_parse(pair: pest::iterators::Pair<Rule>) -> Result<Parse, String> {
    let child = pair.into_inner().next().unwrap();
    let rule = child.as_rule();
    let mut inner = child.into_inner();
    match rule {
        Rule::line_parse => {
            let name_pair = inner.next().ok_or("missing line name")?;
            let name_text = name_pair.as_str();
            let marker = name_pair.into_inner().next().map(string).transpose()?;
            let name = if name_text.starts_with("before") {
                NameMode::Before(marker.ok_or("before needs a marker")?)
            } else if name_text.starts_with("after") {
                NameMode::After(marker.ok_or("after needs a marker")?)
            } else {
                NameMode::Rest
            };
            let extract_pair = inner.next().ok_or("missing extract block")?;
            let extract = parse_extract(extract_pair)?;
            Ok(Parse::Line {
                name,
                extract,
                fields: parse_fields(inner)?,
            })
        }
        Rule::records_parse => {
            let start = string(inner.next().ok_or("missing records start")?)?;
            let name_pair = inner.next().ok_or("missing records name")?;
            let name = if name_pair.as_str().starts_with("after") {
                let marker = string(
                    name_pair
                        .into_inner()
                        .next()
                        .ok_or("records name after needs a marker")?,
                )?;
                NameMode::After(marker)
            } else {
                NameMode::Rest
            };
            let extract_pair = inner.next().ok_or("missing extract block")?;
            let extract = parse_extract(extract_pair)?;
            Ok(Parse::Records {
                start,
                name,
                extract,
                fields: parse_fields(inner)?,
            })
        }
        Rule::columns_parse => {
            let split = inner.next().ok_or("missing split")?.as_str().to_owned();
            let require_first = string(inner.next().ok_or("missing require_first")?)?;
            let columns_pair = inner.next().ok_or("missing columns block")?;
            let mut columns = Vec::new();
            for pair in columns_pair.into_inner() {
                if pair.as_rule() != Rule::column {
                    continue;
                }
                let mut parts = pair.into_inner();
                let name = parts.next().unwrap().as_str().to_owned();
                let index = parts
                    .next()
                    .and_then(|value| value.as_str().parse::<usize>().ok())
                    .ok_or_else(|| "column index must be an integer".to_owned())?;
                columns.push((name, index));
            }
            Ok(Parse::Columns {
                split,
                require_first,
                columns,
                fields: parse_fields(inner)?,
            })
        }
        Rule::text_parse => {
            let rows = regex(inner.next().unwrap())?;
            Ok(Parse::Text {
                rows,
                fields: parse_fields(inner)?,
            })
        }
        Rule::json_parse => {
            let rows = string(inner.next().unwrap())?;
            Ok(Parse::Json {
                rows,
                fields: parse_fields(inner)?,
            })
        }
        Rule::jsonl_parse => {
            let rows = string(inner.next().unwrap())?;
            Ok(Parse::JsonLines {
                rows,
                fields: parse_fields(inner)?,
            })
        }
        Rule::csv_parse => Ok(Parse::Csv {
            fields: parse_fields(inner)?,
        }),
        Rule::toml_parse => {
            let rows = string(inner.next().unwrap())?;
            Ok(Parse::Toml {
                rows,
                fields: parse_fields(inner)?,
            })
        }
        Rule::directory_parse => {
            let _name = inner.next().ok_or("missing directory name")?;
            let adapter = inner
                .next()
                .ok_or("missing directory adapter")?
                .as_str()
                .to_owned();
            Ok(Parse::Directory { adapter })
        }
        other => Err(format!("unexpected parse {other:?}")),
    }
}

fn parse_fields<'a>(
    inner: impl Iterator<Item = pest::iterators::Pair<'a, Rule>>,
) -> Result<Fields, String> {
    let mut fields = empty_fields();
    let mut has_id = false;
    let mut has_value = false;
    let mut has_unit = false;
    let mut has_params = false;
    let mut has_metrics = false;
    let mut has_stats = false;
    for item in inner {
        let item = if item.as_rule() == Rule::parse_item {
            item.into_inner().next().unwrap()
        } else {
            item
        };
        match item.as_rule() {
            Rule::field => {
                let mut parts = item.into_inner();
                let name = parts.next().unwrap().as_str().to_owned();
                let expr = parse_expr(parts.next().unwrap());
                match name.as_str() {
                    "id" => {
                        fields.id = expr;
                        has_id = true;
                    }
                    "label" => fields.label = Some(expr),
                    "group" => fields.group = Some(expr),
                    "note" => fields.note = Some(expr),
                    "value" => {
                        fields.value = expr;
                        has_value = true;
                    }
                    "unit" => {
                        fields.unit = expr;
                        has_unit = true;
                    }
                    "direction" => fields.direction = Some(expr),
                    other => return Err(format!("unknown field {other}")),
                }
            }
            Rule::block => {
                let (name, pairs) = parse_block(item)?;
                match name.as_str() {
                    "params" => {
                        if has_params {
                            return Err("duplicate params block".to_owned());
                        }
                        fields.params = pairs;
                        has_params = true;
                    }
                    "metrics" => {
                        if has_metrics {
                            return Err("duplicate metrics block".to_owned());
                        }
                        fields.metrics = pairs;
                        has_metrics = true;
                    }
                    "stats" => {
                        if has_stats {
                            return Err("duplicate stats block".to_owned());
                        }
                        fields.stats = pairs;
                        has_stats = true;
                    }
                    other => return Err(format!("unknown block {other}")),
                }
            }
            _ => {}
        }
    }
    if !has_id || !has_value || !has_unit {
        return Err("parse block requires id, value and unit".to_owned());
    }
    Ok(fields)
}

fn parse_extract(
    pair: pest::iterators::Pair<Rule>,
) -> Result<Vec<(String, ExtractKind, String)>, String> {
    let mut extract = Vec::new();
    for pair in pair.into_inner() {
        if pair.as_rule() != Rule::extraction {
            continue;
        }
        let raw = pair.as_str();
        let kind = if raw.contains("number_before") {
            ExtractKind::Before
        } else if raw.contains("number_after") {
            ExtractKind::After
        } else if raw.contains("unit_after") {
            ExtractKind::UnitAfter
        } else {
            return Err(format!("unknown extraction {raw}"));
        };
        let mut parts = pair.into_inner();
        let field = parts.next().unwrap().as_str().to_owned();
        let marker = string(parts.next().ok_or("missing extraction marker")?)?;
        extract.push((field, kind, marker));
    }
    Ok(extract)
}

fn parse_block(pair: pest::iterators::Pair<Rule>) -> Result<(String, Vec<(String, Expr)>), String> {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_owned();
    let mut pairs = Vec::new();
    for pair in inner {
        if pair.as_rule() != Rule::pair {
            continue;
        }
        let mut parts = pair.into_inner();
        let key = parts.next().unwrap().as_str().to_owned();
        pairs.push((key, parse_expr(parts.next().unwrap())));
    }
    Ok((name, pairs))
}

fn parse_expr(pair: pest::iterators::Pair<Rule>) -> Expr {
    match pair.as_rule() {
        Rule::expr => {
            let mut inner = pair.into_inner();
            let first = parse_expr(inner.next().unwrap());
            let mut expr = first;
            while let Some(op) = inner.next() {
                let rhs = parse_expr(inner.next().unwrap());
                let ch = op.as_str().chars().next().unwrap();
                expr = Expr::Binary(ch, Box::new(expr), Box::new(rhs));
            }
            expr
        }
        Rule::term => {
            let mut inner = pair.into_inner();
            let mut expr = parse_expr(inner.next().unwrap());
            while let Some(op) = inner.next() {
                let rhs = parse_expr(inner.next().unwrap());
                let ch = op.as_str().chars().next().unwrap();
                expr = Expr::Binary(ch, Box::new(expr), Box::new(rhs));
            }
            expr
        }
        Rule::factor => parse_expr(pair.into_inner().next().unwrap()),
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

fn regex(pair: pest::iterators::Pair<Rule>) -> Result<String, String> {
    let raw = pair.as_str();
    if !raw.starts_with('/') || !raw.ends_with('/') {
        return Err("expected regex".to_owned());
    }
    let inner = &raw[1..raw.len() - 1];
    regex::Regex::new(inner).map_err(|error| format!("invalid regex: {error}"))?;
    Ok(inner.to_owned())
}

fn ident_value(pair: pest::iterators::Pair<Rule>) -> Result<String, String> {
    pair.into_inner()
        .next()
        .map(|pair| pair.as_str().to_owned())
        .ok_or_else(|| "expected identifier".to_owned())
}
