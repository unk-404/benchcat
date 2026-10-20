//! V2 compiler: table variants with canonical records.

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
source csharp aliases ["csharp"] extensions ["md", "csv"] {
    inputs {
        markdown {
            accept header "| Method |";
            record {
                id: cell("Method");
                value: measurement_value("Mean");
                unit: measurement_unit("Mean");
            }
        }
        csv {
            accept header "Method";
            record {
                id: cell("Method");
                value: measurement_value("Mean");
                unit: measurement_unit("Mean");
            }
        }
    }
    rows {
        id: id;
        value: value;
        unit: unit;
        direction: unit;
        where: has(value);
    }
}
"#;

    #[test]
    fn compiles_multi_format_source() {
        let tokens = compile(SOURCE).unwrap().to_string();
        assert!(tokens.contains("SourceDef"));
        assert!(tokens.contains("parse_markdown_table"));
        assert!(tokens.contains("parse_csv_table"));
    }

    #[test]
    fn rejects_missing_rows() {
        let source = SOURCE.replace(
            "    rows {\n        id: id;\n        value: value;\n        unit: unit;\n        direction: unit;\n        where: has(value);\n    }\n",
            "",
        );
        assert!(compile(&source).is_err());
    }
}
