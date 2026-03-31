#![cfg(feature = "serde")]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[test]
fn transform_benchmark_produces_measured_metrics_and_strategy_tradeoffs() {
    let baseline = run_benchmark(&[
        ("TRANSFORM_STRATEGY", "baseline"),
        ("TRANSFORM_ROLE", "latency-baseline"),
    ]);
    let cache = run_benchmark(&[
        ("TRANSFORM_STRATEGY", "cache-aware"),
        ("TRANSFORM_CACHE_MODE", "hot-path"),
        ("TRANSFORM_ROLE", "cache-locality"),
    ]);
    let strict = run_benchmark(&[
        ("TRANSFORM_STRATEGY", "conservative-validation"),
        ("TRANSFORM_VALIDATION_MODE", "strict"),
        ("TRANSFORM_ROLE", "validation-risk"),
    ]);
    let low_cpu = run_benchmark(&[
        ("TRANSFORM_STRATEGY", "low-cpu"),
        ("TRANSFORM_CPU_BUDGET", "55"),
        ("TRANSFORM_ROLE", "cpu-budget"),
    ]);

    assert_eq!(baseline["status"], "success");
    assert!(baseline["metrics"]["latency_p99_ms"].as_f64().unwrap() > 0.0);
    assert!((0.0..=1.0).contains(&baseline["metrics"]["error_rate"].as_f64().unwrap()));
    assert!((0.0..=100.0).contains(&baseline["metrics"]["cpu_pct"].as_f64().unwrap()));

    assert!(
        cache["metrics"]["latency_p99_ms"].as_f64().unwrap()
            < baseline["metrics"]["latency_p99_ms"].as_f64().unwrap(),
        "cache-aware should beat baseline on p99 latency"
    );
    assert!(
        strict["metrics"]["error_rate"].as_f64().unwrap()
            < baseline["metrics"]["error_rate"].as_f64().unwrap(),
        "strict validation should reduce error rate"
    );
    assert!(
        low_cpu["metrics"]["cpu_pct"].as_f64().unwrap()
            < baseline["metrics"]["cpu_pct"].as_f64().unwrap(),
        "low-cpu should lower cpu_pct"
    );
}

fn run_benchmark(envs: &[(&str, &str)]) -> Value {
    let output_path = temp_output_path();
    let mut command = Command::new("python3");
    command.arg("examples/void-box/transform_benchmark.py");
    command.current_dir(repo_root());
    command.env("OUTPUT_FILE", &output_path);
    for (key, value) in envs {
        command.env(key, value);
    }

    let output = command.output().expect("spawn benchmark");
    assert!(
        output.status.success(),
        "benchmark failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let body = fs::read_to_string(&output_path).expect("read benchmark output");
    serde_json::from_str(&body).expect("parse benchmark output json")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn temp_output_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("transform-benchmark-{nanos}.json"))
}
