# OOPS Language Specification v2

- Spec ID: `oops-v2`
- Language version: `2`
- Status: normative
- Compiler: `benchcat-source-gen/compiler/v2`
- File extension: `.oops`
- Keywords: MUST, MUST NOT, SHALL, SHALL NOT, SHOULD, MAY (RFC 2119)

## 1. Scope

Version 2 defines multi-format table sources. A single `source` MAY accept several table
representations, such as BenchmarkDotNet Markdown and CSV, and map each representation to one canonical
record shape.

Version 2 does not replace v1. `@version 1` files remain compiled by the frozen v1 compiler.

## 2. Version declaration

```text
@version 2
```

1. `@version 2` MUST be the first non-empty, non-comment line.
2. `@version` MUST be followed by exactly one ASCII space and the decimal digit `2`.
3. The remainder of the file MUST parse as `source` (Section 4).

## 3. Lexical grammar

```pest
WHITESPACE = _{ " " | "\t" | "\r" | "\n" }
COMMENT = _{ "//" ~ (!"\n" ~ ANY)* }
ident = @{ (ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_")* }
string = ${ "\"" ~ (("\\" ~ ANY) | (!"\"" ~ ANY))* ~ "\"" }
number = @{ "-"? ~ ASCII_DIGIT+ ("." ~ ASCII_DIGIT+)? (("e"|"E") ("+"|"-")? ASCII_DIGIT+)? }
```

`string` escapes: `\"`, `\\`, `\n`, `\r`, `\t`. Identifiers are case-sensitive.

## 4. Source grammar

```pest
source = { SOI ~ "source" ~ ident ~ aliases? ~ extensions? ~ "{" ~ item* ~ "}" ~ EOI }
aliases = { "aliases" ~ string_array }
extensions = { "extensions" ~ string_array }
string_array = { "[" ~ (string ~ ("," ~ string)*)? ~ ","? ~ "]" }

item = { inputs | rows | params | metrics | stats }

inputs = { "inputs" ~ "{" ~ input_variant+ ~ "}" }
input_variant = { variant_kind ~ "{" ~ accept ~ record ~ "}" }
variant_kind = { "markdown" | "csv" }
accept = { "accept" ~ "header" ~ string ~ ";" }
record = { "record" ~ "{" ~ pair* ~ "}" }

rows = { "rows" ~ "{" ~ row_field* ~ "}" }
row_field = { row_name ~ ":" ~ expr ~ ";" }
row_name = { "id" | "value" | "unit" | "label" | "group" | "note" | "direction" | "where" }

params = { "params" ~ "{" ~ pair* ~ "}" }
metrics = { "metrics" ~ "{" ~ pair* ~ "}" }
stats = { "stats" ~ "{" ~ pair* ~ "}" }
pair = { ident ~ ":" ~ expr ~ ";" }
```

Requirements:

1. Exactly one `inputs` item MUST be present.
2. Exactly one `rows` item MUST be present.
3. `params`, `metrics` and `stats` MUST occur at most once each.
4. `inputs` MUST contain at least one variant.
5. Each variant MUST contain exactly one `accept` and exactly one `record`.
6. `rows` MUST contain `id`, `value` and `unit`.
7. `rows.direction` MUST be `unit`, `lower` or `higher`.

## 5. Multi-format semantics

### 5.1 Variant order

Variants are tested in declaration order. The first `accept` match selects the parser.

### 5.2 `accept header "value"`

`accept` is true iff one of the first eight input lines contains `value` as a substring.

### 5.3 `markdown`

The input is a Markdown table:

1. a header row beginning with `|` and containing `Mean`;
2. a separator row containing `---`;
3. data rows beginning with `|`.

Header and data cells are trimmed. Header names are matched exactly.

### 5.4 `csv`

The input is parsed with `csv::Reader`:

1. a UTF-8 BOM is removed;
2. the first record is the header;
3. one table row per data record.

### 5.5 Table cells

- `cell(name)` returns the trimmed cell for an exact header name.
- `measurement_value(name)` parses the first numeric token of the cell.
- `measurement_unit(name)` returns the first token in `ns`, `us`, `µs`, `ms`, `s`.
- A missing or empty cell is missing.

### 5.6 Record

Each variant's `record` block maps table cells to canonical fields. All variants MUST define the
canonical fields required by `rows`.

Canonical numeric fields:

```text
value, mean, median, min, max, stddev, error, samples, rme_pct
```

All other record fields are text.

## 6. Canonical row construction

For each table row:

1. Evaluate `where`; if present and false, skip.
2. Evaluate `id`; missing or empty skips.
3. Evaluate `value`; missing or non-finite skips.
4. Evaluate `unit`; missing becomes `ops/s`.
5. Normalize the unit (`ns` -> `ns/op`, `us` -> `us/op`, `ms` -> `ms/op`, `s` -> `s/op`).
6. Construct a `Benchmark`; label defaults to `id`.
7. Set `label`, `group`, `note` when present.
8. Set `direction`: `lower`, `higher`, or inferred from a `/op` unit.
9. Evaluate `params`, `metrics`, `stats`.

## 7. Expressions

```pest
expr = { coalesce ~ ("|>" ~ call)* }
coalesce = { primary ~ ("??" ~ primary)* }
primary = { "(" ~ expr ~ ")" | call | string | number | boolean | ident }
call = { func ~ "(" ~ (expr ~ ("," ~ expr)*)? ~ ")" }
func = @{
    "cell" | "measurement_value" | "measurement_unit" |
    "to_number" | "to_text" | "has" | "if" | "concat" |
    "strip_prefix" | "last" | "drop_last" | "time_scale" | "percent"
}
boolean = { "true" | "false" }
```

### 7.1 `x |> f(a)` 

`x |> f(a)` is exactly `f(x, a)`. The pipe inserts the left value as the first argument.

### 7.2 `a ?? b`

`a ?? b` evaluates `a`; if present, result is `a`; otherwise result is `b`.

### 7.3 Functions

| Function | Result |
|---|---|
| `cell(name)` | `Option<Text>` |
| `measurement_value(name)` | `Option<Number>` |
| `measurement_unit(name)` | `Option<Text>` |
| `to_number(value)` | `Option<Number>` |
| `to_text(value)` | `Option<Text>` |
| `has(value)` | `Bool` |
| `if(cond, a, b)` | type of `a`/`b` |
| `concat(...)` | `Option<Text>` |
| `strip_prefix(value, prefix)` | `Option<Text>` |
| `last(value, separator)` | `Option<Text>` |
| `drop_last(value, separator)` | `Option<Text>` |
| `time_scale(value)` | `Option<Number>` |
| `percent(value)` | `Option<Number>` |

## 8. Code generation

1. The compiler MUST emit `pub static SOURCE: crate::source::SourceDef`.
2. It MUST emit `fn detect(text: &str) -> bool` and `fn parse(text: &str) -> Result<Loaded, String>`.
3. `detect` MUST be true iff any variant's `accept` matches.
4. `parse` MUST select the first matching variant and parse that table.
5. No runtime expression evaluator, AST interpreter or source metadata lookup is permitted.
6. Table parsing MUST use `crate::table`.
7. Unit normalization MAY use `crate::source::row`.

## 9. Diagnostics

| Condition | Phase |
|---|---|
| missing or malformed `@version 2` | build |
| syntax error | build |
| missing/duplicate `inputs` or `rows` | build |
| empty `inputs` | build |
| missing `accept` or `record` | build |
| missing `rows.id`/`value`/`unit` | build |
| invalid `direction` | build |
| unknown function or table field | build |
| invalid regex/string/expression | build |

## 10. Conformance

A conforming v2 implementation MUST:

1. accept every construct in this specification;
2. reject every construct not in this specification;
3. select the compiler by exact `@version 2`;
4. preserve all v1 files and the v1 compiler;
5. pass v2 compiler unit tests and BenchmarkDotNet Markdown/CSV integration tests;
6. remain frozen after `source-compiler-v2` is tagged.
