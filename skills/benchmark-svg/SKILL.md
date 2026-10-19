---
name: benchmark-svg
description: Render benchmark results as a self-contained SVG card for a README with the bench-svg CLI or its GitHub Action. Use when a repository needs a benchmark chart committed as SVG, when wiring criterion, hyperfine, Dart benchmark_test, canonical JSON or CSV into CI, or when comparing the same operation across several platforms/sources.
---

# Benchmark SVG

`bench-svg` reads benchmark output and writes one deterministic SVG card. It never runs benchmarks,
touches git, uploads artifacts or comments on pull requests — those stay in the surrounding workflow.
Repository: <https://github.com/unk-404/benchmark_svg>.

## 60-second path

```bash
# from a checkout of this repository
cargo install --path .

# criterion
bench-svg render target/criterion -o assets/benchmarks.svg --format criterion

# hyperfine
bench-svg render bench.json -o assets/benchmarks.svg --format hyperfine

# anything that can print JSONL/CSV
bench-svg render bench.jsonl -o assets/benchmarks.svg
```

Validate in this order:

```bash
test -s assets/benchmarks.svg
head -c 5 assets/benchmarks.svg          # must print <?xml
bench-svg render <same input> -o /tmp/again.svg
cmp assets/benchmarks.svg /tmp/again.svg # must be identical
```

## Pick the input format

| Producer | Use | Notes |
|---|---|---|
| `cargo bench` / criterion | `--format criterion`, input `target/criterion` | walks `<bench>/new/estimates.json`, pairs it with `benchmark.json` |
| `hyperfine --export-json bench.json` | `--format hyperfine` | v2 and v1 shapes |
| Dart `benchmark_test` | `--format benchmark_test` (or `auto`) | one JSON object per line; non-JSON progress lines are ignored |
| Any tool | canonical JSON (`bench-svg/report@1`) | preferred for new integrations |
| Any script | CSV/TSV | header needs at least a `value` column; `id`, `name`, `label`, `note`, `group`, `unit`, `rme_pct` optional |

`auto` is the default and recognises all of the above. `bench-svg formats` prints the names.

Do not invent a format: emit canonical JSON (below) or CSV. Do not scrape benchmark stdout unless the
tool has no JSON at all (then emit CSV).

## Canonical input

```jsonc
{
  "schema": "bench-svg/report@1",
  "meta": {
    "tool": "my-bench",           // shown in the default subtitle
    "tool_version": "1.2.0",
    "commit": "cd5cd87",
    "branch": "master",
    "platform": {
      "os": "Linux", "os_version": "6.6", "arch": "x86_64",
      "cpu": "Ryzen 9 7950X", "cores": 32, "ram_gib": 64,
      "runner": "github-actions"
    }
  },
  "benchmarks": [{
    "id": "io.read.8MiB",         // stable key; required
    "group": "File I/O",          // optional, recorded only
    "label": "Sequential read",   // display; defaults to id
    "note": "8 MiB blocks",       // second line
    "unit": "B/s",                // required, drives formatting
    "value": 2254857830.0,        // required, in `unit`
    "direction": "higher",        // "higher" (default) or "lower"
    "params": { "block": "8MiB" },// structured, usable as {block} in row templates
    "metrics": { "peak_bytes": 104857600 },  // recorded, not rendered yet
    "stats": { "rme_pct": 1.21, "samples": 5, "median": 0.0, "stddev": 0.0 },
    "deterministic": false        // instruction counts etc.; recorded only
  }]
}
```

Units are dimensional, not decorative. Recognised families and examples:

| Family | Accepted spellings | Rendered as |
|---|---|---|
| bytes/time | `B/s`, `KiB/s`, `MiB/s`, `GiB/s`, `TiB/s`, `KB/s`, `MB/s`, `GB/s` | `2.10 GiB/s` |
| ops/time | `ops/s`, `op/s`, `IOPS`, `req/s`, `kops/s`, `mops/s` | `82.4k IOPS` |
| time/op | `ns/op`, `us/op`, `ms/op`, `s/op`, `ns`, `us`, `ms`, `s` | `1.24 ms` |
| count | `instr`, `instructions`, `count`, `ops` | `812.3k instr` |
| ratio | `x`, `ratio` | `1.00×` |
| anything else | raw symbol | `123.00 custom` |

Internally values are normalised to the SI base of their family. Bars are scaled **within one family**
against the best value, and `direction: "lower"` inverts the bar so longer always means better. Two
series of the same `id` are scaled against each other.

## Config (`bench-svg.toml`)

Discovered in the working directory; `--config FILE` overrides. All keys optional.

```toml
[font]
family = "mplus-rounded"        # file stem in ./fonts (or set --font / --font-file)

[render]
title = "my benchmarks"         # default: "benchmarks"
subtitle = "{platform|local} · {tool} {tool_version} · commit {commit}"
footer = "Generated from {rows} rows"
width = 920.0
min_height = 0.0                # footer stays pinned to the bottom
card_height = 38.0
row_gap = 10.0
padding = 22.0
theme = "pastel"                # "pastel" | "dark"
kaomoji = "(◕‿◕)♡"             # empty string removes it
max_rows = 12                   # optional; drops rows beyond N
only_matched = false            # true drops rows no [[row]] matched

# Two or more sources switch on the comparison layout.
[[series]]
label = "Linux x64"
input = "bench-linux.jsonl"
format = "benchmark_test"       # optional per-source override
color = "#7FD1B9"               # optional

[[series]]
label = "Windows x64"
input = "bench-windows.jsonl"

# Declarative row mapping; rules apply in order and define row order.
[[row]]
id = "io.read.8MiB"             # exact match
label = "Sequential read"
note = "8 MiB blocks · 1 GiB file"

[[row]]
match = '^pool\.read\.random4K\.w(\d+)$'   # regex on id
label = "Pool read · $1 workers"
note = "4 KiB · batch 32"
value = "value * $1 * 32"       # arithmetic over value and $1...
unit = "IOPS"
```

Row semantics:

- `id` exact or `match` regex; a rule needs one of them. Capture groups become `$1`… in `label`,
  `note` and `value`.
- `note = ""` clears the note; omitted keeps the input's note or the `params` summary.
- `value` grammar: numbers, `value`, `$1`…, `+ - * /`, parentheses, `min`, `max`, `abs`, `log10`,
  `sqrt`, `round`. No I/O, no property access, no eval. Results are formatted with `unit`.
- Unmatched rows are appended after matched ones unless `only_matched = true`.

Placeholders available in `title`, `subtitle`, `footer`:

`{commit}` (7 chars), `{commit:full}`, `{commit:short}`, `{branch}`, `{tool}`, `{tool_version}`,
`{tool:full}`, `{platform}`, `{os}`, `{os_version}`, `{arch}`, `{cpu}`, `{cores}`, `{ram_gib}`,
`{runner}`, `{rows}`, `{font}`.

`{key|fallback}` uses the fallback when the value is empty; `{{` is a literal `{`. Values come from the
input plus `GITHUB_SHA`, `RUNNER_OS`, `RUNNER_ARCH`. The clock is never read.

## CLI reference

```
bench-svg render <INPUT...> [--out FILE] [--format FORMAT] [--config FILE]
                 [--font NAME] [--font-file FILE]
                 [--title T] [--subtitle T] [--footer T]
                 [--width PX] [--min-height PX] [--max-rows N] [--theme NAME]
                 [--stdout]
bench-svg formats
```

- `--out -` writes the SVG to stdout.
- Font resolution: `--font-file` → `$BENCH_SVG_FONT_DIR` → `./fonts`. Inside a directory it prefers
  `<family>.ttf`, then falls back to any `*.ttf`. Without any font the command fails with exit code 2.
- Exit codes: `0` success, `2` any error (bad flags, unreadable input, no records, font missing, layout
  failure). Treat non-zero as failure and surface stderr.

## GitHub Action

```yaml
- uses: unk-404/benchmark_svg@v1
  id: bench
  with:
    input: target/criterion
    format: criterion
    out: assets/benchmarks.svg
    config: bench-svg.toml
```

Inputs: `input` (required), `format` (default `auto`), `config`, `out` (default
`assets/benchmarks.svg`), `font`, `font_file`, `version` (default `auto`), `tool_path`.
Outputs: `svg`, `bytes`. The step summary contains both.

Install behaviour:

1. `tool_path` set → use that binary, no network.
2. `version` not `auto` → download `releases/download/<version>/bench-svg-<target>.tar.gz`.
3. `version: auto` and `GITHUB_ACTION_REF` matches `vYYYY.MM.DD-<7 hex>` → use that tag.
4. Otherwise → `releases/latest/download/...` (no API call, no rate limit).

Downloads are cached under `RUNNER_TOOL_CACHE/bench-svg/<version>-<target>` and verified against the
published `.sha256`. `BENCH_SVG_RELEASE_BASE` overrides the release host (mirror, air-gapped proxy).

The action needs no token and no write permission. Fork pull requests work for rendering; committing
or commenting must be done by your own steps and is subject to the usual fork-token limits.

### Workflow shapes

Commit-back on the default branch:

```yaml
- uses: unk-404/benchmark_svg@v1
  with: { input: target/criterion, format: criterion, out: assets/benchmarks.svg }
- run: |
    git diff --quiet -- assets/benchmarks.svg && exit 0
    git config user.name  "github-actions[bot]"
    git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
    git add assets/benchmarks.svg
    git commit -m "chore(bench): refresh chart [skip ci]"
    git push
```

Pull-request preview without write access:

```yaml
- uses: unk-404/benchmark_svg@v1
  with: { input: target/criterion, format: criterion, out: target/preview.svg }
- uses: actions/upload-artifact@v4
  with: { name: benchmark-svg, path: target/preview.svg }
```

Multi-platform comparison: run the benchmark in a matrix, upload one JSON per platform, then render once
with `[[series]]` entries pointing at the downloaded files (see `templates/hyperfine-matrix.yml`).

Ready-made workflows: `templates/rust-criterion.yml`, `templates/dart-benchmark-test.yml`,
`templates/hyperfine-matrix.yml`.

## Verification checklist

Before claiming success:

1. `test -s <out>.svg` and the file starts with `<?xml`.
2. Every expected row appears: grep the label text or count `<g><title>` occurrences.
3. Rendering twice from the same inputs yields identical bytes (`cmp`).
4. `git status --short` shows only the intended SVG (generated assets are deterministic).
5. For a new integration, also run `cargo run --example gallery` and `cargo run --example specimen`
   and confirm `git diff --exit-code -- assets/` is clean (the repository commits its generated
   assets).

## Failure modes

| Symptom | Cause | Fix |
|---|---|---|
| `no font configured` | no TTF found | `--font-file`, `--font <name>`, or place a TTF in `./fonts` |
| `no benchmark records found` | wrong format or empty input | run `bench-svg formats`; pass `--format` explicitly; check the criterion path points at the tree containing `new/estimates.json` |
| `no benchmark records found` for criterion | input is a single `estimates.json` | pass the directory (`target/criterion`), not the file |
| rows missing | `max_rows`, `only_matched`, or ids deduped | raise `max_rows`, drop `only_matched` |
| wrong units | `unit` not recognised | use a known spelling or fix the emitter; unknown units render verbatim |
| bytes differ across machines | CRLF checkout or a local font edit | keep `.gitattributes` `* text=auto eol=lf`; do not edit `fonts/*.ttf` |
| criterion fields shift | its JSON is a private detail | pin the criterion version, or emit canonical JSON from `cargo-criterion` |
| action cannot download | no release yet / blocked network | pass `tool_path`, or set `BENCH_SVG_RELEASE_BASE` |

## What the tool does not do

No benchmark execution, no git, no tokens, no PR comments, no artifact upload, no history, no
regression gate, no hosted dashboard. It renders one SVG from data you already have. Keep those
concerns in the workflow.

Importers that do not exist yet: pytest-benchmark, JMH, gungraun, divan. Emit canonical JSON or CSV for
those.

## Repository map

| Path | Content |
|---|---|
| `src/model.rs` | canonical model, units and formatting |
| `src/input.rs` | format detection, JSONL/CSV/benchmark_test loading |
| `src/adapters.rs` | criterion and hyperfine importers |
| `src/config.rs` | TOML schema (`render`, `font`, `series`, `row`) |
| `src/expr.rs` | safe expression evaluator for `value` |
| `src/rows.rs` | `[[row]]` matching and templating |
| `src/font.rs` | TrueType metrics, outlines, ink boxes, rayon extraction |
| `src/layout.rs` | taffy layout, card geometry, legend |
| `src/svg.rs` | SVG emission, glyph atlas, themes |
| `src/render.rs` | orchestration and multi-source merge |
| `src/template.rs` | `{placeholder}` expansion |
| `action.yml`, `scripts/` | composite action: install + render |
| `.github/workflows/bundle.yml` | verify → identical-bytes → assets → action → build → publish |
| `examples/` | fixtures, gallery generator, specimens, rasteriser |
| `templates/` | copy-paste workflows |
