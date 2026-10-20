//! V1 code generation. Emits direct Rust; no runtime AST or evaluator.

use proc_macro2::TokenStream;
use quote::quote;

use super::ast::{Detect, Expr, ExtractKind, Fields, NameMode, Parse, Source};

pub(crate) fn compile(source: &Source) -> Result<TokenStream, String> {
    let name = &source.name;
    let aliases = &source.aliases;
    let extensions = &source.extensions;
    let detect = detect_fn(&source.detect)?;
    let dedupe = dedupe_tokens(source.dedupe.as_deref())?;
    let parse = parse_fn(&source.parse, dedupe)?;
    let (detect_static, parse_static, detect_dir_static, parse_dir_static) = match &source.parse {
        Parse::Directory { .. } => (
            quote!(None),
            quote!(parse_file),
            quote!(Some(detect_dir)),
            quote!(Some(parse_dir)),
        ),
        _ => (
            quote!(Some(detect)),
            quote!(parse),
            quote!(None),
            quote!(None),
        ),
    };
    let parse = match parse {
        ParseTokens::Single(parse) => parse,
        ParseTokens::Directory(items) => items,
    };
    Ok(quote! {
        pub static SOURCE: crate::source::SourceDef = crate::source::SourceDef {
            name: #name,
            aliases: &[#(#aliases),*],
            extensions: &[#(#extensions),*],
            detect: #detect_static,
            parse: #parse_static,
            detect_dir: #detect_dir_static,
            parse_dir: #parse_dir_static,
        };

        #detect
        #parse
    })
}

enum ParseTokens {
    Single(TokenStream),
    Directory(TokenStream),
}

fn detect_fn(detect: &Detect) -> Result<TokenStream, String> {
    match detect {
        Detect::Text(pattern) => Ok(quote! {
            fn detect(text: &str) -> bool {
                static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
                RE.get_or_init(|| regex::Regex::new(#pattern).expect("valid detect regex"))
                    .is_match(text)
            }
        }),
        Detect::Json(path) => Ok(quote! {
            fn detect(text: &str) -> bool {
                serde_json::from_str::<serde_json::Value>(text)
                    .ok()
                    .is_some_and(|value| crate::source::exists(&value, #path))
            }
        }),
        Detect::Prefix(prefix) => Ok(quote! {
            fn detect(text: &str) -> bool {
                text.lines().any(|line| line.trim_start().starts_with(#prefix))
            }
        }),
        Detect::Contains(needle) => Ok(quote! {
            fn detect(text: &str) -> bool {
                text.contains(#needle)
            }
        }),
        Detect::Any(needles) => Ok(quote! {
            fn detect(text: &str) -> bool {
                [#(#needles),*].iter().any(|needle| text.contains(needle))
            }
        }),
        Detect::Csv(header) => Ok(quote! {
            fn detect(text: &str) -> bool {
                text.lines()
                    .next()
                    .is_some_and(|line| line.contains(#header))
            }
        }),
        Detect::Toml(path) => Ok(quote! {
            fn detect(text: &str) -> bool {
                toml::from_str::<toml::Value>(text)
                    .ok()
                    .and_then(|value| serde_json::to_value(value).ok())
                    .is_some_and(|value| crate::source::exists(&value, #path))
            }
        }),
        Detect::Directory(adapter) => {
            if adapter != "criterion" {
                return Err(format!("unknown directory detector {adapter:?}"));
            }
            Ok(quote! {
                fn detect_dir(path: &std::path::Path) -> bool {
                    crate::input::is_criterion_dir(path)
                }
            })
        }
    }
}

fn parse_fn(parse: &Parse, dedupe: TokenStream) -> Result<ParseTokens, String> {
    match parse {
        Parse::Line {
            name,
            extract,
            fields,
        } => {
            let body = row_body(fields)?;
            let name_expr = match name {
                NameMode::Rest => quote!(__line.trim().to_owned()),
                NameMode::Before(marker) => quote! {
                    __line
                        .split_once(#marker)
                        .map(|(value, _)| value.trim().to_owned())
                        .unwrap_or_else(|| __line.trim().to_owned())
                },
                NameMode::After(marker) => quote! {
                    __line
                        .split_once(#marker)
                        .map(|(_, value)| value.trim().to_owned())
                        .unwrap_or_default()
                },
            };
            let mut text_arms = Vec::new();
            let mut num_arms = Vec::new();
            let mut has_arms = Vec::new();
            for (field, kind, marker) in extract {
                has_arms.push(quote!(#field => true,));
                match kind {
                    ExtractKind::Before => num_arms.push(quote! {
                        #field => crate::source::number_before(&[__line], #marker),
                    }),
                    ExtractKind::After => num_arms.push(quote! {
                        #field => crate::source::number_after(&[__line], #marker),
                    }),
                    ExtractKind::UnitAfter => text_arms.push(quote! {
                        #field => crate::source::unit_after(&[__line], #marker),
                    }),
                }
            }
            text_arms.push(quote! {
                "name" => Some(__name.clone()),
            });
            Ok(ParseTokens::Single(quote! {
                fn parse(text: &str) -> Result<crate::input::Loaded, String> {
                    let mut loaded = crate::input::Loaded::default();
                    loaded.meta.tool = Some("source".to_owned());
                    for __line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
                        let __name = #name_expr;
                        let __str = |path: &str| match path {
                            #(#text_arms)*
                            _ => None,
                        };
                        let __num = |path: &str| match path {
                            #(#num_arms)*
                            _ => None,
                        };
                        let __has = |path: &str| match path {
                            #(#has_arms)*
                            _ => false,
                        };
                        let __ctx = |_: usize| None::<String>;
                        #body
                    }
                    #dedupe
                    Ok(loaded)
                }
            }))
        }
        Parse::Records {
            start,
            name,
            extract,
            fields,
        } => {
            let body = row_body(fields)?;
            let name_expr = match name {
                NameMode::Rest => quote! {
                    __start.trim_start_matches(#start).trim().to_owned()
                },
                NameMode::Before(marker) => quote! {
                    __start
                        .split_once(#marker)
                        .map(|(value, _)| value.trim().to_owned())
                        .unwrap_or_default()
                },
                NameMode::After(marker) => quote! {
                    __start
                        .split_once(#marker)
                        .map(|(_, value)| value.trim().to_owned())
                        .unwrap_or_default()
                },
            };
            let mut text_arms = Vec::new();
            let mut num_arms = Vec::new();
            let mut has_arms = Vec::new();
            for (field, kind, marker) in extract {
                has_arms.push(quote!(#field => true,));
                match kind {
                    ExtractKind::Before => num_arms.push(quote! {
                        #field => crate::source::number_before(&__block, #marker),
                    }),
                    ExtractKind::After => num_arms.push(quote! {
                        #field => crate::source::number_after(&__block, #marker),
                    }),
                    ExtractKind::UnitAfter => text_arms.push(quote! {
                        #field => crate::source::unit_after(&__block, #marker),
                    }),
                }
            }
            text_arms.push(quote! {
                "name" => Some(__name.clone()),
            });
            Ok(ParseTokens::Single(quote! {
                fn parse(text: &str) -> Result<crate::input::Loaded, String> {
                    let mut loaded = crate::input::Loaded::default();
                    loaded.meta.tool = Some("source".to_owned());
                    let mut __blocks: Vec<Vec<&str>> = Vec::new();
                    for line in text.lines() {
                        if line.starts_with(#start) {
                            __blocks.push(vec![line]);
                        } else if let Some(block) = __blocks.last_mut() {
                            block.push(line);
                        }
                    }
                    for __block in __blocks {
                        let __start = __block[0];
                        let __name = #name_expr;
                        let __str = |path: &str| match path {
                            #(#text_arms)*
                            _ => None,
                        };
                        let __num = |path: &str| match path {
                            #(#num_arms)*
                            _ => None,
                        };
                        let __has = |path: &str| match path {
                            #(#has_arms)*
                            _ => false,
                        };
                        let __ctx = |_: usize| None::<String>;
                        #body
                    }
                    #dedupe
                    Ok(loaded)
                }
            }))
        }
        Parse::Columns {
            split,
            require_first,
            columns,
            fields,
        } => {
            let body = row_body(fields)?;
            let split = match split.as_str() {
                "whitespace" => quote!(line.split_whitespace().collect::<Vec<_>>()),
                "comma" => quote!(line.split(',').map(str::trim).collect::<Vec<_>>()),
                "tab" => quote!(line.split('\t').collect::<Vec<_>>()),
                other => return Err(format!("unknown column split {other:?}")),
            };
            let require = if require_first.is_empty() {
                quote!()
            } else {
                quote! {
                    if !__cols.first().is_some_and(|value| value.starts_with(#require_first)) {
                        continue;
                    }
                }
            };
            let mut arms = Vec::new();
            for (name, index) in columns {
                arms.push(quote! {
                    #name => __cols.get(#index).copied(),
                });
            }
            Ok(ParseTokens::Single(quote! {
                fn parse(text: &str) -> Result<crate::input::Loaded, String> {
                    let mut loaded = crate::input::Loaded::default();
                    loaded.meta.tool = Some("source".to_owned());
                    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
                        let __cols: Vec<&str> = #split;
                        #require
                        let __str = |path: &str| match path {
                            #(#arms)*
                            _ => None,
                        }
                        .map(|value| value.to_owned());
                        let __num = |path: &str| match path {
                            #(#arms)*
                            _ => None,
                        }
                        .and_then(|value| value.parse::<f64>().ok());
                        let __has = |path: &str| match path {
                            #(#arms)*
                            _ => None,
                        }
                        .is_some();
                        let __ctx = |_: usize| None::<String>;
                        #body
                    }
                    #dedupe
                    Ok(loaded)
                }
            }))
        }
        Parse::Text { rows, fields } => {
            let body = row_body(fields)?;
            Ok(ParseTokens::Single(quote! {
                fn parse(text: &str) -> Result<crate::input::Loaded, String> {
                    let mut loaded = crate::input::Loaded::default();
                    loaded.meta.tool = Some("source".to_owned());
                    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
                    let re = RE.get_or_init(|| regex::Regex::new(#rows).expect("valid rows regex"));
                    for caps in re.captures_iter(text) {
                        let __str = |path: &str| {
                            caps.name(path).map(|value| value.as_str().to_owned())
                        };
                        let __num = |path: &str| {
                            caps.name(path)
                                .and_then(|value| value.as_str().parse::<f64>().ok())
                        };
                        let __has = |path: &str| caps.name(path).is_some();
                        let __ctx = |_: usize| None::<String>;
                        #body
                    }
                    #dedupe
                    Ok(loaded)
                }
            }))
        }
        Parse::Json { rows, fields } => {
            let body = json_body(fields)?;
            Ok(ParseTokens::Single(quote! {
                fn parse(text: &str) -> Result<crate::input::Loaded, String> {
                    let mut loaded = crate::input::Loaded::default();
                    loaded.meta.tool = Some("source".to_owned());
                    let __doc: serde_json::Value =
                        serde_json::from_str(text).map_err(|error| error.to_string())?;
                    for __row_ref in crate::source::json_rows(&__doc, #rows) {
                        let __row = __row_ref.value;
                        let __context = &__row_ref.context;
                        let __str = |path: &str| crate::source::string_at(__row, path);
                        let __num = |path: &str| crate::source::number_at(__row, path);
                        let __has = |path: &str| crate::source::lookup(__row, path).is_some();
                        let __ctx = |index: usize| {
                            __context.get(index).and_then(crate::source::context_text)
                        };
                        #body
                    }
                    #dedupe
                    Ok(loaded)
                }
            }))
        }
        Parse::JsonLines { rows, fields } => {
            let body = json_body(fields)?;
            Ok(ParseTokens::Single(quote! {
                fn parse(text: &str) -> Result<crate::input::Loaded, String> {
                    let mut loaded = crate::input::Loaded::default();
                    loaded.meta.tool = Some("source".to_owned());
                    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
                        let line = line.strip_prefix('\u{feff}').unwrap_or(line);
                        let Ok(__doc) = serde_json::from_str::<serde_json::Value>(line) else {
                            continue;
                        };
                        for __row_ref in crate::source::json_rows(&__doc, #rows) {
                            let __row = __row_ref.value;
                            let __context = &__row_ref.context;
                            let __str = |path: &str| crate::source::string_at(__row, path);
                            let __num = |path: &str| crate::source::number_at(__row, path);
                            let __has = |path: &str| crate::source::lookup(__row, path).is_some();
                            let __ctx = |index: usize| {
                                __context.get(index).and_then(crate::source::context_text)
                            };
                            #body
                        }
                    }
                    #dedupe
                    Ok(loaded)
                }
            }))
        }
        Parse::Csv { fields } => {
            let body = row_body(fields)?;
            Ok(ParseTokens::Single(quote! {
                fn parse(text: &str) -> Result<crate::input::Loaded, String> {
                    let mut loaded = crate::input::Loaded::default();
                    loaded.meta.tool = Some("source".to_owned());
                    let delimiter = if text.lines().next().is_some_and(|line| line.contains('\t')) {
                        b'\t'
                    } else {
                        b','
                    };
                    let mut reader = csv::ReaderBuilder::new()
                        .delimiter(delimiter)
                        .flexible(true)
                        .from_reader(text.as_bytes());
                    let headers = reader
                        .headers()
                        .map_err(|error| error.to_string())?
                        .clone();
                    for record in reader.records() {
                        let record = record.map_err(|error| error.to_string())?;
                        let __str = |path: &str| {
                            headers
                                .iter()
                                .position(|header| header == path)
                                .and_then(|index| record.get(index))
                                .map(|value| value.to_owned())
                        };
                        let __num = |path: &str| {
                            headers
                                .iter()
                                .position(|header| header == path)
                                .and_then(|index| record.get(index))
                                .and_then(|value| value.parse::<f64>().ok())
                        };
                        let __has = |path: &str| headers.iter().any(|header| header == path);
                        let __ctx = |_: usize| None::<String>;
                        #body
                    }
                    #dedupe
                    Ok(loaded)
                }
            }))
        }
        Parse::Toml { rows, fields } => {
            let body = json_body(fields)?;
            Ok(ParseTokens::Single(quote! {
                fn parse(text: &str) -> Result<crate::input::Loaded, String> {
                    let mut loaded = crate::input::Loaded::default();
                    loaded.meta.tool = Some("source".to_owned());
                    let __toml: toml::Value =
                        toml::from_str(text).map_err(|error| error.to_string())?;
                    let __doc: serde_json::Value =
                        serde_json::to_value(__toml).map_err(|error| error.to_string())?;
                    for __row_ref in crate::source::json_rows(&__doc, #rows) {
                        let __row = __row_ref.value;
                        let __context = &__row_ref.context;
                        let __str = |path: &str| crate::source::string_at(__row, path);
                        let __num = |path: &str| crate::source::number_at(__row, path);
                        let __has = |path: &str| crate::source::lookup(__row, path).is_some();
                        let __ctx = |index: usize| {
                            __context.get(index).and_then(crate::source::context_text)
                        };
                        #body
                    }
                    #dedupe
                    Ok(loaded)
                }
            }))
        }
        Parse::Directory { adapter } => {
            if adapter != "criterion" {
                return Err(format!("unknown directory adapter {adapter:?}"));
            }
            Ok(ParseTokens::Directory(quote! {
                fn parse_file(_: &str) -> Result<crate::input::Loaded, String> {
                    Err("directory source; pass the directory path".to_owned())
                }

                fn parse_dir(path: &std::path::Path) -> Result<crate::input::Loaded, String> {
                    let benchmarks = crate::adapters::load_criterion(path)?;
                    let mut loaded = crate::input::Loaded::default();
                    loaded.meta.tool = Some("criterion".to_owned());
                    loaded.benchmarks = benchmarks;
                    Ok(loaded)
                }
            }))
        }
    }
}

fn row_body(fields: &Fields) -> Result<TokenStream, String> {
    let id = gen_text(&fields.id)?;
    let value = gen_num(&fields.value)?;
    let unit = gen_text(&fields.unit)?;
    let label = match &fields.label {
        Some(expr) => {
            let expr = gen_text(expr)?;
            quote!(__benchmark.label = #expr;)
        }
        None => quote!(),
    };
    let group = match &fields.group {
        Some(expr) => {
            let expr = gen_text(expr)?;
            quote!(__benchmark.group = #expr;)
        }
        None => quote!(),
    };
    let note = match &fields.note {
        Some(expr) => {
            let expr = gen_text(expr)?;
            quote!(__benchmark.note = #expr;)
        }
        None => quote!(),
    };
    let direction = match &fields.direction {
        Some(Expr::Var(value)) if value == "lower" => {
            quote!(__benchmark.direction = crate::model::Direction::Lower;)
        }
        Some(Expr::Var(value)) if value == "higher" => {
            quote!(__benchmark.direction = crate::model::Direction::Higher;)
        }
        Some(Expr::Var(value)) if value == "unit" => quote!(),
        Some(other) => return Err(format!("invalid direction {other:?}")),
        None => quote!(),
    };
    let params = param_tokens(&fields.params, true)?;
    let metrics = param_tokens(&fields.metrics, false)?;
    let stats = stat_tokens(&fields.stats)?;
    Ok(quote! {
        let Some(__id) = (#id) else { continue; };
        let Some(__value) = (#value) else { continue; };
        if !__value.is_finite() { continue; }
        let __unit = #unit.unwrap_or_else(|| "ops/s".to_owned());
        let mut __benchmark = crate::source::row(__id, __value, &__unit);
        #label
        #group
        #note
        #direction
        #params
        #metrics
        #stats
        loaded.benchmarks.push(__benchmark);
    })
}

fn json_body(fields: &Fields) -> Result<TokenStream, String> {
    row_body(fields)
}

fn param_tokens(pairs: &[(String, Expr)], allow_text: bool) -> Result<TokenStream, String> {
    let mut out = TokenStream::new();
    for (name, expr) in pairs {
        if is_num_expr(expr) {
            let value = gen_num(expr)?;
            out.extend(quote! {
                if let Some(__value) = #value {
                    __benchmark.params.insert(#name.to_owned(), __value.into());
                }
            });
        } else if allow_text {
            let value = gen_text(expr)?;
            out.extend(quote! {
                if let Some(__value) = #value {
                    __benchmark.params.insert(#name.to_owned(), __value.into());
                }
            });
        } else {
            return Err(format!("metric {name} must be numeric"));
        }
    }
    Ok(out)
}

fn stat_tokens(pairs: &[(String, Expr)]) -> Result<TokenStream, String> {
    let mut out = TokenStream::new();
    for (name, expr) in pairs {
        let value = gen_num(expr)?;
        let field = match name.as_str() {
            "mean" => quote!(__benchmark.stats.mean = Some(value);),
            "median" => quote!(__benchmark.stats.median = Some(value);),
            "min" => quote!(__benchmark.stats.min = Some(value);),
            "max" => quote!(__benchmark.stats.max = Some(value);),
            "stddev" => quote!(__benchmark.stats.stddev = Some(value);),
            "samples" => quote!(__benchmark.stats.samples = Some(value as u32);),
            "rme_pct" => quote!(__benchmark.stats.rme_pct = Some(value);),
            other => return Err(format!("unknown stat {other}")),
        };
        out.extend(quote! {
            if let Some(value) = #value {
                #field
            }
        });
    }
    Ok(out)
}

fn is_num_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Num(_) => true,
        Expr::Binary(_, left, right) => is_num_expr(left) && is_num_expr(right),
        Expr::Call(name, args) => match name.as_str() {
            "num" | "time_scale" | "percent" => true,
            "first" => args.iter().all(is_num_expr),
            "if" => args.get(1).is_some_and(is_num_expr) && args.get(2).is_some_and(is_num_expr),
            _ => false,
        },
        _ => false,
    }
}

fn gen_text(expr: &Expr) -> Result<TokenStream, String> {
    match expr {
        Expr::Str(value) => Ok(quote!(Some(#value.to_owned()))),
        Expr::Num(value) => Ok(quote!(Some(#value.to_string()))),
        Expr::Call(name, args) => match name.as_str() {
            "str" => {
                let path = text_arg(args, 0)?;
                Ok(quote!(__str(#path)))
            }
            "ctx" => {
                let index = num_arg(args, 0)?;
                Ok(quote!(__ctx(#index as usize)))
            }
            "first" => {
                let values = args.iter().map(gen_text).collect::<Result<Vec<_>, _>>()?;
                Ok(quote!(crate::source::first_str(vec![#(#values),*])))
            }
            "if" => {
                let condition = gen_bool(args.first().ok_or("if needs a condition")?)?;
                let yes = gen_text(args.get(1).ok_or("if needs a value")?)?;
                let no = gen_text(args.get(2).ok_or("if needs an else")?)?;
                Ok(quote!(if #condition { #yes } else { #no }))
            }
            "concat" => {
                let values = args.iter().map(gen_text).collect::<Result<Vec<_>, _>>()?;
                let parts = values
                    .into_iter()
                    .map(|value| quote!(#value.unwrap_or_default()))
                    .collect::<Vec<_>>();
                Ok(quote!(Some([#(#parts),*].concat())))
            }
            "last" | "drop_last" | "strip_prefix" => {
                let value = gen_text(args.first().ok_or("missing text argument")?)?;
                let separator = gen_text(args.get(1).ok_or("missing separator")?)?;
                let call = match name.as_str() {
                    "last" => quote!(crate::source::last(&value, &separator).to_owned()),
                    "drop_last" => quote!(crate::source::drop_last(&value, &separator).to_owned()),
                    _ => quote!(value.strip_prefix(&separator).unwrap_or(&value).to_owned()),
                };
                Ok(quote! {
                    match (#value, #separator) {
                        (Some(value), Some(separator)) => Some(#call),
                        _ => None,
                    }
                })
            }
            "num" => {
                let value = gen_num(expr)?;
                Ok(quote!(#value.map(|value| value.to_string())))
            }
            "time_scale" | "percent" => {
                let value = gen_num(expr)?;
                Ok(quote!(#value.map(|value| value.to_string())))
            }
            other => Err(format!("{other} is not a text expression")),
        },
        Expr::Var(value) => Ok(quote!(Some(#value.to_owned()))),
        _ => Err("expected text expression".to_owned()),
    }
}

fn gen_num(expr: &Expr) -> Result<TokenStream, String> {
    match expr {
        Expr::Num(value) => Ok(quote!(Some(#value))),
        Expr::Call(name, args) => match name.as_str() {
            "num" => {
                let path = text_arg(args, 0)?;
                Ok(quote!(__num(#path)))
            }
            "ctx" => {
                let index = num_arg(args, 0)?;
                Ok(quote!(__ctx(#index as usize).and_then(|value| value.parse::<f64>().ok())))
            }
            "first" => {
                let values = args.iter().map(gen_num).collect::<Result<Vec<_>, _>>()?;
                Ok(quote!(crate::source::first_num(vec![#(#values),*])))
            }
            "if" => {
                let condition = gen_bool(args.first().ok_or("if needs a condition")?)?;
                let yes = gen_num(args.get(1).ok_or("if needs a value")?)?;
                let no = gen_num(args.get(2).ok_or("if needs an else")?)?;
                Ok(quote!(if #condition { #yes } else { #no }))
            }
            "time_scale" => {
                let unit = gen_text(args.first().ok_or("time_scale needs a unit")?)?;
                Ok(quote!(#unit.map(|unit| crate::source::time_scale(&unit))))
            }
            "percent" => {
                let value = gen_text(args.first().ok_or("percent needs text")?)?;
                Ok(quote!(#value.map(|value| crate::source::percent(&value))))
            }
            "str" => {
                let value = gen_text(expr)?;
                Ok(quote!(#value.and_then(|value| value.parse::<f64>().ok())))
            }
            other => Err(format!("{other} is not a numeric expression")),
        },
        Expr::Binary(op, left, right) => {
            let left = gen_num(left)?;
            let right = gen_num(right)?;
            let op = match op {
                '+' => quote!(+),
                '-' => quote!(-),
                '*' => quote!(*),
                '/' => quote!(/),
                other => return Err(format!("unknown numeric operator {other}")),
            };
            Ok(quote! {
                match (#left, #right) {
                    (Some(left), Some(right)) => Some(left #op right),
                    _ => None,
                }
            })
        }
        _ => Err("expected numeric expression".to_owned()),
    }
}

fn gen_bool(expr: &Expr) -> Result<TokenStream, String> {
    match expr {
        Expr::Bool(value) => Ok(quote!(#value)),
        Expr::Call(name, args) if name == "has" => {
            let path = text_arg(args, 0)?;
            Ok(quote!(__has(#path)))
        }
        other => Err(format!("expected boolean expression, got {other:?}")),
    }
}

fn text_arg(args: &[Expr], index: usize) -> Result<String, String> {
    match args.get(index) {
        Some(Expr::Str(value)) => Ok(value.clone()),
        _ => Err("path argument must be a string literal".to_owned()),
    }
}

fn num_arg(args: &[Expr], index: usize) -> Result<f64, String> {
    match args.get(index) {
        Some(Expr::Num(value)) => Ok(*value),
        _ => Err("index argument must be a number literal".to_owned()),
    }
}

fn dedupe_tokens(dedupe: Option<&str>) -> Result<TokenStream, String> {
    match dedupe {
        Some("max") => Ok(quote! {
            loaded.benchmarks = crate::source::dedupe_max(loaded.benchmarks);
        }),
        Some(other) => Err(format!("unknown dedupe {other:?}")),
        None => Ok(TokenStream::new()),
    }
}
