---
name: benchmark-svg
description: Add the bench-svg GitHub Action step to a workflow to render benchmark results (criterion, hyperfine, Dart benchmark_test, canonical JSON/JSONL, CSV) as a committed SVG card. Use when wiring a benchmark chart into CI or comparing several sources/platforms. The action installs its own prebuilt binary — never download releases, vendor a binary, or call the CLI in CI.
---

# benchmark-svg

Renders benchmark output as one deterministic SVG card, inside your CI.

- **Reference**: `unk-404/benchmark_svg@master`. There is no `@v1` and no semantic version tag; releases
  are tagged `vYYYY.MM.DD-<sha7>`. Pin one of those only when you need an immutable ref.
- **The action installs its own binary** (cached, checksum-verified). You never download or manage it.

## Do this

```yaml
- uses: unk-404/benchmark_svg@master
  with:
    input: target/criterion
    format: criterion
    out: assets/benchmarks.svg
```

That is the whole integration. The action writes the SVG where `out` points and exposes `svg` and
`bytes` outputs.

## Do not

- ❌ `gh release download`, `curl`, `wget` the archives
- ❌ `cargo install`, `git clone`, or vendor the binary
- ❌ run `bench-svg render` as a `run:` step in CI
- ❌ fetch the SVG from a release or artifact — the action already wrote it locally

If a plan contains any of the above, replace that part with the single `uses:` step.

## Full example

```yaml
name: benchmarks
on:
  push:
    branches: [main]

permissions:
  contents: write

jobs:
  chart:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      - name: Run benchmarks
        run: cargo bench -- --save-baseline main

      - name: Render the chart
        uses: unk-404/benchmark_svg@master
        id: bench
        with:
          input: target/criterion
          format: criterion
          out: assets/benchmarks.svg
          config: bench-svg.toml        # optional

      - name: Commit when the chart changed
        run: |
          git diff --quiet -- assets/benchmarks.svg && exit 0
          git config pull.rebase true
          git config user.name  "github-actions[bot]"
          git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
          git add assets/benchmarks.svg
          git commit -m "chore(bench): refresh chart [skip ci]"
          git push
```

For pull requests, skip the commit and upload `out` as an artifact instead.

## Inputs

| input | default | meaning |
|---|---|---|
| `input` | — | files or directories, space/newline separated (required) |
| `format` | `auto` | `auto`, `json`, `jsonl`, `csv`, `benchmark_test`, `criterion`, `hyperfine` |
| `config` | — | `bench-svg.toml`; auto-discovered when omitted |
| `out` | `assets/benchmarks.svg` | output path |
| `font` | — | family inside `./fonts` |
| `font_file` | — | explicit TTF/OTF |
| `version` | `auto` | binary release tag |
| `tool_path` | — | existing binary, no download (air-gapped runners) |

Outputs: `svg`, `bytes` — both appear in the step summary.

Binary resolution: `tool_path` → explicit `version` → the action tag when it is a release tag → the
latest release. Cached under `RUNNER_TOOL_CACHE/bench-svg/` and verified against the published
`.sha256`. `BENCH_SVG_RELEASE_BASE` points the download at a mirror.

The action needs no token and no write permission.

## Data formats

| Producer | `input` | `format` |
|---|---|---|
| criterion (`cargo bench`) | `target/criterion` (directory) | `criterion` |
| hyperfine `--export-json` | the JSON file | `hyperfine` |
| Dart `benchmark_test` | the JSONL file | `benchmark_test` |
| anything else | canonical JSON / CSV you emit | `json` / `jsonl` / `csv` |
| unknown | — | `auto` |

Canonical shape (`bench-svg/report@1`):

```jsonc
{
  "schema": "bench-svg/report@1",
  "meta": {
    "tool": "my-bench", "tool_version": "1.2.0", "commit": "cd5cd87",
    "platform": { "os": "Linux", "arch": "x86_64", "cpu": "Ryzen 9 7950X", "cores": 32 }
  },
  "benchmarks": [{
    "id": "io.read.8MiB",        // stable key, required
    "group": "File I/O",         // section heading
    "label": "Sequential read",  // display; defaults to id
    "note": "8 MiB blocks",      // second line
    "unit": "B/s",               // required
    "value": 2254857830.0,       // required, in `unit`
    "direction": "higher",       // or "lower"
    "params": { "block": "8MiB" },
    "stats": { "rme_pct": 1.21, "samples": 5 }
  }]
}
```

Units are dimensional: `B/s`…`TiB/s`, `ops/s`/`IOPS`, `ns/op`…`s/op`, `instr`, `×`; unknown spellings
render verbatim. Bars are scaled inside one unit family and `direction: "lower"` inverts the bar so
longer always means better.

## Multi-source comparison

Two or more `[[series]]` switch to the comparison layout: one row per operation, one bar per source, a
legend, one value per bar.

```toml
[render]
bar_scale = "row"      # row (default) | source | global
bar_curve = "linear"   # linear (default) | log
missing = "skip"       # skip (default) | placeholder

[[series]]
label = "Linux x64"
input = "bench-linux.jsonl"

[[series]]
label = "Windows x64"
input = "bench-windows.jsonl"
color = "#8CB8F0"
```

- `bar_scale = "row"`: bars are relative to the row's own best value; `"source"`: each source against
  its own best across rows; `"global"`: one reference per unit family for the whole card.
- `bar_curve = "linear"` keeps ratios honest; `"log"` compresses wide ranges. Single-source cards keep
  the log scale against the best row of the unit family.
- `missing = "skip"` draws no bar for a source absent from a row, and the row shrinks; `"placeholder"`
  keeps the slot and prints `—`.

Rows are the union of ids across sources.

## Config (`bench-svg.toml`)

```toml
[font]
family = "mplus-rounded"        # file stem in ./fonts

[render]
title = "my benchmarks"
subtitle = "{platform|local} · {tool} {tool_version} · commit {commit}"
footer = "Generated from {rows} rows"
width = 920.0
min_height = 0.0                # footer stays pinned to the bottom
card_height = 42.0
theme = "pastel"                # or "dark"
kaomoji = "(◕‿◕)♡"
max_rows = 12
only_matched = false

[[row]]
id = "io.read.8MiB"
label = "Sequential read"
note = "8 MiB blocks"

[[row]]
match = '^pool\.read\.random4K\.w(\d+)$'   # regex on id
label = "Pool read · $1 workers"
value = "value * $1 * 32"       # safe arithmetic
unit = "IOPS"
group = "Thread pool"           # section heading
```

Row rules: `id` exact or `match` regex; captures become `$1`… in `label`, `note`, `value`; `value`
supports `+ - * /`, parentheses and `min/max/abs/log10/sqrt/round`; `group` draws a section heading;
unmatched rows append unless `only_matched = true`.

Each `[render]` key can be overridden from the environment: `BENCH_SVG_RENDER__TITLE`,
`BENCH_SVG_RENDER__WIDTH`, `BENCH_SVG_FONT__FAMILY` (double underscore nests).

Placeholders: `{commit}`, `{commit:full}`, `{branch}`, `{tool}`, `{tool_version}`, `{platform}`,
`{os}`, `{os_version}`, `{arch}`, `{cpu}`, `{cores}`, `{ram_gib}`, `{runner}`, `{rows}`, `{font}`.
`{key|fallback}` supplies a fallback; `{{` is a literal `{`.

## Verify

```bash
test -s assets/benchmarks.svg
head -c 5 assets/benchmarks.svg     # must print <?xml
git status --short                  # only the SVG you meant to produce
```

The output is deterministic: unchanged data produces byte-identical files, so the commit step is a
no-op unless something actually changed.

## Failure modes

| Symptom | Fix |
|---|---|
| `no benchmark records found` | wrong `format`; criterion needs the directory, not `estimates.json` |
| `no font configured` | pass `font_file`, or put a TTF in `./fonts` |
| rows missing | `max_rows`, `only_matched`, or duplicate ids |
| bytes differ between machines | CRLF checkout or an edited font; keep LF |
| action cannot download | no release yet / blocked network → `tool_path` or `BENCH_SVG_RELEASE_BASE` |

## Example gallery and fonts

Every feature has a committed example under `assets/examples/` (see the README table); fonts are the
OFL catalogue under `fonts/`, each with a type specimen in `assets/fonts/`.

## Local debugging only

```bash
cargo install --git https://github.com/unk-404/benchmark_svg --branch master
bench-svg render target/criterion -o /tmp/bench.svg --format criterion
```

Reproduce a CI failure or check a config before committing. CI itself uses the action step.

## What it does not do

No benchmark execution, no git, no tokens, no PR comments, no artifact upload, no history, no
regression gate. It renders one SVG from data you already have.
