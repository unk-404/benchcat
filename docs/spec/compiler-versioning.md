# Source Compiler Versioning Specification

- Spec ID: `oops-compiler-versioning`
- Status: normative
- Keywords: MUST, MUST NOT, SHALL, SHALL NOT, SHOULD, MAY (RFC 2119)

## 1. Terminology

- **Compiler version**: a directory `crates/source-gen/src/compiler/vN`.
- **Language version**: the integer `N` in `@version N`.
- **Frozen**: no semantic, syntactic or code-generation changes are permitted.
- **Host compiler**: the compiler selected by a file's `@version`.

## 2. Version declaration

1. Every `.oops` file MUST begin with `@version N`.
2. `N` MUST be a positive decimal integer.
3. `N` MUST identify an existing compiler version.
4. The host compiler MUST be `compiler/vN`.
5. A file with an unsupported `N` MUST be rejected at build time.

## 3. Compiler selection

```rust
match version {
    1 => compiler::v1::compile(source),
    2 => compiler::v2::compile(source),
    n => Err(format!("unsupported .oops version {n}")),
}
```

1. Selection MUST be exact.
2. A newer compiler MUST NOT be used to compile an older language version.
3. An older compiler MUST NOT be extended to accept a newer language version.
4. Compiler selection MUST occur before parsing the source body.

## 4. Dependency direction

```text
shared/        version-neutral utilities
v1/            MUST NOT reference v2/ or any later version
v2/            MAY reference v1/ and shared/
v3/            MAY reference v1/, v2/ and shared/
```

1. `v1` MUST NOT contain the token `v2` in a module path.
2. `v1` MUST NOT depend on a later compiler's AST, grammar, checker or codegen.
3. A later compiler MAY reuse an earlier compiler's stable public items.
4. Reuse MUST NOT mutate earlier compiler behavior.
5. `shared/` MUST be version-neutral. It MUST NOT contain `.oops` syntax, AST nodes, or version-specific
   semantics.

## 5. Immutability

After `source-compiler-vN` is tagged:

1. `compiler/vN/grammar.pest` MUST NOT change.
2. `compiler/vN/ast.rs` MUST NOT change.
3. `compiler/vN/check.rs` MUST NOT change.
4. `compiler/vN/codegen.rs` MUST NOT change.
5. Changes to a frozen version MUST be reverted or moved to a new version.

## 6. Adding a version

1. Create `compiler/vN+1/`.
2. Add a new grammar; reuse of the previous grammar file is forbidden.
3. Add a new AST, checker and codegen.
4. Add `N+1` to the dispatch table.
5. Add fixtures and snapshots for `N+1`.
6. Keep all `v1..vN` directories and fixtures.
7. Tag `source-compiler-vN+1`.

## 7. Fixtures

Each compiler version MUST have compiler unit tests covering:

1. a valid source of every supported container (`text`, `line`, `records`, `columns`, `json`,
   `jsonl`, `csv`, `toml`, `directory`);
2. rejection of malformed directives, expressions, and duplicate `detect`/`parse`/blocks;
3. the generated `SourceDef` token stream for at least one source.

Generated-code snapshots MAY be stored under `compiler/vN/tests/snapshots/`; when present they MUST NOT
be regenerated for a frozen version.

## 8. Enforcement

The workspace MUST contain a test that:

1. scans `compiler/v1/**` and fails on any reference to `v2` or later;
2. scans `sources/*.oops` and fails on missing or unsupported `@version`;
3. verifies every declared version has a matching compiler directory;
4. verifies frozen compiler directories are unchanged relative to their tag.

## 9. Git policy

1. One commit per implementation phase.
2. Tag `source-compiler-vN` only after the version's full fixture suite passes.
3. A version tag MUST NOT be moved.
4. Reverts of a frozen compiler MUST be a new commit, not a force-push.
