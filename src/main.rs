use std::path::PathBuf;

#[cfg(not(target_env = "musl"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use bench_svg::config::Config;
use bench_svg::input::{self, Format};
use bench_svg::render::{RenderRequest, SeriesInput, load_font};

#[derive(Debug, Parser)]
#[command(
    name = "bench-svg",
    version,
    about = "Render benchmark results as a self-contained SVG card for READMEs"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Command {
    /// Render benchmark results to SVG.
    Render(RenderArgs),
    /// List the input formats this build understands.
    Formats,
}

#[derive(Debug, clap::Args)]
struct RenderArgs {
    /// Input files or directories (JSON, JSONL, CSV).
    #[arg(value_name = "INPUT")]
    inputs: Vec<PathBuf>,

    /// Output SVG path; `-` writes to stdout.
    #[arg(short, long, value_name = "FILE", default_value = "benchmarks.svg")]
    out: String,

    /// Input format: auto, json, jsonl, csv, benchmark_test.
    #[arg(long, default_value = "auto", value_name = "FORMAT")]
    format: String,

    /// Compare an extra source: `label=path`. Repeat for more sources.
    #[arg(long = "series", value_name = "LABEL=PATH")]
    series: Vec<String>,

    /// Config file; defaults to ./bench-svg.toml when present.
    #[arg(long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// TrueType/OpenType file used to outline every label.
    #[arg(long, value_name = "FILE")]
    font_file: Option<PathBuf>,

    /// Font family name resolved inside ./fonts.
    #[arg(long, value_name = "NAME")]
    font: Option<String>,

    /// Override the heading.
    #[arg(long, value_name = "TEXT")]
    title: Option<String>,

    /// Override the provenance line (supports {placeholders}).
    #[arg(long, value_name = "TEXT")]
    subtitle: Option<String>,

    /// Override the footer line (supports {placeholders}).
    #[arg(long, value_name = "TEXT")]
    footer: Option<String>,

    /// Canvas width in pixels.
    #[arg(long, value_name = "PX")]
    width: Option<f32>,

    /// Minimum card height in pixels.
    #[arg(long = "min-height", value_name = "PX")]
    min_height: Option<f32>,

    /// Only render the first N rows.
    #[arg(long = "max-rows", value_name = "N")]
    max_rows: Option<usize>,

    /// Visual theme: pastel, dark.
    #[arg(long, value_name = "NAME")]
    theme: Option<String>,

    /// Print the SVG to stdout as well.
    #[arg(long)]
    stdout: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("bench-svg: {message}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Formats => {
            for format in Format::all() {
                println!("{format}");
            }
            Ok(())
        }
        Command::Render(args) => render(args),
    }
}

fn render(args: RenderArgs) -> Result<(), String> {
    let format = Format::parse(&args.format).ok_or_else(|| {
        format!(
            "unknown format {:?}; expected one of {:?}",
            args.format,
            Format::all()
        )
    })?;

    let mut config = Config::discover(args.config.as_deref())
        .map_err(|err| err.to_string())?
        .unwrap_or_default();

    if let Some(title) = args.title {
        config.render.title = Some(title);
    }
    if let Some(subtitle) = args.subtitle {
        config.render.subtitle = Some(subtitle);
    }
    if let Some(footer) = args.footer {
        config.render.footer = Some(footer);
    }
    if let Some(width) = args.width {
        config.render.width = width;
    }
    if let Some(min_height) = args.min_height {
        config.render.min_height = min_height;
    }
    if let Some(max_rows) = args.max_rows {
        config.render.max_rows = Some(max_rows);
    }
    if let Some(theme) = args.theme {
        config.render.theme = theme;
    }

    // CLI `--series label=path` takes precedence over the config file.
    for spec in &args.series {
        let (label, path) = spec
            .split_once('=')
            .ok_or_else(|| format!("--series expects `label=path`, got {spec:?}"))?;
        config.series.push(bench_svg::config::Series {
            label: label.to_owned(),
            input: path.to_owned(),
            format: None,
            color: None,
        });
    }

    // Each source is loaded independently so they can use different formats.
    let mut loaded = Vec::new();
    if config.series.is_empty() {
        if args.inputs.is_empty() {
            return Err("no inputs: pass files, or configure [[series]]".to_owned());
        }
        loaded.push(input::load(&args.inputs, format).map_err(|err| err.to_string())?);
    } else {
        for source in &config.series {
            let source_format = match &source.format {
                Some(name) => Format::parse(name).ok_or_else(|| {
                    format!(
                        "unknown format {name:?}; expected one of {:?}",
                        Format::all()
                    )
                })?,
                None => format,
            };
            let path = PathBuf::from(&source.input);
            loaded.push(
                input::load(std::slice::from_ref(&path), source_format)
                    .map_err(|err| format!("series {:?}: {err}", source.label))?,
            );
        }
    }

    // The first source carries the provenance for the whole card.
    let mut meta = loaded
        .first()
        .map(|source| source.meta.clone())
        .unwrap_or_default();
    if meta.commit.is_none() {
        meta.commit = std::env::var("GITHUB_SHA")
            .ok()
            .filter(|sha| !sha.is_empty());
    }
    if meta.platform.os.is_none() {
        meta.platform.os = std::env::var("RUNNER_OS")
            .ok()
            .filter(|value| !value.is_empty());
    }
    if meta.platform.arch.is_none() {
        meta.platform.arch = std::env::var("RUNNER_ARCH")
            .ok()
            .filter(|value| !value.is_empty());
    }

    let family = args.font.clone().or_else(|| config.font.family.clone());
    let font = load_font(args.font_file.as_deref(), family.as_deref()).map_err(|err| {
        format!("{err}\nhint: pass --font-file, set BENCH_SVG_FONT_DIR, or place a .ttf in ./fonts")
    })?;

    let series: Vec<SeriesInput<'_>> = loaded
        .iter()
        .enumerate()
        .map(|(index, source)| SeriesInput {
            label: config.series.get(index).map(|series| series.label.clone()),
            color: config
                .series
                .get(index)
                .and_then(|series| series.color.clone()),
            benchmarks: &source.benchmarks,
        })
        .collect();

    let rendered = bench_svg::render::render(RenderRequest {
        config: &config,
        meta: &meta,
        series: &series,
        font: &font,
    })
    .map_err(|err| err.to_string())?;

    for warning in &rendered.warnings {
        eprintln!("warning: {warning}");
    }

    if args.out == "-" {
        print!("{}", rendered.svg);
    } else {
        if args.stdout {
            print!("{}", rendered.svg);
        }
        let path = PathBuf::from(&args.out);
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("cannot create {}: {err}", parent.display()))?;
        }
        std::fs::write(&path, &rendered.svg)
            .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
    }

    eprintln!(
        "{} rows · {} series · {} bytes · font {}",
        rendered.rows,
        series.len().max(1),
        rendered.svg.len(),
        font.family
    );
    Ok(())
}
