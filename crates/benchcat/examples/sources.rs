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
const DATA_DIR: &str = "target/source-examples";

struct Case {
    slug: &'static str,
    format: Format,
    title: &'static str,
    description: &'static str,
    extension: &'static str,
    data: Option<&'static str>,
    existing: Option<&'static str>,
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
            extension: "txt",
            data: Some(
                "goos: linux\ngoarch: amd64\n\
                 BenchmarkParseJSON-8  200000  812 ns/op\n\
                 BenchmarkEncodeJSON-8  150000  1040 ns/op\n\
                 BenchmarkParseTOML-8  120000  1320 ns/op\n\
                 BenchmarkHash-8  300000  540 ns/op\n",
            ),
            existing: None,
        },
        Case {
            slug: "jmh",
            format: Format::Source("jmh"),
            title: "JMH",
            description: "JMH JSON; ns/op lower is better",
            extension: "json",
            data: Some(
                r#"[
  {"benchmark":"bench.Parse.json","threads":4,"primaryMetric":{"score":812.0,"scoreUnit":"ns/op"}},
  {"benchmark":"bench.Encode.json","threads":4,"primaryMetric":{"score":1040.0,"scoreUnit":"ns/op"}},
  {"benchmark":"bench.Parse.toml","threads":4,"primaryMetric":{"score":1320.0,"scoreUnit":"ns/op"}},
  {"benchmark":"bench.Hash.sha256","threads":4,"primaryMetric":{"score":540.0,"scoreUnit":"ns/op"}}
]"#,
            ),
            existing: None,
        },
        Case {
            slug: "google",
            format: Format::Source("google"),
            title: "Google Benchmark JSON",
            description: "cpu_time normalised to ns/op",
            extension: "json",
            data: Some(
                r#"{"benchmarks":[
  {"name":"BM_Parse/1KiB","cpu_time":612.0,"time_unit":"ns","iterations":1000},
  {"name":"BM_Parse/8KiB","cpu_time":894.0,"time_unit":"ns","iterations":500},
  {"name":"BM_Encode/1KiB","cpu_time":704.0,"time_unit":"ns","iterations":800},
  {"name":"BM_Hash","cpu_time":322.0,"time_unit":"ns","iterations":2000}
]}"#,
            ),
            existing: None,
        },
        Case {
            slug: "google-console",
            format: Format::Source("google-console"),
            title: "Google Benchmark console",
            description: "BM_ columns copied from the console table",
            extension: "txt",
            data: Some(
                "BM_Parse/1KiB  612 ns  600 ns  1000\n\
                 BM_Parse/8KiB  894 ns  880 ns  500\n\
                 BM_Encode/1KiB 704 ns  700 ns  800\n\
                 BM_Hash        322 ns  320 ns  2000\n",
            ),
            existing: None,
        },
        Case {
            slug: "swift",
            format: Format::Source("swift"),
            title: "swift-benchmark",
            description: "swift-benchmark --format json",
            extension: "json",
            data: Some(
                r#"{"benchmarks":[
  {"name":"ParseSuite.json","time":812.0,"median":800.0,"min":790.0,"max":840.0,"std":12.0},
  {"name":"ParseSuite.toml","time":1320.0,"median":1300.0,"min":1280.0,"max":1360.0,"std":18.0},
  {"name":"EncodeSuite.json","time":1040.0,"median":1030.0,"min":1010.0,"max":1070.0,"std":15.0},
  {"name":"HashSuite.sha256","time":540.0,"median":535.0,"min":520.0,"max":560.0,"std":9.0}
]}"#,
            ),
            existing: None,
        },
        Case {
            slug: "androidx",
            format: Format::Source("androidx"),
            title: "AndroidX Benchmark",
            description: "benchmarkData.json; ns/op or ms/op",
            extension: "json",
            data: Some(
                r#"{"benchmarks":[
  {"name":"parseJson","className":"example.ParseBench","metrics":{"timeNs":{"minimum":780.0,"maximum":860.0,"median":812.0,"runs":[780,812,860]}}},
  {"name":"parseToml","className":"example.ParseBench","metrics":{"timeNs":{"minimum":1280.0,"maximum":1360.0,"median":1320.0,"runs":[1280,1320,1360]}}},
  {"name":"frame","className":"example.FrameBench","metrics":{"frameDurationCpuMs":{"minimum":2.0,"maximum":4.0,"median":3.0,"runs":[2,3,4]}}},
  {"name":"hash","className":"example.HashBench","metrics":{"timeNs":{"minimum":520.0,"maximum":560.0,"median":540.0,"runs":[520,540,560]}}}
]}"#,
            ),
            existing: None,
        },
        Case {
            slug: "phpbench",
            format: Format::Source("phpbench"),
            title: "PHPBench",
            description: "PHPBench JSONL report",
            extension: "jsonl",
            data: Some(
                "[{\"benchmark\":\"Bench\",\"subject\":\"parseJson\",\"mean\":12.4,\"unit\":\"us\",\"rstdev\":1.2}]\n\
                 [{\"benchmark\":\"Bench\",\"subject\":\"parseToml\",\"mean\":18.2,\"unit\":\"us\",\"rstdev\":1.8}]\n\
                 [{\"benchmark\":\"Bench\",\"subject\":\"hash\",\"mean\":8.6,\"unit\":\"us\",\"rstdev\":0.9}]\n",
            ),
            existing: None,
        },
        Case {
            slug: "techempower",
            format: Format::Source("techempower"),
            title: "TechEmpower",
            description: "results.json rawData; req/s higher is better",
            extension: "json",
            data: Some(
                r#"{"concurrencyLevels":[16,64],"rawData":{"json":{"framework-a":{"results":[{"requestsPerSecond":120000},{"requestsPerSecond":145000}]},"framework-b":{"results":[{"requestsPerSecond":98000},{"requestsPerSecond":112000}]},"framework-c":{"results":[{"requestsPerSecond":76000},{"requestsPerSecond":88000}]}}}}"#,
            ),
            existing: None,
        },
        Case {
            slug: "benchmark-test-human",
            format: Format::Source("benchmark-test-human"),
            title: "benchmark_test human",
            description: "Dart benchmark_test default output",
            extension: "txt",
            data: Some(
                "Benchmark: parseJson\n  1234.5 ops/sec\n  ±1.25% margin of error\n  10 runs sampled\n\
                 Benchmark: parseToml\n  812.3 ops/sec\n  ±1.60% margin of error\n  10 runs sampled\n\
                 Benchmark: hash\n  2450.8 ops/sec\n  ±0.80% margin of error\n  10 runs sampled\n",
            ),
            existing: None,
        },
        Case {
            slug: "benchmark-test-benchmarkjs",
            format: Format::Source("benchmark-test-benchmarkjs"),
            title: "benchmark_test BenchmarkJS",
            description: "Dart benchmark_test --output=benchmarkjs",
            extension: "txt",
            data: Some(
                "parseJson x 1234.5 ops/sec ±1.25% (10 runs sampled)\n\
                 parseToml x 812.3 ops/sec ±1.60% (10 runs sampled)\n\
                 hash x 2450.8 ops/sec ±0.80% (10 runs sampled)\n",
            ),
            existing: None,
        },
        Case {
            slug: "benchmark-runner",
            format: Format::Source("benchmark-runner"),
            title: "benchmark_runner",
            description: "Dart benchmark_runner report blocks",
            extension: "txt",
            data: Some(
                "[000ms:123us] Parse: json\n  mean: 1234.50 ± 15.20 us, median: 1220.00 ± 20.00 us\n  sample size: 10\n\n\
                 [000ms:456us] Parse: toml\n  mean: 812.30 ± 12.10 us, median: 805.00 ± 15.00 us\n  sample size: 10\n\n\
                 [000ms:789us] Hash: sha256\n  mean: 240.80 ± 3.20 us, median: 238.00 ± 4.00 us\n  sample size: 10\n\n",
            ),
            existing: None,
        },
        Case {
            slug: "criterion",
            format: Format::Criterion,
            title: "Criterion",
            description: "criterion target directory",
            extension: "json",
            data: None,
            existing: Some("examples/criterion"),
        },
        Case {
            slug: "benchmark-test-jsonl",
            format: Format::BenchmarkTest,
            title: "benchmark_test JSONL",
            description: "Dart benchmark_test --output=jsonl",
            extension: "jsonl",
            data: Some(
                "{\"name\":\"parseJson\",\"throughput\":{\"value\":1234.5,\"unit\":\"ops/s\"},\"statistics\":{\"relativeMarginOfError\":1.25},\"compiler\":\"jit\"}\n\
                 {\"name\":\"parseToml\",\"throughput\":{\"value\":812.3,\"unit\":\"ops/s\"},\"statistics\":{\"relativeMarginOfError\":1.6},\"compiler\":\"jit\"}\n\
                 {\"name\":\"hash\",\"throughput\":{\"value\":2450.8,\"unit\":\"ops/s\"},\"statistics\":{\"relativeMarginOfError\":0.8},\"compiler\":\"jit\"}\n",
            ),
            existing: None,
        },
        Case {
            slug: "hyperfine",
            format: Format::Hyperfine,
            title: "hyperfine",
            description: "hyperfine --export-json",
            extension: "json",
            data: None,
            existing: Some("examples/hyperfine.json"),
        },
    ]
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
    std::fs::create_dir_all(DATA_DIR)?;

    let mut catalogue = Vec::new();
    for case in &cases {
        let input_path = match case.existing {
            Some(path) => PathBuf::from(path),
            None => {
                let path = Path::new(DATA_DIR).join(format!("{}.{}", case.slug, case.extension));
                std::fs::write(&path, case.data.unwrap_or_default())?;
                path
            }
        };
        let loaded = input::load(std::slice::from_ref(&input_path), case.format)?;
        let mut meta = loaded.meta.clone();
        meta.tool = Some(case.slug.to_owned());
        let mut config = Config::default();
        config.render.title = Some(case.title.to_owned());
        config.render.subtitle = Some("{rows} rows · mock input · {tool}".to_owned());
        config.render.footer = Some(case.description.to_owned());
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
        let source = if case.slug == "criterion" {
            Some("sources/criterion.oops".to_owned())
        } else {
            matches!(&case.format, Format::Source(_))
                .then(|| format!("sources/{}.oops", case.slug.replace('-', "_")))
        };
        catalogue.push(serde_json::json!({
            "id": case.slug,
            "format": case.slug,
            "title": case.title,
            "description": case.description,
            "image": format!("{}.svg", case.slug),
            "input": input,
            "source": source,
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
