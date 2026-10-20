//! Third-party benchmark formats mapped onto the canonical model.

mod criterion;
mod hyperfine;

pub use criterion::load_criterion;
pub use hyperfine::parse_hyperfine;

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::Value;

    use super::criterion::parse_criterion;
    use super::*;
    use crate::model::Direction;

    #[test]
    fn criterion_time_benchmark_is_ns_per_op() {
        let estimates: Value = serde_json::from_str(
            r#"{
                "mean": {
                    "confidence_interval": {"confidence_level": 0.95, "lower_bound": 980.0, "upper_bound": 1020.0},
                    "point_estimate": 1000.0,
                    "standard_error": 10.0
                },
                "std_dev": {"point_estimate": 40.0},
                "median": {"point_estimate": 995.0}
            }"#,
        )
        .unwrap();
        let manifest: Value = serde_json::from_str(
            r#"{"group_id":"io","function_id":"read","full_id":"io/read","throughput":null}"#,
        )
        .unwrap();
        let benchmark = parse_criterion(&estimates, Some(&manifest), Path::new("io/read")).unwrap();
        assert_eq!(benchmark.unit, "ns/op");
        assert_eq!(benchmark.direction, Direction::Lower);
        assert_eq!(benchmark.id, "io/read");
        assert_eq!(benchmark.label.as_deref(), Some("read"));
        assert!((benchmark.stats.rme_pct.unwrap() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn criterion_throughput_becomes_bytes_per_second() {
        let estimates: Value = serde_json::from_str(
            r#"{"mean":{"point_estimate":2000000.0,"confidence_interval":{"lower_bound":1900000.0,"upper_bound":2100000.0}}}"#,
        )
        .unwrap();
        let manifest: Value = serde_json::from_str(
            r#"{"group_id":"io","function_id":"read","full_id":"io/read","throughput":{"Bytes":8388608}}"#,
        )
        .unwrap();
        let benchmark = parse_criterion(&estimates, Some(&manifest), Path::new("io/read")).unwrap();
        assert_eq!(benchmark.unit, "B/s");
        assert_eq!(benchmark.direction, Direction::Higher);
        // 8 MiB in 2 ms => ~4.19 GB/s
        assert!((benchmark.value - 4_194_304_000.0).abs() < 1.0);
    }

    #[test]
    fn hyperfine_v2_uses_summary_mean() {
        let value: Value = serde_json::from_str(
            r#"{
                "schema_version": 2,
                "metadata": {"primary_metric": "time_wall_clock"},
                "results": [{
                    "command": "rg pattern",
                    "name": "search",
                    "summary": {
                        "time_wall_clock": {"unit": "second", "count": 10, "mean": 0.042, "stddev": 0.002, "median": 0.041, "min": 0.039, "max": 0.048}
                    }
                }]
            }"#,
        )
        .unwrap();
        let rows = parse_hyperfine(&value).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "search");
        assert_eq!(rows[0].unit, "s");
        assert_eq!(rows[0].stats.samples, Some(10));
        assert!((rows[0].value - 0.042).abs() < 1e-12);
    }

    #[test]
    fn hyperfine_v1_flat_shape_still_parses() {
        let value: Value = serde_json::from_str(
            r#"{"results":[{"command":"sleep 0.01","mean":0.011,"stddev":0.0004,"median":0.011,"min":0.010,"max":0.013}]}"#,
        )
        .unwrap();
        let rows = parse_hyperfine(&value).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "sleep 0.01");
        assert!((rows[0].value - 0.011).abs() < 1e-12);
    }
}
