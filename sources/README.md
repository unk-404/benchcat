# OOPS Source Directory

- Directory: `sources/`
- File pattern: `*.oops`
- Ignored pattern: `_*.oops`
- Language version: `1`
- Normative specification: [`../docs/spec/oops-v1.md`](../docs/spec/oops-v1.md)
- Versioning specification: [`../docs/spec/compiler-versioning.md`](../docs/spec/compiler-versioning.md)

## File contract

1. Each file MUST begin with `@version 1`.
2. Each file MUST contain exactly one `source <name> { ... }` block.
3. Each file MUST contain exactly one `detect` item.
4. Each file MUST contain exactly one `parse` item.
5. Non-directory sources MUST define `id`, `value` and `unit`.
6. Changes to these files require a rebuild; the macro scans the directory at build time.

## Registry

| File | `--format` | Input |
|---|---|---|
| `androidx.oops` | `androidx` | AndroidX `benchmarkData.json` |
| `benchmark_runner.oops` | `benchmark-runner` | Dart `benchmark_runner` report |
| `benchmark_test_benchmarkjs.oops` | `benchmark-test-benchmarkjs` | Dart `benchmark_test --output=benchmarkjs` |
| `benchmark_test_human.oops` | `benchmark-test-human` | Dart `benchmark_test` human output |
| `criterion.oops` | `criterion` | Criterion directory |
| `go.oops` | `go` | Go `testing` columns output |
| `google.oops` | `google` | Google Benchmark JSON |
| `google_console.oops` | `google-console` | Google Benchmark console columns |
| `jmh.oops` | `jmh` (`kotlinx`) | JMH / kotlinx-benchmark JSON |
| `phpbench.oops` | `phpbench` | PHPBench JSON/JSONL |
| `swift.oops` | `swift` | swift-benchmark JSON |
| `techempower.oops` | `techempower` | TechEmpower `results.json` |

Dart `benchmark_test --output=jsonl` is handled by the built-in `benchmark_test` reader; it is not an
`.oops` file.

## Template

`_template.oops` is ignored by the macro. Copy it to `<name>.oops` and replace the body.
