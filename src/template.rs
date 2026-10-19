//! Deterministic `{placeholder}` expansion for title, subtitle and footer.
//!
//! Values come only from the input document, the row count and the font name.
//! Nothing reads the clock or the host, so the same inputs always produce the
//! same bytes — which is what makes "commit only when it changed" work.

use crate::model::Meta;

pub struct Context<'a> {
    pub meta: &'a Meta,
    pub rows: usize,
    pub font: &'a str,
}

/// Expand `{name}` placeholders. Unknown names are left verbatim and reported.
///
/// `{{` produces a literal `{`.
pub fn expand(template: &str, context: &Context<'_>) -> (String, Vec<String>) {
    let mut out = String::with_capacity(template.len() + 32);
    let mut warnings = Vec::new();
    let bytes = template.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'{' {
            if bytes.get(index + 1) == Some(&b'{') {
                out.push('{');
                index += 2;
                continue;
            }
            if let Some(offset) = template[index + 1..].find('}') {
                let end = index + 1 + offset;
                let key = &template[index + 1..end];
                match resolve(key, context) {
                    Some(value) => out.push_str(&value),
                    None => {
                        warnings.push(format!("unknown placeholder {{{key}}}"));
                        out.push_str(&template[index..=end]);
                    }
                }
                index = end + 1;
                continue;
            }
        }
        let ch = template[index..]
            .chars()
            .next()
            .expect("index is inside the string");
        if bytes[index] == b'}' && bytes.get(index + 1) == Some(&b'}') {
            out.push('}');
            index += 2;
            continue;
        }
        out.push(ch);
        index += ch.len_utf8();
    }
    (out, warnings)
}

fn resolve(key: &str, context: &Context<'_>) -> Option<String> {
    // `{platform|local}` renders the fallback when the value is empty.
    let (key, fallback) = match key.split_once('|') {
        Some((key, fallback)) => (key, Some(fallback)),
        None => (key, None),
    };
    let meta = context.meta;
    let value = match key {
        "commit" => short(meta.commit.as_deref(), 7),
        "commit:full" => meta.commit.clone().unwrap_or_default(),
        "commit:short" => short(meta.commit.as_deref(), 7),
        "branch" => meta.branch.clone().unwrap_or_default(),
        "tool" => meta.tool.clone().unwrap_or_default(),
        "tool_version" | "version" => meta.tool_version.clone().unwrap_or_default(),
        "tool:full" => match &meta.tool {
            Some(tool) => match &meta.tool_version {
                Some(version) => format!("{tool} {version}"),
                None => tool.clone(),
            },
            None => String::new(),
        },
        "platform" => meta.platform.summary().unwrap_or_default(),
        "os" => meta.platform.os.clone().unwrap_or_default(),
        "os_version" => meta.platform.os_version.clone().unwrap_or_default(),
        "arch" => meta.platform.arch.clone().unwrap_or_default(),
        "cpu" => meta.platform.cpu.clone().unwrap_or_default(),
        "cores" => meta
            .platform
            .cores
            .map(|cores| cores.to_string())
            .unwrap_or_default(),
        "ram_gib" => meta
            .platform
            .ram_gib
            .map(|ram| ram.to_string())
            .unwrap_or_default(),
        "runner" => meta.platform.runner.clone().unwrap_or_default(),
        "rows" => context.rows.to_string(),
        "font" => context.font.to_owned(),
        _ => return None,
    };
    Some(if value.is_empty() {
        fallback.unwrap_or("").to_owned()
    } else {
        value
    })
}

fn short(value: Option<&str>, length: usize) -> String {
    match value {
        Some(value) if !value.is_empty() => value.chars().take(length).collect(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Meta, Platform};

    fn context() -> Context<'static> {
        // Leaks are fine in a test; keeps the borrow simple.
        let meta: &'static Meta = Box::leak(Box::new(Meta {
            tool: Some("benchmark_test".to_owned()),
            tool_version: Some("0.9.1".to_owned()),
            commit: Some("cd5cd87aabbccddee".to_owned()),
            branch: Some("main".to_owned()),
            platform: Platform {
                os: Some("Windows".to_owned()),
                arch: Some("x86_64".to_owned()),
                ..Platform::default()
            },
        }));
        Context {
            meta,
            rows: 10,
            font: "mplus-rounded",
        }
    }

    #[test]
    fn expands_provenance_placeholders() {
        let (text, warnings) = expand(
            "{platform} · {tool} {tool_version} · commit {commit} · {rows} rows",
            &context(),
        );
        assert_eq!(
            text,
            "Windows · x86_64 · benchmark_test 0.9.1 · commit cd5cd87 · 10 rows"
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn keeps_unknown_placeholders_verbatim() {
        let (text, warnings) = expand("commit {commit} {nope}", &context());
        assert_eq!(text, "commit cd5cd87 {nope}");
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn doubles_escape_braces() {
        let (text, warnings) = expand("{{commit}} {commit:full}", &context());
        assert_eq!(text, "{commit} cd5cd87aabbccddee");
        assert!(warnings.is_empty());
    }

    #[test]
    fn missing_values_become_empty() {
        let meta = Meta::default();
        let context = Context {
            meta: &meta,
            rows: 0,
            font: "f",
        };
        let (text, _) = expand("commit {commit}.", &context);
        assert_eq!(text, "commit .");
    }

    #[test]
    fn fallback_renders_only_when_empty() {
        let meta = Meta::default();
        let context = Context {
            meta: &meta,
            rows: 3,
            font: "f",
        };
        let (text, warnings) = expand("{platform|local} · {rows} rows", &context);
        assert_eq!(text, "local · 3 rows");
        assert!(warnings.is_empty());
    }
}
