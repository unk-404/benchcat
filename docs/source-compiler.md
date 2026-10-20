# Source Compiler Documentation Index

Normative documents:

- [OOPS Language Specification v1](spec/oops-v1.md) — syntax, semantics, diagnostics, code-generation contract.
- [Compiler Versioning Specification](spec/compiler-versioning.md) — version selection, dependency direction, immutability, fixtures, tags.

Normative source directory:

- [`sources/*.oops`](../sources/) — one file per source, each starting with `@version N`.

Implementation:

- `crates/source-gen/src/lib.rs` — scans `.oops`, validates `@version`, dispatches.
- `crates/source-gen/src/compiler/mod.rs` — version dispatch.
- `crates/source-gen/src/compiler/v1/` — frozen v1 grammar, AST, checker and codegen.

Translation:

- `.oops` → pest AST → checks → Rust `TokenStream` → `rustc`.
- No runtime DSL parser, AST, expression evaluator or metadata lookup.
