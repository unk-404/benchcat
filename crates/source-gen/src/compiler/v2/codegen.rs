//! V2 code generation: table variants compiled to direct Rust.

use proc_macro2::TokenStream;
use quote::quote;

use super::ast::{Expr, Input, RowFields, Source};

pub(crate) fn compile(source: &Source) -> Result<TokenStream, String> {
    let name = &source.name;
    let aliases = &source.aliases;
    let extensions = &source.extensions;
    let detect = detect_fn(&source.inputs);
    let parse = parse_fn(source)?;
    Ok(quote! {
        pub static SOURCE: crate::source::SourceDef = crate::source::SourceDef {
            name: #name,
            aliases: &[#(#aliases),*],
            extensions: &[#(#extensions),*],
            detect: Some(detect),
            parse,
            detect_dir: None,
            parse_dir: None,
        };

        #detect
        #parse
    })
}

fn detect_fn(inputs: &[Input]) -> TokenStream {
    let headers: Vec<&String> = inputs.iter().map(|input| &input.accept_header).collect();
    quote! {
        fn detect(text: &str) -> bool {
            [#(#headers),*]
                .iter()
                .any(|header| crate::table::header_contains(text, header))
        }
    }
}

fn parse_fn(source: &Source) -> Result<TokenStream, String> {
    let row_body = row_body(&source.rows, &source.params, &source.metrics, &source.stats)?;
    let mut branches = Vec::new();
    for (index, input) in source.inputs.iter().enumerate() {
        let header = &input.accept_header;
        let table = match input.kind.as_str() {
            "markdown" => quote!(crate::table::parse_markdown_table(text)),
            "csv" => quote!(crate::table::parse_csv_table(text)),
            other => return Err(format!("unknown input kind {other:?}")),
        };
        let record = record_bindings(&input.record)?;
        let condition = if index == 0 {
            quote!(if crate::table::header_contains(text, #header))
        } else {
            quote!(else if crate::table::header_contains(text, #header))
        };
        branches.push(quote! {
            #condition {
                let __table = #table.ok_or_else(|| "table did not parse".to_owned())?;
                for __row in &__table.rows {
                    let __cell = |name: &str| __table.cell(__row, name);
                    let __mv = |name: &str| __table.measurement_value(__row, name);
                    let __mu = |name: &str| __table.measurement_unit(__row, name);
                    let __has = |name: &str| __table.cell(__row, name).is_some();
                    #record
                    #row_body
                }
            }
        });
    }
    Ok(quote! {
        fn parse(text: &str) -> Result<crate::input::Loaded, String> {
            let mut loaded = crate::input::Loaded::default();
            loaded.meta.tool = Some("benchmarkdotnet".to_owned());
            #(#branches)*
            else {
                return Err("no table variant matched".to_owned());
            }
            Ok(loaded)
        }
    })
}

fn record_bindings(record: &[(String, Expr)]) -> Result<TokenStream, String> {
    let mut out = TokenStream::new();
    for (name, expr) in record {
        let ident = proc_macro2::Ident::new(name, proc_macro2::Span::call_site());
        if is_number_name(name) {
            let value = gen_num(expr)?;
            out.extend(quote!(let #ident: Option<f64> = #value;));
        } else {
            let value = gen_text(expr)?;
            out.extend(quote!(let #ident: Option<String> = #value;));
        }
    }
    Ok(out)
}

fn row_body(
    rows: &RowFields,
    params: &[(String, Expr)],
    metrics: &[(String, Expr)],
    stats: &[(String, Expr)],
) -> Result<TokenStream, String> {
    let id = gen_text(&rows.id)?;
    let value = gen_num(&rows.value)?;
    let unit = gen_text(&rows.unit)?;
    let filter = match &rows.filter {
        Some(expr) => {
            let condition = gen_bool(expr)?;
            quote!(if !(#condition) { continue; })
        }
        None => quote!(),
    };
    let label = match &rows.label {
        Some(expr) => {
            let expr = gen_text(expr)?;
            quote!(if let Some(value) = #expr { __benchmark.label = Some(value); })
        }
        None => quote!(),
    };
    let group = match &rows.group {
        Some(expr) => {
            let expr = gen_text(expr)?;
            quote!(if let Some(value) = #expr { __benchmark.group = Some(value); })
        }
        None => quote!(),
    };
    let note = match &rows.note {
        Some(expr) => {
            let expr = gen_text(expr)?;
            quote!(if let Some(value) = #expr { __benchmark.note = Some(value); })
        }
        None => quote!(),
    };
    let direction = match &rows.direction {
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
    let mut params_tokens = TokenStream::new();
    for (name, expr) in params {
        if is_num_expr(expr) {
            let value = gen_num(expr)?;
            params_tokens.extend(quote! {
                if let Some(__value) = #value {
                    __benchmark.params.insert(#name.to_owned(), __value.into());
                }
            });
        } else {
            let value = gen_text(expr)?;
            params_tokens.extend(quote! {
                if let Some(__value) = #value {
                    __benchmark.params.insert(#name.to_owned(), __value.into());
                }
            });
        }
    }
    let mut metrics_tokens = TokenStream::new();
    for (name, expr) in metrics {
        let value = gen_num(expr)?;
        metrics_tokens.extend(quote! {
            if let Some(value) = #value {
                __benchmark.metrics.insert(#name.to_owned(), value);
            }
        });
    }
    let mut stats_tokens = TokenStream::new();
    for (name, expr) in stats {
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
        stats_tokens.extend(quote! {
            if let Some(value) = #value {
                #field
            }
        });
    }
    Ok(quote! {
        #filter
        let Some(__id) = (#id) else { continue; };
        if __id.is_empty() { continue; }
        let Some(__value) = (#value) else { continue; };
        if !__value.is_finite() { continue; }
        let __unit = (#unit).unwrap_or_else(|| "ops/s".to_owned());
        let mut __benchmark = crate::source::row(__id, __value, &__unit);
        #label
        #group
        #note
        #direction
        #params_tokens
        #metrics_tokens
        #stats_tokens
        loaded.benchmarks.push(__benchmark);
    })
}

fn gen_text(expr: &Expr) -> Result<TokenStream, String> {
    match expr {
        Expr::Str(value) => Ok(quote!(Some(#value.to_owned()))),
        Expr::Var(name) => {
            let ident = proc_macro2::Ident::new(name, proc_macro2::Span::call_site());
            Ok(quote!(#ident.clone()))
        }
        Expr::Call(name, args) => match name.as_str() {
            "cell" => {
                let field = text_arg(args, 0)?;
                Ok(quote!(__cell(#field)))
            }
            "measurement_unit" => {
                let field = text_arg(args, 0)?;
                Ok(quote!(__mu(#field)))
            }
            "to_text" => gen_text(args.first().ok_or("to_text needs a value")?),
            "strip_prefix" | "last" | "drop_last" => {
                let value = gen_text(args.first().ok_or("missing text value")?)?;
                let sep = gen_text(args.get(1).ok_or("missing separator")?)?;
                let call = match name.as_str() {
                    "strip_prefix" => {
                        quote!(value.strip_prefix(&separator).unwrap_or(&value).to_owned())
                    }
                    "last" => quote!(crate::source::last(&value, &separator).to_owned()),
                    _ => quote!(crate::source::drop_last(&value, &separator).to_owned()),
                };
                Ok(quote! {
                    match (#value, #sep) {
                        (Some(value), Some(separator)) => Some(#call),
                        _ => None,
                    }
                })
            }
            "if" => {
                let cond = gen_bool(args.first().ok_or("if needs a condition")?)?;
                let yes = gen_text(args.get(1).ok_or("if needs a value")?)?;
                let no = gen_text(args.get(2).ok_or("if needs an else")?)?;
                Ok(quote!(if #cond { #yes } else { #no }))
            }
            "concat" => {
                let values = args
                    .iter()
                    .map(gen_text)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .map(|value| quote!(#value.unwrap_or_default()))
                    .collect::<Vec<_>>();
                Ok(quote!(Some([#(#values),*].concat())))
            }
            "to_number" => {
                let value = gen_num(expr)?;
                Ok(quote!(#value.map(|value| value.to_string())))
            }
            "measurement_value" => {
                let value = gen_num(expr)?;
                Ok(quote!(#value.map(|value| value.to_string())))
            }
            other => Err(format!("{other} is not a text expression")),
        },
        Expr::Coalesce(left, right) => {
            let left = gen_text(left)?;
            let right = gen_text(right)?;
            Ok(quote!(#left.or_else(|| #right)))
        }
        Expr::Pipe(left, right) => gen_pipe_text(left, right),
        _ => Err("expected text expression".to_owned()),
    }
}

fn gen_num(expr: &Expr) -> Result<TokenStream, String> {
    match expr {
        Expr::Num(value) => Ok(quote!(Some(#value))),
        Expr::Var(name) => {
            let ident = proc_macro2::Ident::new(name, proc_macro2::Span::call_site());
            Ok(quote!(#ident))
        }
        Expr::Call(name, args) => match name.as_str() {
            "measurement_value" => {
                let field = text_arg(args, 0)?;
                Ok(quote!(__mv(#field)))
            }
            "to_number" => {
                let value = gen_text(args.first().ok_or("to_number needs a value")?)?;
                Ok(quote!(#value.and_then(|value| value.parse::<f64>().ok())))
            }
            "cell" => {
                let value = gen_text(expr)?;
                Ok(quote!(#value.and_then(|value| value.parse::<f64>().ok())))
            }
            "time_scale" | "percent" => {
                let value = gen_text(args.first().ok_or("missing text")?)?;
                let call = if name == "time_scale" {
                    quote!(crate::source::time_scale(&value))
                } else {
                    quote!(crate::source::percent(&value))
                };
                Ok(quote!(#value.map(|value| #call)))
            }
            "if" => {
                let cond = gen_bool(args.first().ok_or("if needs a condition")?)?;
                let yes = gen_num(args.get(1).ok_or("if needs a value")?)?;
                let no = gen_num(args.get(2).ok_or("if needs an else")?)?;
                Ok(quote!(if #cond { #yes } else { #no }))
            }
            other => Err(format!("{other} is not a numeric expression")),
        },
        Expr::Coalesce(left, right) => {
            let left = gen_num(left)?;
            let right = gen_num(right)?;
            Ok(quote!(#left.or_else(|| #right)))
        }
        Expr::Pipe(left, right) => gen_pipe_num(left, right),
        _ => Err("expected numeric expression".to_owned()),
    }
}

fn gen_bool(expr: &Expr) -> Result<TokenStream, String> {
    match expr {
        Expr::Bool(value) => Ok(quote!(#value)),
        Expr::Call(name, args) if name == "has" => match args.first() {
            Some(Expr::Var(name)) => {
                let ident = proc_macro2::Ident::new(name, proc_macro2::Span::call_site());
                Ok(quote!(#ident.is_some()))
            }
            _ => {
                let field = text_arg(args, 0)?;
                Ok(quote!(__has(#field)))
            }
        },
        other => Err(format!("expected boolean, got {other:?}")),
    }
}

fn gen_pipe_text(left: &Expr, right: &Expr) -> Result<TokenStream, String> {
    match right {
        Expr::Call(name, args) => {
            let mut args = args.clone();
            args.insert(0, left.clone());
            gen_text(&Expr::Call(name.clone(), args))
        }
        _ => Err("pipe right side must be a call".to_owned()),
    }
}

fn gen_pipe_num(left: &Expr, right: &Expr) -> Result<TokenStream, String> {
    match right {
        Expr::Call(name, args) => {
            let mut args = args.clone();
            args.insert(0, left.clone());
            gen_num(&Expr::Call(name.clone(), args))
        }
        _ => Err("pipe right side must be a call".to_owned()),
    }
}

fn is_num_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Num(_) => true,
        Expr::Var(name) => is_number_name(name),
        Expr::Call(name, args) => match name.as_str() {
            "measurement_value" | "to_number" | "time_scale" | "percent" => true,
            "if" => args.get(1).is_some_and(is_num_expr) && args.get(2).is_some_and(is_num_expr),
            _ => false,
        },
        Expr::Coalesce(left, _) => is_num_expr(left),
        Expr::Pipe(left, _) => is_num_expr(left),
        _ => false,
    }
}

fn is_number_name(name: &str) -> bool {
    matches!(
        name,
        "value" | "mean" | "median" | "min" | "max" | "stddev" | "error" | "samples" | "rme_pct"
    )
}

fn text_arg(args: &[Expr], index: usize) -> Result<String, String> {
    match args.get(index) {
        Some(Expr::Str(value)) => Ok(value.clone()),
        _ => Err("table field argument must be a string literal".to_owned()),
    }
}
