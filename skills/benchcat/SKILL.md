---
name: benchcat
description: Add the benchcat GitHub Action step to a workflow to render benchmark results (Criterion, hyperfine, Go, JMH/kotlinx, Google Benchmark, swift-benchmark, PHPBench, TechEmpower, AndroidX, Dart benchmark_test/benchmark_runner, canonical JSON/JSONL/CSV) as a committed SVG card. Use when wiring a benchmark chart into CI or comparing several sources/platforms. The action installs its own prebuilt binary — never download releases, vendor a binary, or call the CLI in CI.
---

# benchcat

Renders benchmark output as one deterministic SVG card, inside your CI.

- **Reference**: `unk-404/benchcat@master`. There is no `@v1` and no semantic version tag; releases
  are tagged `vYYYY.MM.DD-<sha7>`. Pin one of those only when you need an immutable ref.
- **The action installs its own binary** (cached, checksum-verified). You never download or manage it.

## Do this

```yaml
- uses: unk-404/benchcat@master
  with:
    input: target/criterion
    format: criterion
    out: assets/benchmarks.svg
```

**Fonts are automatic.** The action carries the OFL catalogue and points the renderer at it at
runtime. Do not add a `fonts/` directory, download TTFs, or commit font files. Leave `font` unset for the
default (`mplus-rounded`) or name a catalogue family; use `font_file` only for a custom TTF you already
have.

That is the whole integration. The action writes the SVG where `out` points (parent directories are
created) and exposes `svg` and `bytes` outputs. It reads the checked-out workspace, so run
`actions/checkout` first; `input` and `out` are relative to the repository root.

## Do not

- ❌ `gh release download`, `curl`, `wget` the archives
- ❌ `cargo install`, `git clone`, or vendor the binary
- ❌ run `benchcat render` as a `run:` step in CI
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
        uses: unk-404/benchcat@master
        id: bench
        with:
          input: target/criterion
          format: criterion
          out: assets/benchmarks.svg
          config: benchcat.toml        # optional

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
| `input` | — | files or directories, space/newline separated; optional when the config defines `[[series]]` |
| `format` | `auto` | `auto`, `json`, `jsonl`, `csv`, `benchmark_test`, `criterion`, `hyperfine`, `go`, `phpbench`, `jmh`, `google`, `google-console`, `swift`, `techempower`, `androidx`, `benchmark-test-human`, `benchmark-test-benchmarkjs`, `benchmark-runner` |
| `config` | — | `benchcat.toml`; auto-discovered when omitted |
| `out` | `assets/benchmarks.svg` | output path |
| `font` | — | one or more bundled catalogue families, comma separated, e.g. `quicksand,zcool-kuaile` |
| `font_file` | — | custom TTF/OTF (not needed for the catalogue) |
| `version` | `auto` | binary release tag |
| `tool_path` | — | existing binary, no download (air-gapped runners) |

Outputs: `svg`, `bytes` — both appear in the step summary.

Binary resolution: `tool_path` → explicit `version` → the action tag when it is a release tag → the
latest release. Cached under `RUNNER_TOOL_CACHE/benchcat/` and verified against the release's
`SHA256SUMS` (or a per-archive `.sha256` when present). The action derives its own repository from
`GITHUB_ACTION_PATH` (GitHub leaves `GITHUB_ACTION_REPOSITORY` empty inside composite steps), never your
repository; `BENCHCAT_RELEASE_BASE` overrides the release base for mirrors.

The action needs no token and no write permission.

## Data formats

| Producer | `input` | `format` |
|---|---|---|
| criterion (`cargo bench`) | `target/criterion` (directory) | `criterion` |
| hyperfine `--export-json` | the JSON file | `hyperfine` |
| Go testing | `go test -bench . -benchmem` console output | `go` |
| JMH / kotlinx-benchmark | `-rf json` (kotlinx default JSON) | `jmh` / `kotlinx` |
| Google Benchmark | `--benchmark_format=json` or console table | `google` / `google-console` |
| swift-benchmark | `--format json` | `swift` |
| PHPBench | `phpbench run --output=json` | `phpbench` |
| TechEmpower | `results.json` | `techempower` |
| AndroidX Benchmark | `benchmarkData.json` | `androidx` |
| `benchmark_test` human | default CLI output | `benchmark-test-human` |
| `benchmark_test` BenchmarkJS | `--output=benchmarkjs` | `benchmark-test-benchmarkjs` |
| `benchmark_test` JSONL | `--output=jsonl` | `benchmark_test` |
| `benchmark_runner` | exported stdout / report | `benchmark-runner` |
| anything else | canonical JSON / CSV you emit | `json` / `jsonl` / `csv` |
| unknown | — | `auto` |

Directory inputs are read recursively in sorted order. `format` also accepts the aliases `ndjson`,
`dart`, `canonical`, `cargo-criterion`, `java`, `kotlinx`, `gbench`, `c++`, `tfb` and
`android-benchmark`. Source presets live in `sources/*.oops`; see `sources/README.md`. The
[Sources wiki page](https://github.com/unk-404/benchcat/wiki/Sources) shows one mock card per format.

Canonical shape (`benchcat/report@1`):

```jsonc
{
  "schema": "benchcat/report@1",
  "meta": {
    "tool": "my-bench", "tool_version": "1.2.0", "commit": "cd5cd87",
    "platform": { "os": "Linux", "arch": "x86_64", "cpu": "Ryzen 9 7950X", "cores": 32 }
  },
  "benchmarks": [{
    "id": "io.read.8MiB",        // stable key, required
    "group": "File I/O",         // section heading
    "label": "Sequential read",  // display; defaults to id
    "note": "8 MiB blocks",      // second line
    "unit": "B/s",               // optional, default "ops/s"
    "value": 2254857830.0,       // required, in `unit`
    "direction": "higher",       // "higher" (default) or "lower"
    "params": { "block": "8MiB" },   // becomes the note when `note` is absent
    "stats": { "rme_pct": 1.21 }     // fills the right-hand ± column
  }]
}
```

`params` renders as the second line when `note` is absent (`key value · key value`, sorted by key); an
explicit `note` wins. `stats.rme_pct` also accepts `relativeMarginOfError`, `relative_margin_of_error`
and `rme`. `direction` accepts `higher-is-better`/`max` and `lower-is-better`/`min` as aliases.

`format: json` accepts the report object above, a bare array of records, or one record; JSONL is one
record per line, and lines that do not start with `{` are skipped (benchmark_test interleaves progress
output). CSV is header-driven: `value` (`value`, `ops`, `throughput`, `mean`) is required, `id` (`id`,
`name`, `benchmark`) falls back to `label` and then `row-N`; optional columns are `label`/`display`,
`note`/`notes`/`detail`, `group`/`section`, `unit`/`units` (default `ops/s`) and
`rme_pct`/`rme`/`rme%`/`margin`. CSV rows are always higher-is-better — use canonical JSON for
lower-is-better metrics.

Units are dimensional: `B/s`…`TiB/s`, `ops/s`/`IOPS`, `ns/op`…`s/op`, `instr`, `×`; unknown spellings
render verbatim. `direction: "lower"` inverts the bar so longer always means better.

A single-source card scales each bar with the log curve against the best row of the same unit family;
`bar_scale` and `bar_curve` do not apply there.

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
format = "jsonl"       # optional per-source format override
```

- `bar_scale = "row"`: bars are relative to the row's own best value; `"source"`: each source against
  its own best across rows; `"global"`: one reference per unit family for the whole card.
- `bar_curve = "linear"`: proportional bars (default in a comparison card); `"log"` compresses wide
  ranges.
- `missing = "skip"`: a source absent from a row draws no bar, and the **row height follows the bars it
  actually draws**: one bar means the row is exactly `card_height` (the single-source height); more bars
  grow with the stack. `"placeholder"` keeps the slot with an empty track and `—`, at the full row height.
- Bar thickness is 6 px inside a comparison card (even for a lone bar) and 10 px in a single-source
  card.

Rows are the union of ids across sources. When the config defines `[[series]]`, positional `input` is
ignored, so the action can omit it.

## Config (`benchcat.toml`)

```toml
[font]
family = ["mplus-rounded", "zcool-kuaile"]   # "a", "a,b" or ["a","b"]
enable_guaranteed_font = false              # opt-in fixed fallback list
enable_system_font = false                  # opt-in system-family lookup

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

### `[render]` keys

| key | default | meaning |
|---|---|---|
| `title` / `subtitle` / `footer` | derived | heading, provenance line, footer |
| `width` | `920` | canvas width in px |
| `min_height` | `0` | minimum card height; the footer stays pinned to the bottom |
| `card_height` | `42` | single-source row height; comparison rows may grow |
| `row_gap` | `10` | gap between cards |
| `padding` | `22` | inner page padding |
| `theme` | `"pastel"` | `pastel` or `dark` |
| `kaomoji` | `"(◕‿◕)♡"` | footer decoration; empty string removes it |
| `max_rows` | none | render only the first N rows |
| `only_matched` | `false` | drop rows no `[[row]]` rule matched |
| `bar_scale` | `"row"` | comparison cards only |
| `bar_curve` | `"linear"` | comparison cards only |
| `missing` | `"skip"` | comparison cards only |

`family` accepts one stem, a comma-separated string, or a list; entries are tried in order (a single
string keeps the old behaviour exactly).

The action sets `BENCHCAT_FONT_DIR` to its own `fonts/` catalogue, so no repository font files are
needed. Order: `font_file`/`--font-file` wins, then `font`/`--font` (or the `mplus-rounded` default)
looked up in `BENCHCAT_FONT_DIR` and then `./fonts`, then any TTF/OTF in those directories. For the CLI
outside the action, use `--font-file`, or `--font` with `./fonts/<stem>.ttf`, or
`BENCHCAT_FONT_DIR=/path/to/fonts`.

### Guaranteed fallback

`[font] enable_guaranteed_font = true` (or `BENCHCAT_FONT__ENABLE_GUARANTEED_FONT=true`) turns on a fixed
per-character fallback: our catalogue (`mplus-rounded`, `zen-maru-gothic`, `zcool-kuaile`, `quicksand`)
and then a short system list, at most four faces. The list is not configurable, and the option is off by
default — with it off, behaviour and bytes are exactly as before. If the configured face is missing or
unreadable, the first guaranteed face becomes the primary. Enable it for mixed-script cards; runs fully
covered by the primary face stay byte-identical.

`enable_system_font = true` (or `BENCHCAT_FONT__ENABLE_SYSTEM_FONT=true`) lets `family` names resolve
against installed system fonts after the repository catalogue; name matching ignores case and
punctuation, and collections are supported. It is off by default and does not change the guaranteed
list. Repository fonts always win.

### Keep automated builds deterministic

**Leave `enable_guaranteed_font` and `enable_system_font` off in CI.** Both options can consult fonts
installed on the machine: the guaranteed list falls through to a short system list, and
`enable_system_font` resolves family names from the OS. A GitHub runner, another runner image and a
developer laptop can therefore produce different SVG bytes from the same input. The defaults are already
`false`; do not turn them on in a config consumed by the action or any other automated build. If CI needs
another script or face, add it to the repository catalogue or pass `font` / `font_file` — never rely on
runner fonts. Enable these options only for a deliberate, one-off local render.

**Set exactly one font source.** A family list counts as one source. `font_file` overrides it;
`--font` overrides `[font] family`.
`BENCHCAT_FONT__FAMILY` (double underscore) is the only font config override; `BENCHCAT_FONT` and
`BENCHCAT_FONT_FAMILY` are ignored. Leave `BENCHCAT_FONT_DIR` alone when the action already sets it: it is
a runtime path, not a config key.

Each `[render]` key can be overridden from the environment: `BENCHCAT_RENDER__TITLE`,
`BENCHCAT_RENDER__WIDTH`, `BENCHCAT_FONT__FAMILY` (double underscore nests).

Placeholders: `{commit}`, `{commit:short}`, `{commit:full}`, `{branch}`, `{tool}`, `{tool:full}`,
`{tool_version}` (alias `{version}`), `{platform}`, `{os}`, `{os_version}`, `{arch}`, `{cpu}`,
`{cores}`, `{ram_gib}`, `{runner}`, `{rows}`, `{font}`. `{key|fallback}` supplies a fallback; `{{` is a
literal `{`. Inside GitHub Actions, a missing commit, OS or arch falls back to `GITHUB_SHA`, `RUNNER_OS`
and `RUNNER_ARCH`.

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
| `input path does not exist` | `input` is workspace-relative; the action must run after `actions/checkout` |
| `unknown format` | use one of the names in the format table (aliases such as `ndjson`, `dart` work) |
| `no font configured` | the action should not hit this; for the CLI pass `font_file`, use `--font` with `./fonts/<stem>.ttf`, or set `BENCHCAT_FONT_DIR` — never commit fonts |
| rows missing | `max_rows`, `only_matched`, duplicate ids, or positional input ignored because the config has `[[series]]` |
| unknown theme | `theme` must be `pastel` or `dark` |
| bytes differ between machines | CRLF checkout or an edited font; keep LF |
| action cannot download | no release yet / blocked network → `tool_path` or `BENCHCAT_RELEASE_BASE` |

## Example gallery and fonts

Every feature has a committed example under `assets/examples/` (see the README table); fonts are the
OFL catalogue under `fonts/`, each with a type specimen in `assets/fonts/`. Source-format cards live in
`assets/sources/` and are rendered by `cargo run --example sources`; the matching wiki page is
[Sources](https://github.com/unk-404/benchcat/wiki/Sources).

## Local debugging only

```bash
git clone https://github.com/unk-404/benchcat
cargo install --path benchcat/crates/benchcat
benchcat formats
benchcat render target/criterion -o /tmp/bench.svg --format criterion
benchcat render --config benchcat.toml --stdout  # [[series]] come from the config
benchcat render -o - examples/bench.jsonl        # "-" prints only to stdout
```

`--series LABEL=PATH` adds a comparison source; when any series exists (config or flag), positional
paths are ignored, so use one form or the other. `--stdout` writes the SVG to stdout in addition to
`--out`, `-o -` prints only, and `--font` / `--font-file` override `[font]`. Reproduce a CI failure or
check a config before committing; CI itself uses the action step.

## What it does not do

No benchmark execution, no git, no tokens, no PR comments, no artifact upload, no history, no
regression gate. It renders one SVG from data you already have.
