//! V1 compiler. Frozen once tagged `source-compiler-v1`.

mod ast;
mod codegen;

use proc_macro2::TokenStream;

pub(crate) fn compile(text: &str) -> Result<TokenStream, String> {
    let source = ast::parse(text)?;
    codegen::compile(&source)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"
source test {
    aliases ["test"];
    detect prefix "B";
    parse text {
        rows /^(?P<value>[0-9.]+)$/;
        id: str("value");
        value: num("value");
        unit: "ops/s";
        direction: higher;
    }
}
"#;

    #[test]
    fn compiles_valid_source_to_rust() {
        let tokens = compile(SOURCE).unwrap().to_string();
        assert!(tokens.contains("SourceDef"));
        assert!(tokens.contains("\"test\""));
        assert!(tokens.contains("fn parse"));
    }

    #[test]
    fn compiles_every_container() {
        let sources = [
            r#"source t { detect prefix "B"; parse text { rows /(?P<value>[0-9.]+)/; id: str("value"); value: num("value"); unit: "ops/s"; } }"#,
            r#"source t { detect contains "x"; parse line { name: rest; extract { value: number_before "v" } id: str("name"); value: num("value"); unit: "ops/s"; } }"#,
            r#"source t { detect contains "["; parse records { start: "["; name: after "]"; extract { value: number_after "v" } id: str("name"); value: num("value"); unit: "ops/s"; } }"#,
            r#"source t { detect prefix "x"; parse columns { split: whitespace; require_first: "x"; columns { name: 0, value: 1 } id: str("name"); value: num("value"); unit: "ops/s"; } }"#,
            r#"source t { detect json "$.x"; parse json { rows "$.x"; id: str("name"); value: num("value"); unit: "ops/s"; } }"#,
            r#"source t { detect contains "x"; parse jsonl { rows "$"; id: str("name"); value: num("value"); unit: "ops/s"; } }"#,
            r#"source t { detect csv "name"; parse csv { id: str("name"); value: num("value"); unit: "ops/s"; } }"#,
            r#"source t { detect toml "$.x"; parse toml { rows "$.x"; id: str("name"); value: num("value"); unit: "ops/s"; } }"#,
            r#"source t { detect directory criterion; parse directory criterion { adapter = criterion; } }"#,
        ];
        for source in sources {
            compile(source).unwrap_or_else(|error| panic!("{error}\n{source}"));
        }
    }

    #[test]
    fn rejects_duplicate_detect() {
        let source = SOURCE.replace(
            "detect prefix \"B\";",
            "detect prefix \"B\"; detect prefix \"B\";",
        );
        assert!(compile(&source).is_err());
    }
}
