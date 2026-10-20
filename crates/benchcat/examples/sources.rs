//! Generate one mock card per supported source format under `assets/sources/`.
//!
//! Usage:
//!   cargo run --example sources
//!   cargo run --example sources -- --only go,jmh
//!   cargo run --example sources -- --list

use std::error::Error;
use std::path::{Path, PathBuf};

use benchcat::config::Config;
use benchcat::font::FontStack;
use benchcat::input::{self, Format};
use benchcat::render::{RenderRequest, SeriesInput, load_font};

const OUT_DIR: &str = "assets/sources";

struct Case {
    slug: &'static str,
    format: Format,
    title: &'static str,
    description: &'static str,
    input: &'static str,
}

#[derive(clap::Parser)]
#[command(about = "Generate one mock card per source format under assets/sources/")]
struct Args {
    /// Only these cases, comma separated
    #[arg(long, value_delimiter = ',')]
    only: Vec<String>,
    /// Print case names and exit
    #[arg(long)]
    list: bool,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            slug: "go",
            format: Format::Source("go"),
            title: "Go testing",
            description: "go test -bench columns; ns/op",
            input: "examples/sources/go.txt",
        },
        Case {
            slug: "jmh",
            format: Format::Source("jmh"),
            title: "JMH",
            description: "JMH JSON; ns/op lower is better",
            input: "examples/sources/jmh.json",
        },
        Case {
            slug: "google",
            format: Format::Source("google"),
            title: "Google Benchmark JSON",
            description: "cpu_time normalised to ns/op",
            input: "examples/sources/google.json",
        },
        Case {
            slug: "google-console",
            format: Format::Source("google-console"),
            title: "Google Benchmark console",
            description: "BM_ columns copied from the console table",
            input: "examples/sources/google-console.txt",
        },
        Case {
            slug: "swift",
            format: Format::Source("swift"),
            title: "swift-benchmark",
            description: "swift-benchmark --format json",
            input: "examples/sources/swift.json",
        },
        Case {
            slug: "androidx",
            format: Format::Source("androidx"),
            title: "AndroidX Benchmark",
            description: "benchmarkData.json; ns/op or ms/op",
            input: "examples/sources/androidx.json",
        },
        Case {
            slug: "phpbench",
            format: Format::Source("phpbench"),
            title: "PHPBench",
            description: "PHPBench JSONL report",
            input: "examples/sources/phpbench.jsonl",
        },
        Case {
            slug: "techempower",
            format: Format::Source("techempower"),
            title: "TechEmpower",
            description: "results.json rawData; req/s higher is better",
            input: "examples/sources/techempower.json",
        },
        Case {
            slug: "benchmark-test-human",
            format: Format::Source("benchmark-test-human"),
            title: "benchmark_test human",
            description: "Dart benchmark_test default output",
            input: "examples/sources/benchmark-test-human.txt",
        },
        Case {
            slug: "benchmark-test-benchmarkjs",
            format: Format::Source("benchmark-test-benchmarkjs"),
            title: "benchmark_test BenchmarkJS",
            description: "Dart benchmark_test --output=benchmarkjs",
            input: "examples/sources/benchmark-test-benchmarkjs.txt",
        },
        Case {
            slug: "benchmark-runner",
            format: Format::Source("benchmark-runner"),
            title: "benchmark_runner",
            description: "Dart benchmark_runner report blocks",
            input: "examples/sources/benchmark-runner.txt",
        },
        Case {
            slug: "criterion",
            format: Format::Criterion,
            title: "Criterion",
            description: "criterion target directory",
            input: "examples/sources/criterion",
        },
        Case {
            slug: "benchmark-test-jsonl",
            format: Format::BenchmarkTest,
            title: "benchmark_test JSONL",
            description: "Dart benchmark_test --output=jsonl",
            input: "examples/sources/benchmark-test-jsonl.jsonl",
        },
        Case {
            slug: "benchmarkdotnet",
            format: Format::Source("benchmarkdotnet"),
            title: "BenchmarkDotNet",
            description: "C#/.NET Markdown and CSV tables; ns/op or us/op",
            input: "examples/sources/benchmarkdotnet.md",
        },
        Case {
            slug: "hyperfine",
            format: Format::Hyperfine,
            title: "hyperfine",
            description: "hyperfine --export-json",
            input: "examples/sources/hyperfine.json",
        },
    ]
}

fn oops_version(source: &str) -> Option<u32> {
    let text = std::fs::read_to_string(source).ok()?;
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("//"))
        .and_then(|line| line.strip_prefix("@version "))
        .and_then(|value| value.trim().parse().ok())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = <Args as clap::Parser>::parse();
    let all = cases();
    if args.list {
        for case in &all {
            println!("{}", case.slug);
        }
        return Ok(());
    }
    let selected: Vec<&str> = args.only.iter().map(String::as_str).collect();
    let cases: Vec<&Case> = all
        .iter()
        .filter(|case| selected.is_empty() || selected.contains(&case.slug))
        .collect();
    if cases.is_empty() {
        return Err("no matching sources; try --list".into());
    }

    let font = FontStack::single(load_font(None, Some("mplus-rounded"))?);
    std::fs::create_dir_all(OUT_DIR)?;

    let mut catalogue = Vec::new();
    for case in &cases {
        let input_path = PathBuf::from(case.input);
        let loaded = input::load(std::slice::from_ref(&input_path), case.format)?;
        let mut meta = loaded.meta.clone();
        meta.tool = Some(case.slug.to_owned());
        let source = if case.slug == "criterion" {
            Some("sources/criterion.oops".to_owned())
        } else {
            matches!(&case.format, Format::Source(_))
                .then(|| format!("sources/{}.oops", case.slug.replace('-', "_")))
        };
        let version = source.as_deref().and_then(oops_version);
        let version_label = version
            .map(|version| format!("OOPS v{version}"))
            .unwrap_or_else(|| "built-in".to_owned());
        let mut config = Config::default();
        config.render.title = Some(case.title.to_owned());
        config.render.subtitle = Some(format!("{{rows}} rows · {version_label} · {{tool}}"));
        config.render.footer = Some(format!("{} · {version_label}", case.description));
        let series = [SeriesInput {
            label: None,
            color: None,
            benchmarks: &loaded.benchmarks,
        }];
        let rendered = benchcat::render::render(RenderRequest {
            config: &config,
            meta: &meta,
            series: &series,
            font: &font,
        })?;
        let svg_path = Path::new(OUT_DIR).join(format!("{}.svg", case.slug));
        std::fs::write(&svg_path, &rendered.svg)?;
        println!(
            "{:<32} {:>3} rows  {:>7} bytes",
            svg_path.display(),
            rendered.rows,
            rendered.svg.len()
        );
        let input = input_path.display().to_string().replace('\\', "/");
        catalogue.push(serde_json::json!({
            "id": case.slug,
            "format": case.slug,
            "title": case.title,
            "description": case.description,
            "image": format!("{}.svg", case.slug),
            "input": input,
            "source": source,
            "version": version,
            "version_label": version_label,
        }));
    }

    let catalogue_path = Path::new(OUT_DIR).join("catalogue.json");
    std::fs::write(
        &catalogue_path,
        serde_json::to_string_pretty(&serde_json::json!({ "sources": catalogue }))? + "\n",
    )?;

    if selected.is_empty() {
        let keep: std::collections::HashSet<String> = all
            .iter()
            .map(|case| format!("{}.svg", case.slug))
            .collect();
        for entry in std::fs::read_dir(OUT_DIR)? {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == "svg")
                && !keep.contains(&path.file_name().unwrap().to_string_lossy().into_owned())
            {
                std::fs::remove_file(path)?;
            }
        }
    }
    Ok(())
}
