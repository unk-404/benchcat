//! Generate the example gallery under `assets/examples/`.
//!
//! Usage:
//!   cargo run --example gallery                # write SVGs
//!   cargo run --example gallery -- --png       # also rasterise previews to target/previews

use std::error::Error;
use std::path::{Path, PathBuf};

use bench_svg::config::Config;
use bench_svg::font::Font;
use bench_svg::input::{self, Format, Loaded};
use bench_svg::model::{Benchmark, Direction, Meta, Platform, Stats};
use bench_svg::render::{RenderRequest, SeriesInput, load_font};

const OUT_DIR: &str = "assets/examples";
const PREVIEW_DIR: &str = "target/previews";

fn main() -> Result<(), Box<dyn Error>> {
    let with_png = std::env::args().any(|arg| arg == "--png");
    let font = load_font(None, Some("mplus-rounded"))?;
    std::fs::create_dir_all(OUT_DIR)?;
    if with_png {
        std::fs::create_dir_all(PREVIEW_DIR)?;
    }

    let basic = load("examples/bench.jsonl")?;
    let mixed = load("examples/mixed.json")?;
    let cjk = load("examples/cjk.json")?;
    let windows = load("examples/series/windows.jsonl")?;
    let linux = load("examples/series/linux.jsonl")?;
    let macos = load("examples/series/macos.jsonl")?;

    // 01 · the default card.
    let mut config = Config::default();
    config.render.title = Some("benchmark_test · single source".to_owned());
    config.render.subtitle = Some("{platform|local} · {tool} · commit {commit}".to_owned());
    emit(
        &font,
        "01-basic",
        &config,
        &basic.meta,
        &[("", None, &basic.benchmarks)],
        with_png,
    )?;

    // 02 · the same operations measured on three platforms.
    let mut config = Config::default();
    config.render.title = Some("same operations, three platforms".to_owned());
    config.render.subtitle = Some("{rows} rows · {tool} · commit {commit}".to_owned());
    emit(
        &font,
        "02-comparison",
        &config,
        &linux.meta,
        &[
            ("Linux x64", Some("#7FD1B9"), &linux.benchmarks),
            ("Windows x64", Some("#8CB8F0"), &windows.benchmarks),
            ("macOS arm64", Some("#F2B984"), &macos.benchmarks),
        ],
        with_png,
    )?;

    // 03 · units come from the data instead of being baked into the renderer.
    let mut config = Config::default();
    config.render.title = Some("units are data, not constants".to_owned());
    config.render.subtitle = Some("{platform} · {tool} {tool_version}".to_owned());
    config.render.footer =
        Some("GiB/s, IOPS, ns/op and instruction counts formatted from their own unit".to_owned());
    emit(
        &font,
        "03-units",
        &config,
        &mixed.meta,
        &[("", None, &mixed.benchmarks)],
        with_png,
    )?;

    // 04 · min_height keeps two cards aligned even when row counts differ.
    let mut config = Config::default();
    config.render.title = Some("min_height = 520".to_owned());
    config.render.subtitle = Some("a short list still fills the card".to_owned());
    config.render.min_height = 520.0;
    emit(
        &font,
        "04-min-height",
        &config,
        &basic.meta,
        &[("", None, &basic.benchmarks[..4])],
        with_png,
    )?;

    // 05 · dark palette.
    let mut config = Config::default();
    config.render.title = Some("dark theme".to_owned());
    config.render.subtitle = Some("{platform|local} · commit {commit}".to_owned());
    config.render.theme = "dark".to_owned();
    emit(
        &font,
        "05-dark",
        &config,
        &basic.meta,
        &[("", None, &basic.benchmarks)],
        with_png,
    )?;

    // 06 · top-N truncation for a stable README height.
    let mut config = Config::default();
    config.render.title = Some("max_rows = 4".to_owned());
    config.render.subtitle = Some("only the first four rows are rendered".to_owned());
    config.render.max_rows = Some(4);
    emit(
        &font,
        "06-max-rows",
        &config,
        &basic.meta,
        &[("", None, &basic.benchmarks)],
        with_png,
    )?;

    // 07 · height follows the content, however long the list is.
    let dense = synthetic(24);
    let mut config = Config::default();
    config.render.title = Some("height follows the rows".to_owned());
    config.render.subtitle = Some("{rows} synthetic rows · no manual height".to_owned());
    emit(
        &font,
        "07-dynamic-height",
        &config,
        &synthetic_meta(),
        &[("", None, &dense)],
        with_png,
    )?;

    // 08 · any language, as long as the supplied font has the glyphs.
    let mut config = Config::default();
    config.render.title = Some("any language the font covers".to_owned());
    config.render.subtitle = Some("{font} · labels are outlines, not <text>".to_owned());
    config.render.kaomoji = "ᕕ( ᐛ )ᕗ".to_owned();
    emit(
        &font,
        "08-cjk",
        &config,
        &cjk.meta,
        &[("", None, &cjk.benchmarks)],
        with_png,
    )?;

    // 09 · notes and structured params render under the label.
    let mut config = Config::default();
    config.render.title = Some("notes and parameters".to_owned());
    config.render.subtitle = Some("note + params become the second line".to_owned());
    emit(
        &font,
        "09-notes",
        &config,
        &mixed.meta,
        &[("", None, &mixed.benchmarks[..4])],
        with_png,
    )?;

    // 10 · criterion adapter: a `target/criterion` tree, no conversion step.
    let criterion = input::load(&[PathBuf::from("examples/criterion")], Format::Criterion)?;
    let mut config = Config::default();
    config.render.title = Some("criterion adapter".to_owned());
    config.render.subtitle =
        Some("{tool} · {rows} rows · straight from target/criterion".to_owned());
    emit(
        &font,
        "10-criterion",
        &config,
        &criterion.meta,
        &[("", None, &criterion.benchmarks)],
        with_png,
    )?;

    // 11 · hyperfine adapter: v2 JSON, seconds become ms/µs automatically.
    let hyperfine = load("examples/hyperfine.json")?;
    let mut config = Config::default();
    config.render.title = Some("hyperfine adapter".to_owned());
    config.render.subtitle = Some("{tool} · wall-clock means, lower is better".to_owned());
    emit(
        &font,
        "11-hyperfine",
        &config,
        &hyperfine.meta,
        &[("", None, &hyperfine.benchmarks)],
        with_png,
    )?;

    // 12 · declarative row mapping from examples/rows.toml.
    let rows_config = Config::load(Path::new("examples/rows.toml"))?;
    emit(
        &font,
        "12-rows",
        &rows_config,
        &basic.meta,
        &[("", None, &basic.benchmarks)],
        with_png,
    )?;

    Ok(())
}

fn load(path: &str) -> Result<Loaded, Box<dyn Error>> {
    Ok(input::load(&[PathBuf::from(path)], Format::Auto)?)
}

#[allow(clippy::too_many_arguments)]
fn emit(
    font: &Font,
    name: &str,
    config: &Config,
    meta: &Meta,
    sources: &[(&str, Option<&str>, &[Benchmark])],
    with_png: bool,
) -> Result<(), Box<dyn Error>> {
    let series: Vec<SeriesInput<'_>> = sources
        .iter()
        .map(|(label, color, benchmarks)| SeriesInput {
            label: if label.is_empty() {
                config.series.first().map(|series| series.label.clone())
            } else {
                Some((*label).to_owned())
            },
            color: color.map(|color| color.to_owned()),
            benchmarks,
        })
        .collect();
    let mut meta = meta.clone();
    if meta.commit.is_none() {
        meta.commit = Some("a1b2c3d4e5f60718".to_owned());
    }
    if meta.platform.os.is_none() {
        meta.platform = Platform {
            os: Some("Linux".to_owned()),
            arch: Some("x86_64".to_owned()),
            runner: Some("github-actions".to_owned()),
            ..Platform::default()
        };
    }
    let rendered = bench_svg::render::render(RenderRequest {
        config,
        meta: &meta,
        series: &series,
        font,
    })?;

    let svg_path = Path::new(OUT_DIR).join(format!("{name}.svg"));
    std::fs::write(&svg_path, &rendered.svg)?;
    println!(
        "{:<24} {:>3} rows  {:>7} bytes",
        svg_path.display(),
        rendered.rows,
        rendered.svg.len()
    );
    for warning in &rendered.warnings {
        eprintln!("  warning: {warning}");
    }

    if with_png {
        let png_path = Path::new(PREVIEW_DIR).join(format!("{name}.png"));
        rasterise(&rendered.svg, &png_path, 2.0)?;
    }
    Ok(())
}

fn rasterise(svg: &str, path: &Path, scale: f32) -> Result<(), Box<dyn Error>> {
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_str(svg, &options)?;
    let size = tree.size();
    let width = (size.width() * scale).ceil() as u32;
    let height = (size.height() * scale).ceil() as u32;
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(width, height).ok_or("cannot allocate pixmap")?;
    pixmap.fill(resvg::tiny_skia::Color::WHITE);
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.save_png(path)?;
    Ok(())
}

fn synthetic_meta() -> Meta {
    Meta {
        tool: Some("bench-svg".to_owned()),
        tool_version: Some("0.1.0".to_owned()),
        commit: Some("a1b2c3d4e5f60718".to_owned()),
        platform: Platform {
            os: Some("Linux".to_owned()),
            arch: Some("x86_64".to_owned()),
            ..Platform::default()
        },
        ..Meta::default()
    }
}

/// Deterministic synthetic rows so the gallery never depends on a live run.
fn synthetic(count: usize) -> Vec<Benchmark> {
    let units = ["IOPS", "ns/op", "B/s", "instr"];
    (0..count)
        .map(|index| {
            let unit = units[index % units.len()];
            let value = match unit {
                "IOPS" => 50_000.0 + (index as f64) * 7_919.0,
                "ns/op" => 120.0 + (index as f64) * 31.0,
                "B/s" => (400.0 + (index as f64) * 37.0) * 1024.0 * 1024.0,
                _ => 90_000.0 + (index as f64) * 4_321.0,
            };
            Benchmark {
                id: format!("module{}.operation{:02}", index / 4 + 1, index % 4 + 1),
                group: None,
                label: Some(format!("operation {:02}", index + 1)),
                note: Some(format!("synthetic · unit {unit}")),
                unit: unit.to_owned(),
                value,
                direction: if unit == "ns/op" {
                    Direction::Lower
                } else {
                    Direction::Higher
                },
                params: Default::default(),
                metrics: Default::default(),
                stats: Stats {
                    rme_pct: Some(1.0 + (index % 9) as f64 * 0.4),
                    ..Stats::default()
                },
                deterministic: false,
                extra: Default::default(),
            }
        })
        .collect()
}
