use std::path::PathBuf;

#[cfg(not(target_env = "musl"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
use std::io::Write;
use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, Subcommand};
use tracing::{debug, info};

use benchcat::config::Config;
use benchcat::input::{self, Format};
use benchcat::render::{RenderRequest, RenderedParts, SeriesInput, load_font, render_parts};

#[derive(Debug, Parser)]
#[command(
    name = "benchcat",
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

    /// Config file; defaults to ./benchcat.toml when present.
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

fn init_logging() {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

fn main() -> ExitCode {
    init_logging();
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("benchcat: {message}");
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
        config.series.push(benchcat::config::Series {
            label: label.to_owned(),
            input: path.to_owned(),
            format: None,
            color: None,
        });
    }

    // Each source is loaded independently so they can use different formats.
    let load_start = Instant::now();
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

    info!(
        sources = loaded.len(),
        rows = loaded
            .iter()
            .map(|source| source.benchmarks.len())
            .sum::<usize>(),
        elapsed_ms = load_start.elapsed().as_secs_f64() * 1_000.0,
        "loaded benchmark data"
    );

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
        format!("{err}\nhint: pass --font-file, set BENCHCAT_FONT_DIR, or place a .ttf in ./fonts")
    })?;
    debug!(font = %font.family, "font loaded");

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

    let render_start = Instant::now();
    let parts = render_parts(RenderRequest {
        config: &config,
        meta: &meta,
        series: &series,
        font: &font,
    })
    .map_err(|err| err.to_string())?;
    let bytes = parts.len();

    info!(
        rows = parts.row_count,
        bytes,
        elapsed_ms = render_start.elapsed().as_secs_f64() * 1_000.0,
        "rendered card"
    );

    for warning in &parts.warnings {
        eprintln!("warning: {warning}");
    }

    if args.out == "-" {
        write_parts(&mut std::io::stdout().lock(), &parts)
            .map_err(|err| format!("cannot write stdout: {err}"))?;
    } else {
        if args.stdout {
            write_parts(&mut std::io::stdout().lock(), &parts)
                .map_err(|err| format!("cannot write stdout: {err}"))?;
        }
        let path = PathBuf::from(&args.out);
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("cannot create {}: {err}", parent.display()))?;
        }
        let write_start = Instant::now();
        let mut file = std::fs::File::create(&path)
            .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
        write_parts(&mut file, &parts)
            .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
        info!(
            path = %path.display(),
            bytes,
            elapsed_ms = write_start.elapsed().as_secs_f64() * 1_000.0,
            "wrote svg"
        );
    }

    eprintln!(
        "{} rows · {} series · {} bytes · font {}",
        parts.row_count,
        series.len().max(1),
        bytes,
        font.family
    );
    Ok(())
}

fn write_parts<W: Write>(out: &mut W, parts: &RenderedParts) -> std::io::Result<()> {
    out.write_all(parts.prefix.as_bytes())?;
    for chunk in &parts.rows {
        out.write_all(chunk.as_bytes())?;
    }
    out.write_all(parts.suffix.as_bytes())
}
