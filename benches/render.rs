//! Development-only benchmark for the render pipeline.
//!
//! It is deliberately **not** part of CI: `Cargo.toml` marks the target
//! `test = false` so `cargo test` never runs it, and no workflow calls
//! `cargo bench`. Run it locally when changing parsing, layout, fonts or the
//! SVG writer:
//!
//! ```text
//! cargo bench --bench render
//! cargo bench --bench render -- --rows 20000 --series 3 --iterations 20
//! cargo bench --bench render -- --rows 1000,5000 --phases parse,render --json
//! ```
//!
//! The corpus is generated in a temporary directory, so timings include real
//! file I/O and the same canonical JSONL parser the CLI uses. Phases:
//!
//! - `font`: cold `load_font` (file lookup + parse), once per iteration
//! - `parse`: `input::load` over every source file
//! - `render`: layout + SVG emission from already-parsed rows
//! - `full`: parse + render, i.e. what `benchcat render <files>` does
//!   (excluding process start-up and font loading)

use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Instant;

use benchcat::config::{BarCurve, BarScale, Config, Missing};
use benchcat::input::{self, Format};
use benchcat::render::{self, RenderRequest, SeriesInput};
use clap::Parser;
use serde_json::json;

#[derive(Parser)]
#[command(
    name = "benchcat-render-bench",
    about = "Development render benchmark (not run in CI)"
)]
struct Args {
    /// Row counts to benchmark (comma separated).
    #[arg(long, value_delimiter = ',', default_value = "1000,5000,20000")]
    rows: Vec<usize>,

    /// Source counts to benchmark; two or more use the comparison layout.
    #[arg(long, value_delimiter = ',', default_value = "1")]
    series: Vec<usize>,

    /// Timed iterations per case.
    #[arg(long, default_value_t = 10)]
    iterations: usize,

    /// Untimed warm-up iterations per case.
    #[arg(long, default_value_t = 2)]
    warmup: usize,

    /// Font family from ./fonts.
    #[arg(long, default_value = "mplus-rounded")]
    font: String,

    /// Explicit TTF/OTF file, bypassing the family lookup.
    #[arg(long)]
    font_file: Option<PathBuf>,

    /// Rows per section group in the generated corpus.
    #[arg(long, default_value_t = 250)]
    group_size: usize,

    /// Add a note to every Nth row (0 disables notes).
    #[arg(long, default_value_t = 25)]
    note_every: usize,

    /// Multi-source bar scale: row, source or global.
    #[arg(long, default_value = "row")]
    bar_scale: String,

    /// Multi-source bar curve: linear or log.
    #[arg(long, default_value = "linear")]
    bar_curve: String,

    /// Missing-source behaviour: skip or placeholder.
    #[arg(long, default_value = "skip")]
    missing: String,

    /// Only render the first N rows.
    #[arg(long)]
    max_rows: Option<usize>,

    /// Phases to run: font, parse, render, full.
    #[arg(long, value_delimiter = ',', default_value = "font,parse,render,full")]
    phases: Vec<String>,

    /// Keep the generated corpus directory and print its path.
    #[arg(long)]
    keep: bool,

    /// Emit one JSON object per line instead of tables.
    #[arg(long)]
    json: bool,

    /// Ignored: cargo passes `--bench` to harness-free bench targets.
    #[arg(long, hide = true)]
    bench: bool,
}

#[derive(Clone, Copy)]
struct Stats {
    min: f64,
    median: f64,
    p95: f64,
    mean: f64,
}

fn summarize(mut samples: Vec<f64>) -> Stats {
    samples.sort_by(f64::total_cmp);
    let n = samples.len();
    let median = if n % 2 == 1 {
        samples[n / 2]
    } else {
        (samples[n / 2 - 1] + samples[n / 2]) / 2.0
    };
    let p95_index = ((n as f64 - 1.0) * 0.95).round() as usize;
    Stats {
        min: samples[0],
        median,
        p95: samples[p95_index],
        mean: samples.iter().sum::<f64>() / n as f64,
    }
}

fn timed<F: FnMut() -> usize>(warmup: usize, iterations: usize, mut op: F) -> Stats {
    for _ in 0..warmup {
        black_box(op());
    }
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        let result = op();
        samples.push(start.elapsed().as_secs_f64() * 1_000.0);
        black_box(result);
    }
    summarize(samples)
}

struct Corpus {
    root: PathBuf,
    keep: bool,
}

impl Corpus {
    fn new(keep: bool) -> Result<Self, String> {
        let root = std::env::temp_dir().join(format!("benchcat-bench-{}", process::id()));
        if root.exists() {
            fs::remove_dir_all(&root)
                .map_err(|err| format!("cannot clear {}: {err}", root.display()))?;
        }
        fs::create_dir_all(&root)
            .map_err(|err| format!("cannot create {}: {err}", root.display()))?;
        Ok(Self { root, keep })
    }

    fn files(&self, rows: usize, series: usize, args: &Args) -> Result<Vec<PathBuf>, String> {
        let dir = self.root.join(format!("rows-{rows}-series-{series}"));
        fs::create_dir_all(&dir)
            .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
        let mut paths = Vec::with_capacity(series);
        for source in 0..series {
            let path = dir.join(format!("source-{source}.jsonl"));
            write_corpus(&path, rows, source, args.group_size, args.note_every)?;
            paths.push(path);
        }
        Ok(paths)
    }
}

impl Drop for Corpus {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn write_corpus(
    path: &Path,
    rows: usize,
    source: usize,
    group_size: usize,
    note_every: usize,
) -> Result<(), String> {
    let group_size = group_size.max(1);
    let mut text = String::with_capacity(rows * 160);
    for row in 0..rows {
        let group = row / group_size;
        let mut record = json!({
            "id": format!("op.{group:03}.{row:05}"),
            "group": format!("group {group:03}"),
            "label": format!("operation {row}"),
            "unit": "ops/s",
            "value": 1_000.0 + (row % 977) as f64 * 3.5 + source as f64 * 250.0,
            "direction": "higher",
            "params": { "workers": 1 + (row % 8) },
        });
        if note_every > 0 && row % note_every == 0 {
            record["note"] = json!(format!("batch {row}"));
        }
        text.push_str(&serde_json::to_string(&record).map_err(|err| err.to_string())?);
        text.push('\n');
    }
    fs::write(path, text).map_err(|err| format!("cannot write {}: {err}", path.display()))
}

fn load_one(path: &Path) -> Result<input::Loaded, String> {
    let owned = path.to_path_buf();
    input::load(std::slice::from_ref(&owned), Format::Jsonl)
        .map_err(|err| format!("{}: {err}", path.display()))
}

fn build_config(args: &Args) -> Result<Config, String> {
    let mut config = Config::default();
    config.render.bar_scale = match args.bar_scale.as_str() {
        "row" => BarScale::Row,
        "source" => BarScale::Source,
        "global" => BarScale::Global,
        other => {
            return Err(format!(
                "unknown --bar-scale {other:?}: row, source, global"
            ));
        }
    };
    config.render.bar_curve = match args.bar_curve.as_str() {
        "linear" => BarCurve::Linear,
        "log" => BarCurve::Log,
        other => return Err(format!("unknown --bar-curve {other:?}: linear, log")),
    };
    config.render.missing = match args.missing.as_str() {
        "skip" => Missing::Skip,
        "placeholder" => Missing::Placeholder,
        other => return Err(format!("unknown --missing {other:?}: skip, placeholder")),
    };
    config.render.max_rows = args.max_rows;
    Ok(config)
}

fn main() {
    let args = Args::parse();
    if let Err(message) = run(&args) {
        eprintln!("error: {message}");
        process::exit(2);
    }
}

fn run(args: &Args) -> Result<(), String> {
    if args.rows.is_empty() || args.series.is_empty() {
        return Err("--rows and --series must not be empty".to_owned());
    }
    if args.iterations == 0 {
        return Err("--iterations must be at least 1".to_owned());
    }
    if args.rows.contains(&0) || args.series.contains(&0) {
        return Err("--rows and --series values must be at least 1".to_owned());
    }
    let allowed = ["font", "parse", "render", "full"];
    for phase in &args.phases {
        if !allowed.contains(&phase.as_str()) {
            return Err(format!(
                "unknown --phases entry {phase:?}; expected one of {allowed:?}"
            ));
        }
    }
    let wants = |phase: &str| args.phases.iter().any(|value| value == phase);

    let config = build_config(args)?;
    let corpus = Corpus::new(args.keep)?;
    let family = Some(args.font.as_str());
    let font = render::load_font(args.font_file.as_deref(), family)
        .map_err(|err| format!("{err}\nhint: run from the repository root or pass --font-file"))?;

    if !args.json {
        println!("benchcat render benchmark (development only)");
        println!(
            "font={} iterations={} warmup={} rows={:?} series={:?}",
            args.font, args.iterations, args.warmup, args.rows, args.series
        );
        if args.series.iter().any(|&value| value > 1) {
            println!(
                "multi-source: bar_scale={} bar_curve={} missing={} max_rows={:?}",
                args.bar_scale, args.bar_curve, args.missing, args.max_rows
            );
        }
        println!();
    }

    if wants("font") {
        let stats = timed(args.warmup, args.iterations, || {
            let loaded = render::load_font(args.font_file.as_deref(), family)
                .expect("font should load during the benchmark");
            black_box(loaded);
            0
        });
        if args.json {
            println!(
                "{}",
                json!({
                    "kind": "font",
                    "family": args.font,
                    "median_ms": stats.median,
                    "min_ms": stats.min,
                    "p95_ms": stats.p95,
                    "mean_ms": stats.mean,
                })
            );
        } else {
            println!(
                "font load   median {:8.2} ms   min {:8.2}   p95 {:8.2}",
                stats.median, stats.min, stats.p95
            );
            println!();
        }
    }

    if !args.json {
        println!(
            "{:>7} {:>7} {:>12} {:>12} {:>12} {:>14} {:>10}",
            "rows", "series", "parse ms", "render ms", "full ms", "full rows/s", "svg bytes"
        );
    }

    for &rows in &args.rows {
        for &series in &args.series {
            let paths = corpus.files(rows, series, args)?;
            let total_rows = rows.saturating_mul(series);

            let parse_stats = wants("parse").then(|| {
                timed(args.warmup, args.iterations, || {
                    let loaded =
                        input::load(&paths, Format::Jsonl).expect("generated corpus should parse");
                    loaded.benchmarks.len()
                })
            });

            let loaded: Vec<input::Loaded> = paths
                .iter()
                .map(|path| load_one(path))
                .collect::<Result<_, _>>()?;

            if loaded.iter().any(|source| source.benchmarks.is_empty()) {
                return Err(format!(
                    "generated corpus for {rows} rows produced no records"
                ));
            }

            let mut meta = loaded[0].meta.clone();
            meta.tool = Some("benchcat".to_owned());
            meta.tool_version = Some(env!("CARGO_PKG_VERSION").to_owned());

            let series_inputs: Vec<SeriesInput> = loaded
                .iter()
                .enumerate()
                .map(|(index, source)| SeriesInput {
                    label: Some(format!("source {}", index + 1)),
                    color: None,
                    benchmarks: &source.benchmarks,
                })
                .collect();

            let render_stats = wants("render").then(|| {
                timed(args.warmup, args.iterations, || {
                    render::render(RenderRequest {
                        config: &config,
                        meta: &meta,
                        series: &series_inputs,
                        font: &font,
                    })
                    .expect("render should succeed")
                    .svg
                    .len()
                })
            });

            let full_stats = wants("full").then(|| {
                timed(args.warmup, args.iterations, || {
                    let sources: Vec<input::Loaded> = paths
                        .iter()
                        .map(|path| load_one(path).expect("generated corpus should parse"))
                        .collect();
                    let inputs: Vec<SeriesInput> = sources
                        .iter()
                        .enumerate()
                        .map(|(index, source)| SeriesInput {
                            label: Some(format!("source {}", index + 1)),
                            color: None,
                            benchmarks: &source.benchmarks,
                        })
                        .collect();
                    let mut meta = sources[0].meta.clone();
                    meta.tool = Some("benchcat".to_owned());
                    meta.tool_version = Some(env!("CARGO_PKG_VERSION").to_owned());
                    render::render(RenderRequest {
                        config: &config,
                        meta: &meta,
                        series: &inputs,
                        font: &font,
                    })
                    .expect("render should succeed")
                    .svg
                    .len()
                })
            });

            let svg_bytes = render::render(RenderRequest {
                config: &config,
                meta: &meta,
                series: &series_inputs,
                font: &font,
            })
            .expect("render should succeed")
            .svg
            .len();

            if args.json {
                let phase = |stats: Option<Stats>| match stats {
                    Some(stats) => json!({
                        "median_ms": stats.median,
                        "min_ms": stats.min,
                        "p95_ms": stats.p95,
                        "mean_ms": stats.mean,
                    }),
                    None => serde_json::Value::Null,
                };
                println!(
                    "{}",
                    json!({
                        "kind": "case",
                        "rows": rows,
                        "series": series,
                        "total_rows": total_rows,
                        "parse": phase(parse_stats),
                        "render": phase(render_stats),
                        "full": phase(full_stats),
                        "svg_bytes": svg_bytes,
                    })
                );
            } else {
                let cell = |stats: Option<Stats>| match stats {
                    Some(stats) => format!("{:12.2}", stats.median),
                    None => format!("{:>12}", "-"),
                };
                let rate = full_stats
                    .map(|stats| format!("{:14.0}", total_rows as f64 / (stats.median / 1_000.0)))
                    .unwrap_or_else(|| format!("{:>14}", "-"));
                println!(
                    "{:>7} {:>7} {} {} {} {} {:>10}",
                    rows,
                    series,
                    cell(parse_stats),
                    cell(render_stats),
                    cell(full_stats),
                    rate,
                    svg_bytes
                );
            }
        }
    }

    if args.keep && !args.json {
        println!("\ncorpus kept at {}", corpus.root.display());
    }
    Ok(())
}
