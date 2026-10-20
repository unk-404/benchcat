use benchcat::sources;

fn parse(name: &str, text: &str) -> benchcat::input::Loaded {
    let source = sources::find(name).unwrap_or_else(|| panic!("missing source {name}"));
    (source.parse)(text).unwrap_or_else(|error| panic!("{name}: {error}"))
}

#[test]
fn go_columns_parser_skips_headers_and_maps_columns() {
    let loaded = parse(
        "go",
        "goos: linux\nBenchmarkRead-8  1000000  1234 ns/op  512 B/op\n",
    );
    assert_eq!(loaded.benchmarks.len(), 1);
    let row = &loaded.benchmarks[0];
    assert_eq!(row.id, "BenchmarkRead-8");
    assert_eq!(row.label.as_deref(), Some("Read"));
    assert_eq!(row.value, 1234.0);
    assert_eq!(row.unit, "ns/op");
}

#[test]
fn benchmark_test_human_uses_records_not_regex_rows() {
    let loaded = parse(
        "benchmark-test-human",
        "Benchmark: parse json\n  1234.5 ops/sec\n  ±1.25% margin of error\n  10 runs sampled\n",
    );
    let row = &loaded.benchmarks[0];
    assert_eq!(row.value, 1234.5);
    assert_eq!(row.stats.rme_pct, Some(1.25));
    assert_eq!(row.stats.samples, Some(10));
}

#[test]
fn benchmark_runner_records_extract_key_values() {
    let loaded = parse(
        "benchmark-runner",
        "[000ms:123us] Set: construct\n  mean: 123.45 ± 1.20 us, median: 120.00 ± 2.00 us\n  sample size: 10\n",
    );
    let row = &loaded.benchmarks[0];
    assert_eq!(row.group.as_deref(), Some("Set"));
    assert_eq!(row.label.as_deref(), Some("construct"));
    assert_eq!(row.unit, "us/op");
    assert_eq!(row.stats.median, Some(120.0));
    assert_eq!(row.stats.samples, Some(10));
}

#[test]
fn benchmarkjs_line_parser_extracts_tokens() {
    let loaded = parse(
        "benchmark-test-benchmarkjs",
        "parse json x 1234.5 ops/sec ±1.25% (10 runs sampled)\n",
    );
    let row = &loaded.benchmarks[0];
    assert_eq!(row.id, "parse json");
    assert_eq!(row.value, 1234.5);
    assert_eq!(row.stats.samples, Some(10));
}

#[test]
fn androidx_uses_the_same_metric_for_value_and_stats() {
    let loaded = parse(
        "androidx",
        r#"{"benchmarks":[{"name":"frame","className":"example.Bench",
        "metrics":{"frameDurationCpuMs":{"minimum":2.0,"maximum":4.0,"median":3.0}}}]}"#,
    );
    let row = &loaded.benchmarks[0];
    assert_eq!(row.unit, "ms/op");
    assert_eq!(row.value, 3.0);
    assert_eq!(row.stats.median, Some(3.0));
    assert_eq!(row.stats.min, Some(2.0));
    assert_eq!(row.stats.max, Some(4.0));
}

#[test]
fn benchmarkdotnet_v2_markdown_and_csv_have_the_same_shape() {
    let markdown = "| Method | Mean | Error | StdDev | Median |\n|------- |-----:|------:|-------:|-------:|\n| Parse | 1.234 ns | 0.012 ns | 0.011 ns | 1.230 ns |\n";
    let csv = "\"Method\",\"Mean\",\"Error\",\"StdDev\",\"Median\"\n\"Parse\",\"1.234 ns\",\"0.012 ns\",\"0.011 ns\",\"1.230 ns\"\n";
    for text in [markdown, csv] {
        let loaded = parse("benchmarkdotnet", text);
        let row = &loaded.benchmarks[0];
        assert_eq!(row.id, "Parse");
        assert_eq!(row.unit, "ns/op");
        assert_eq!(row.value, 1.234);
        assert_eq!(row.stats.median, Some(1.23));
    }
}
