# OOPS Language Specification v1

- Spec ID: `oops-v1`
- Language version: `1`
- Status: normative
- Compiler: `benchcat-source-gen/compiler/v1`
- File extension: `.oops`
- Normative keywords: MUST, MUST NOT, SHALL, SHALL NOT, SHOULD, MAY (RFC 2119)

## 1. Scope

This specification defines the syntax, static semantics and Rust code-generation contract of OOPS
version 1. It does not define compiler version 2 or later.

## 2. Version declaration

### 2.1 Syntax

```text
version-directive = "@version" SP DIGIT+ EOL
```

### 2.2 Requirements

1. `version-directive` MUST be the first non-empty, non-comment line.
2. `SP` MUST be one or more ASCII space characters.
3. `DIGIT+` MUST be the decimal representation of `1`.
4. The remainder of the file MUST parse as `source` (Section 4).
5. A file that does not satisfy 1–4 MUST be rejected at build time.

## 3. Lexical grammar

```pest
WHITESPACE = _{ " " | "\t" | "\r" | "\n" }
COMMENT = _{ "//" ~ (!"\n" ~ ANY)* }
ident = @{ (ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_" | "-")* }
string = ${ "\"" ~ (("\\" ~ ANY) | (!"\"" ~ ANY))* ~ "\"" }
regex = ${ "/" ~ (("\\" ~ ANY) | (!"/" ~ ANY))* ~ "/" }
number = @{ "-"? ~ ASCII_DIGIT+ ~ ("." ~ ASCII_DIGIT+)? ~ (("e" | "E") ~ ("+" | "-")? ~ ASCII_DIGIT+)? }
```

- `//` starts a comment that ends at `\n`.
- `string` supports the escapes `\"`, `\\`, `\n`, `\r`, `\t`.
- `regex` is a UTF-8 regular expression accepted by the `regex` crate.
- Identifiers are case-sensitive.

## 4. Source grammar

```pest
source = { SOI ~ "source" ~ ident ~ "{" ~ item* ~ "}" ~ EOI }
item = { aliases | extensions | detect | parse | dedupe }

aliases = { "aliases" ~ string_array ~ ";" }
extensions = { "extensions" ~ string_array ~ ";" }
string_array = { "[" ~ (string ~ ("," ~ string)*)? ~ ","? ~ "]" }

detect = { "detect" ~ (text_detect | prefix_detect | contains_detect | any_detect | json_detect | csv_detect | toml_detect | directory_detect) ~ ";" }
text_detect = { "text" ~ regex }
prefix_detect = { "prefix" ~ string }
contains_detect = { "contains" ~ string }
any_detect = { "any" ~ string_array }
json_detect = { "json" ~ string }
csv_detect = { "csv" ~ string }
toml_detect = { "toml" ~ string }
directory_detect = { "directory" ~ ident }

parse = { "parse" ~ (line_parse | records_parse | columns_parse | text_parse | json_parse | jsonl_parse | csv_parse | toml_parse | directory_parse) }
line_parse = { "line" ~ "{" ~ "name" ~ ":" ~ name_mode ~ ";" ~ "extract" ~ extract_list ~ parse_item* ~ "}" }
records_parse = { "records" ~ "{" ~ "start" ~ ":" ~ string ~ ";" ~ "name" ~ ":" ~ name_mode ~ ";" ~ "extract" ~ extract_list ~ parse_item* ~ "}" }
name_mode = { "rest" | "before" ~ string | "after" ~ string }
extract_list = { "{" ~ (extraction ~ ("," ~ extraction)* ~ ","?)? ~ "}" }
extraction = { ident ~ ":" ~ ("number_before" | "number_after" | "unit_after") ~ string }
columns_parse = { "columns" ~ "{" ~ "split" ~ ":" ~ ident ~ ";" ~ "require_first" ~ ":" ~ string ~ ";" ~ "columns" ~ columns_list ~ parse_item* ~ "}" }
columns_list = { "{" ~ (column ~ ("," ~ column)* ~ ","?)? ~ "}" }
column = { ident ~ ":" ~ number }
text_parse = { "text" ~ "{" ~ "rows" ~ regex ~ ";" ~ parse_item* ~ "}" }
json_parse = { "json" ~ "{" ~ "rows" ~ string ~ ";" ~ parse_item* ~ "}" }
jsonl_parse = { "jsonl" ~ "{" ~ "rows" ~ string ~ ";" ~ parse_item* ~ "}" }
csv_parse = { "csv" ~ "{" ~ parse_item* ~ "}" }
toml_parse = { "toml" ~ "{" ~ "rows" ~ string ~ ";" ~ parse_item* ~ "}" }
directory_parse = { "directory" ~ ident ~ "{" ~ "adapter" ~ "=" ~ ident ~ ";" ~ "}" }

parse_item = { field | block }
field = { field_name ~ ":" ~ expr ~ ";" }
field_name = { "id" | "label" | "group" | "note" | "value" | "unit" | "direction" }
block = { block_name ~ "{" ~ (pair ~ ("," ~ pair)* ~ ","?)? ~ "}" }
block_name = { "params" | "metrics" | "stats" }
pair = { ident ~ ":" ~ expr }
dedupe = { "dedupe" ~ "=" ~ ident ~ ";" }
```

### 4.1 Source name

`source` name MUST be a non-empty identifier. The file stem SHOULD be a Rust identifier and is used as
the generated module name.

### 4.2 Required items

- Exactly one `detect` item MUST be present.
- Exactly one `parse` item MUST be present.
- `aliases` and `extensions` are optional.
- `dedupe` is optional and MUST be `max` when present.
- `params`, `metrics` and `stats` MAY occur at most once each.

### 4.3 Directory form

If `parse` is `directory`, then:

1. `detect` MUST be `directory <adapter>;`.
2. `adapter` MUST equal `criterion`.
3. `id`, `value` and `unit` are NOT required.
4. The generated `SourceDef` MUST have `detect = None`, `detect_dir = Some`, `parse = parse_file`,
   `parse_dir = Some`.

## 5. Expression grammar

```pest
expr = { term ~ (add_op ~ term)* }
term = { factor ~ (mul_op ~ factor)* }
factor = { "(" ~ expr ~ ")" | call | string | number | boolean | ident }
add_op = { "+" | "-" }
mul_op = { "*" | "/" }
call = { func ~ "(" ~ (expr ~ ("," ~ expr)*)? ~ ")" }
func = @{
    "strip_prefix" | "drop_last" | "time_scale" | "percent" |
    "concat" | "first" | "num" | "str" | "if" | "has" | "ctx" | "last"
}
boolean = { "true" | "false" }
```

### 5.1 Expression types

| Expression | Result type |
|---|---|
| `number` | `Number` |
| `string` | `Text` |
| `true`, `false` | `Bool` |
| `ident` | `Symbol` (only valid for `direction`) |
| `str(path)` | `Option<Text>` |
| `num(path)` | `Option<Number>` |
| `ctx(index)` | `Option<Text>` |
| `first(...)` | `Option<T>` where all arguments are `Option<T>` |
| `if(cond, a, b)` | `Option<T>` where `a` and `b` are the same `T` |
| `has(path)` | `Bool` |
| `concat(...)` | `Option<Text>` |
| `last(a, b)` | `Option<Text>` |
| `drop_last(a, b)` | `Option<Text>` |
| `strip_prefix(a, b)` | `Option<Text>` |
| `time_scale(a)` | `Option<Number>` |
| `percent(a)` | `Option<Number>` |
| `a + b`, `a - b`, `a * b`, `a / b` | `Option<Number>` |

### 5.2 Expression constraints

1. `path` arguments to `str`, `num`, `has` MUST be string literals.
2. `ctx` index MUST be a non-negative integer literal.
3. `first` MUST contain at least one argument.
4. `if` MUST contain exactly three arguments.
5. `last`, `drop_last` and `strip_prefix` MUST contain exactly two arguments.
6. `time_scale` and `percent` MUST contain exactly one argument.
7. A `direction` field MUST be one of `unit`, `lower`, `higher`.
8. A `params` value MAY be `Option<Text>` or `Option<Number>`.
9. A `metrics` or `stats` value MUST be `Option<Number>`.

## 6. Parsing semantics

### 6.1 `detect text /regex/`

`detect` returns true iff the compiled regex matches the input text.

### 6.2 `detect prefix "value"`

`detect` returns true iff any input line, after left-trimming, starts with `value`.

### 6.3 `detect json "path"`

The input MUST parse as JSON. `detect` returns true iff `path` exists.

### 6.4 `detect csv "header"`

`detect` returns true iff the first input line contains `header` as a substring.

### 6.5 `detect toml "path"`

The input MUST parse as TOML. The value is converted to JSON. `detect` returns true iff `path` exists.

### 6.6 `detect directory criterion`

`detect_dir` delegates to `benchcat::input::is_criterion_dir`.

### 6.7 `parse columns`

| Property | Semantics |
|---|---|
| `split` | `whitespace` (split on Unicode whitespace), `comma` (split on `,`, trim each field), `tab` (split on `\t`) |
| `require_first` | Empty string disables filtering. Otherwise the first column MUST start with this value. |
| `columns` | Zero-based column index per name. |
| missing column | `str`/`num` yield missing; `has` yields false. |
| rows | One row per non-empty input line that passes `require_first`. |

### 6.8 `parse text`

One row per regex match. Named capture `name` is exposed as a text path. Unnamed captures are not
exposed.

### 6.9 `parse json`

`rows` is a path. The root value is the parsed JSON document. A row is produced for each selected value.

### 6.10 `parse jsonl`

The input is split into lines. Lines that do not parse as JSON are skipped. A UTF-8 BOM is removed from
each line. For each parsed document, `rows` is applied as in Section 6.9.

### 6.11 `parse csv`

The input is parsed with `csv::Reader`:

1. Delimiter is `\t` iff the first line contains `\t`; otherwise `,`.
2. The first record is the header.
3. Field names map to exact header strings.
4. One row is produced per data record.

### 6.12 `parse toml`

The input MUST parse as TOML. It is converted to JSON. `rows` is applied as in Section 6.9.

### 6.13 `parse directory criterion`

The input MUST be a directory. The parser delegates to `benchcat::adapters::load_criterion`.

### 6.14 `parse line`

One row per non-empty line.

| `name` | Semantics |
|---|---|
| `rest` | whole trimmed line |
| `before "marker"` | text before the first marker |
| `after "marker"` | text after the first marker |

`extract` fields are evaluated per line:

| Extraction | Semantics |
|---|---|
| `number_before "marker"` | last number before the first marker |
| `number_after "marker"` | first number after the first marker |
| `unit_after "marker"` | first known time unit after the first marker (`ns`, `us`, `µs`, `ms`, `s`) |

### 6.15 `parse records`

Records are split at every line that starts with `start` (column 0). The start line belongs to the new
record; following lines belong to it.

`name` uses `rest`, `before "marker"`, or `after "marker"` on the start line.

`extract` uses the same extraction operators as `parse line`, but searches the whole record.

### 6.16 `detect contains "value"`

Returns true iff the input text contains `value`.

### 6.17 `detect any ["a", "b"]`

Returns true iff the input text contains any listed value.

## 7. Path semantics

```text
path      = "$" segment*
segment   = "." ( key | index | "*" )
key       = [A-Za-z0-9_-]+
index     = DIGIT+
```

1. `$` selects the root.
2. `.key` selects an object field.
3. `.index` selects an array element.
4. `.*` selects every object value or array element.
5. A path that does not resolve yields missing.
6. `*` contributes a context value: object key as text, array index as number.

## 8. Row construction

For every row:

1. Evaluate `id`. If missing or empty, skip the row.
2. Evaluate `value`. If missing or non-finite, skip the row.
3. Evaluate `unit`. If missing, use `ops/s`.
4. Construct `Benchmark` with `id`, `value`, normalized `unit`.
5. Set `label`, `group`, `note` when their expressions produce a value. If `label` is absent, `label` MUST default to `id`.
6. Set `direction`:
   - `lower` -> `Direction::Lower`;
   - `higher` -> `Direction::Higher`;
   - `unit` or absent -> lower iff unit ends with `/op`, otherwise higher.
7. Evaluate `params`; insert present values.
8. Evaluate `metrics`; insert present numeric values.
9. Evaluate `stats`; assign `mean`, `median`, `min`, `max`, `stddev`, `samples`, `rme_pct`.

### 8.1 Unit normalization

| Input | Output |
|---|---|
| empty | `ops/s` |
| `ns`, `nanoseconds` | `ns/op` |
| `us`, `µs`, `microseconds` | `us/op` |
| `ms`, `milliseconds` | `ms/op` |
| `s`, `sec`, `seconds` | `s/op` |
| other | unchanged (trimmed) |

### 8.2 Dedupe

`dedupe = max;` MUST group rows by `id` and retain the row with the maximum `value`. The output order
MUST be the first occurrence order of each `id`.

## 9. Code-generation contract

1. The compiler MUST emit `pub static SOURCE: crate::source::SourceDef`.
2. `parse` MUST be a Rust function with signature
   `fn parse(text: &str) -> Result<crate::input::Loaded, String>`.
3. The generated code MUST NOT contain a generic expression evaluator, AST interpreter, runtime path
   resolver driven by dynamic opcodes, or source metadata lookup.
4. Generated regexes MUST be compiled once via `OnceLock<Regex>`.
5. Generated code MAY call helper functions in `crate::source` for path traversal, number conversion,
   unit normalization and dedupe.
6. The generated code MUST compile with the workspace Rust edition and toolchain.

## 10. Diagnostics

| Condition | Phase |
|---|---|
| missing or malformed `@version` | build |
| unsupported version | build |
| syntax error | build |
| unknown directive, field, block, function | build |
| missing required `detect`/`parse`/`id`/`value`/`unit` | build |
| invalid regex | build |
| invalid expression type | build |
| invalid `direction` or `dedupe` | build |
| invalid `split` | build |
| unsupported directory adapter | build |

## 11. Conformance

A conforming implementation MUST:

1. accept every construct in this specification;
2. reject every construct not in this specification;
3. select the compiler by exact `@version`;
4. pass the v1 fixture suite and generated-code snapshots;
5. retain a frozen v1 compiler after release.
