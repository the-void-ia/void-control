#![cfg(feature = "serde")]

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

fn require_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("missing required env var: {name}"))
}

#[derive(Clone, Copy)]
enum DefaultSpecKind {
    LongRunning,
    Timeout,
    BaselineSuccess,
    StructuredOutputSuccess,
    StructuredOutputWithArtifact,
    MissingStructuredOutput,
    MalformedStructuredOutput,
}

static FALLBACK_COUNTER: AtomicU64 = AtomicU64::new(0);

fn resolve_spec_path(env_name: &str, kind: DefaultSpecKind) -> String {
    if let Ok(value) = env::var(env_name) {
        if Path::new(&value).exists() {
            return value;
        }
        eprintln!(
            "[void_box_contract] {} points to missing path '{}'; using generated fallback fixture",
            env_name, value
        );
    }

    let path = fallback_spec_path(kind);
    write_fallback_spec(&path, kind);
    path.to_string_lossy().to_string()
}

fn fallback_spec_path(kind: DefaultSpecKind) -> PathBuf {
    let suffix = match kind {
        DefaultSpecKind::LongRunning => "long_running",
        DefaultSpecKind::Timeout => "timeout",
        DefaultSpecKind::BaselineSuccess => "baseline_success",
        DefaultSpecKind::StructuredOutputSuccess => "structured_output_success",
        DefaultSpecKind::StructuredOutputWithArtifact => "structured_output_with_artifact",
        DefaultSpecKind::MissingStructuredOutput => "missing_structured_output",
        DefaultSpecKind::MalformedStructuredOutput => "malformed_structured_output",
    };
    let nonce = FALLBACK_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let pid = std::process::id();
    env::temp_dir().join(format!(
        "void-control-gate-{suffix}-{pid}-{nanos}-{nonce}.yaml"
    ))
}

fn write_fallback_spec(path: &Path, kind: DefaultSpecKind) {
    let yaml = match kind {
        DefaultSpecKind::LongRunning => {
            r#"api_version: v1
kind: workflow
name: long-running

sandbox:
  mode: mock
  network: false

workflow:
  steps:
    - name: wait
      run:
        program: sleep
        args: ["3"]
    - name: done
      depends_on: [wait]
      run:
        program: echo
        args: ["done"]
  output_step: done
"#
        }
        DefaultSpecKind::Timeout => {
            r#"api_version: v1
kind: workflow
name: timeout-case

sandbox:
  mode: local
  network: false

workflow:
  steps:
    - name: slow
      run:
        program: sleep
        args: ["5"]
  output_step: slow
"#
        }
        DefaultSpecKind::BaselineSuccess => {
            r#"api_version: v1
kind: workflow
name: baseline-success

sandbox:
  mode: mock
  network: false

workflow:
  steps:
    - name: fetch
      run:
        program: echo
        args: ["hello from workflow"]
    - name: transform
      depends_on: [fetch]
      run:
        program: tr
        args: ["a-z", "A-Z"]
        stdin_from: fetch
  output_step: transform
"#
        }
        DefaultSpecKind::StructuredOutputSuccess => {
            r#"api_version: v1
kind: workflow
name: structured-output-success

sandbox:
  mode: mock
  network: false

workflow:
  steps:
    - name: produce
      run:
        program: sh
        args:
          - -lc
          - |
            cat > result.json <<'JSON'
            {"status":"success","summary":"ok","metrics":{"latency_p99_ms":87,"cost_usd":0.018},"artifacts":[]}
            JSON
  output_step: produce
"#
        }
        DefaultSpecKind::StructuredOutputWithArtifact => {
            r#"api_version: v1
kind: workflow
name: structured-output-with-artifact

sandbox:
  mode: mock
  network: false

workflow:
  steps:
    - name: produce
      run:
        program: sh
        args:
          - -lc
          - |
            cat > result.json <<'JSON'
            {"status":"success","summary":"ok","metrics":{"latency_p99_ms":87,"cost_usd":0.018},"artifacts":[{"name":"report.md","stage":"main","media_type":"text/markdown"}]}
            JSON
            cat > report.md <<'MD'
            # report
            artifact content
            MD
  output_step: produce
"#
        }
        DefaultSpecKind::MissingStructuredOutput => {
            r#"api_version: v1
kind: workflow
name: missing-structured-output

sandbox:
  mode: mock
  network: false

workflow:
  steps:
    - name: produce
      run:
        program: sh
        args:
          - -lc
          - |
            echo "completed without result.json"
  output_step: produce
"#
        }
        DefaultSpecKind::MalformedStructuredOutput => {
            r#"api_version: v1
kind: workflow
name: malformed-structured-output

sandbox:
  mode: mock
  network: false

workflow:
  steps:
    - name: produce
      run:
        program: sh
        args:
          - -lc
          - |
            cat > result.json <<'JSON'
            {"status":"success","summary":"ok","metrics":not-json,"artifacts":[]}
            JSON
  output_step: produce
"#
        }
    };

    fs::write(path, yaml).unwrap_or_else(|e| {
        panic!(
            "failed to write fallback spec at '{}': {}",
            path.display(),
            e
        )
    });
}

fn unique_run_id(prefix: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis();
    format!("{prefix}-{now}")
}

fn parse_host_port(base_url: &str) -> (String, u16) {
    let stripped = base_url
        .strip_prefix("http://")
        .expect("VOID_BOX_BASE_URL must start with http://");
    let host_port = stripped.split('/').next().unwrap_or(stripped);
    match host_port.split_once(':') {
        Some((host, port)) => (host.to_string(), port.parse::<u16>().expect("valid port")),
        None => (host_port.to_string(), 80),
    }
}

fn http_request(base_url: &str, method: &str, path: &str, body: Option<&str>) -> (u16, String) {
    let (host, port) = parse_host_port(base_url);
    let mut stream = TcpStream::connect(format!("{host}:{port}")).expect("connect");
    let body = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(request.as_bytes()).expect("write request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");

    let (head, body) = response
        .split_once("\r\n\r\n")
        .expect("response has head/body");
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .expect("status code");
    (status, body.to_string())
}

fn http_get_json(base_url: &str, path: &str) -> (u16, Value) {
    let (status, body) = http_request(base_url, "GET", path, None);
    let json = serde_json::from_str::<Value>(&body).unwrap_or_else(|_| json!({}));
    (status, json)
}

fn http_get_text(base_url: &str, path: &str) -> (u16, String) {
    http_request(base_url, "GET", path, None)
}

fn http_post_json(base_url: &str, path: &str, payload: &Value) -> (u16, Value) {
    let body = payload.to_string();
    let (status, body) = http_request(base_url, "POST", path, Some(&body));
    let json = serde_json::from_str::<Value>(&body).unwrap_or_else(|_| json!({}));
    (status, json)
}

fn assert_error_shape(v: &Value) {
    assert!(
        v.get("code").and_then(Value::as_str).is_some(),
        "missing code"
    );
    assert!(
        v.get("message").and_then(Value::as_str).is_some(),
        "missing message"
    );
    assert!(
        v.get("retryable").and_then(Value::as_bool).is_some(),
        "missing retryable"
    );
}

fn start_payload(run_id: &str, spec_file: &str) -> Value {
    json!({
        "run_id": run_id,
        "file": spec_file,
        "policy": {
            "max_parallel_microvms_per_run": 1,
            "max_stage_retries": 1,
            "stage_timeout_secs": 60,
            "cancel_grace_period_secs": 5
        }
    })
}

fn start_payload_with_policy(run_id: &str, spec_file: &str, policy: Value) -> Value {
    json!({
        "run_id": run_id,
        "file": spec_file,
        "policy": policy
    })
}

fn start_payload_without_policy(run_id: &str, spec_file: &str) -> Value {
    json!({
        "run_id": run_id,
        "file": spec_file
    })
}

fn is_terminal_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "succeeded" | "failed" | "cancelled" | "canceled"
    )
}

fn get_artifact_publication(run: &Value) -> &Value {
    run.get("artifact_publication")
        .unwrap_or_else(|| panic!("missing artifact_publication: {run}"))
}

fn get_manifest_entries(run: &Value) -> &[Value] {
    get_artifact_publication(run)
        .get("manifest")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing artifact manifest: {run}"))
}

fn find_manifest_entry<'a>(run: &'a Value, name: &str) -> &'a Value {
    get_manifest_entries(run)
        .iter()
        .find(|entry| entry.get("name").and_then(Value::as_str) == Some(name))
        .unwrap_or_else(|| panic!("missing manifest entry '{name}': {run}"))
}

fn manifest_retrieval_path(run: &Value, name: &str) -> String {
    let path = find_manifest_entry(run, name)
        .get("retrieval_path")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("manifest entry '{name}' missing retrieval_path: {run}"));
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    }
}

fn wait_until_terminal(base: &str, run_id: &str, timeout_secs: u64) -> Value {
    let attempts = timeout_secs * 10;
    for _ in 0..attempts {
        let (status, run) = http_get_json(base, &format!("/v1/runs/{run_id}"));
        if status == 200 {
            if let Some(s) = run.get("status").and_then(Value::as_str) {
                if is_terminal_status(s) {
                    return run;
                }
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    panic!("run '{run_id}' did not reach terminal state within {timeout_secs}s");
}

fn assert_no_spec_parse_failure(base: &str, run_id: &str) {
    let (status, events) = http_get_json(base, &format!("/v1/runs/{run_id}/events"));
    assert_eq!(status, 200, "failed to fetch events for {run_id}: {events}");
    let events = events
        .as_array()
        .unwrap_or_else(|| panic!("events response is not an array for {run_id}: {events}"));
    for event in events {
        let event_type = event
            .get("event_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let event_type_v2 = event
            .get("event_type_v2")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let message = event
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            event_type != "spec.parse_failed" && event_type_v2 != "SpecParseFailed",
            "run '{run_id}' has spec parse failure event: {event}"
        );
        assert!(
            !message.contains("failed to read"),
            "run '{run_id}' has file-read failure message in event: {event}"
        );
    }
}

#[test]
#[ignore = "requires live void-box daemon"]
fn health_check() {
    let base = require_env("VOID_BOX_BASE_URL");
    let (status, json) = http_get_json(&base, "/v1/health");
    assert_eq!(status, 200);
    assert!(json.get("status").and_then(Value::as_str).is_some());
}

#[test]
#[ignore = "requires live void-box daemon"]
fn start_returns_enriched_contract_fields() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path("VOID_BOX_TEST_SPEC_FILE", DefaultSpecKind::LongRunning);
    let run_id = unique_run_id("contract-start");
    let (status, json) = http_post_json(&base, "/v1/runs", &start_payload(&run_id, &spec));
    assert_eq!(status, 200, "body={json}");
    assert_eq!(
        json.get("run_id").and_then(Value::as_str),
        Some(run_id.as_str())
    );
    assert!(json.get("attempt_id").and_then(Value::as_u64).is_some());
    assert!(json.get("state").and_then(Value::as_str).is_some());
}

#[test]
#[ignore = "requires live void-box daemon"]
fn start_idempotency_active_run() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path("VOID_BOX_TEST_SPEC_FILE", DefaultSpecKind::LongRunning);
    let run_id = unique_run_id("contract-idempotent-start");
    let payload = start_payload(&run_id, &spec);

    let (status_1, json_1) = http_post_json(&base, "/v1/runs", &payload);
    assert_eq!(status_1, 200, "body={json_1}");
    let first_attempt = json_1
        .get("attempt_id")
        .and_then(Value::as_u64)
        .expect("attempt_id");

    let (status_2, json_2) = http_post_json(&base, "/v1/runs", &payload);
    if status_2 == 200 {
        assert_eq!(
            json_2.get("run_id").and_then(Value::as_str),
            Some(run_id.as_str())
        );
        assert_eq!(
            json_2.get("attempt_id").and_then(Value::as_u64),
            Some(first_attempt)
        );
        return;
    }

    // Fast-completing fixtures can transition to terminal between start calls.
    assert_eq!(status_2, 409, "body={json_2}");
    assert_error_shape(&json_2);
    assert_eq!(
        json_2.get("code").and_then(Value::as_str),
        Some("ALREADY_TERMINAL")
    );
}

#[test]
#[ignore = "requires live void-box daemon"]
fn inspect_enriched_fields() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path("VOID_BOX_TEST_SPEC_FILE", DefaultSpecKind::LongRunning);
    let run_id = unique_run_id("contract-inspect");
    let (status_start, _) = http_post_json(&base, "/v1/runs", &start_payload(&run_id, &spec));
    assert_eq!(status_start, 200);

    let (status, json) = http_get_json(&base, &format!("/v1/runs/{run_id}"));
    assert_eq!(status, 200, "body={json}");
    assert!(json.get("id").and_then(Value::as_str).is_some());
    assert!(json.get("status").and_then(Value::as_str).is_some());
    assert!(json.get("attempt_id").and_then(Value::as_u64).is_some());
    assert!(json.get("started_at").and_then(Value::as_str).is_some());
    assert!(json.get("updated_at").and_then(Value::as_str).is_some());
    assert!(json
        .get("active_stage_count")
        .and_then(Value::as_u64)
        .is_some());
    assert!(json
        .get("active_microvm_count")
        .and_then(Value::as_u64)
        .is_some());
}

#[test]
#[ignore = "requires live void-box daemon"]
fn events_envelope_required_fields() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path("VOID_BOX_TEST_SPEC_FILE", DefaultSpecKind::LongRunning);
    let run_id = unique_run_id("contract-events-envelope");
    let (status_start, _) = http_post_json(&base, "/v1/runs", &start_payload(&run_id, &spec));
    assert_eq!(status_start, 200);
    let _ = wait_until_terminal(&base, &run_id, 30);

    let (status, json) = http_get_json(&base, &format!("/v1/runs/{run_id}/events"));
    assert_eq!(status, 200, "body={json}");
    let events = json.as_array().expect("events array");
    let mut seqs = Vec::with_capacity(events.len());
    let mut ids = std::collections::BTreeSet::new();
    for e in events {
        let event_id = e.get("event_id").and_then(Value::as_str).expect("event_id");
        let seq = e.get("seq").and_then(Value::as_u64).expect("seq");
        assert!(
            e.get("event_type").and_then(Value::as_str).is_some(),
            "event_type"
        );
        assert!(
            e.get("attempt_id").and_then(Value::as_u64).is_some(),
            "attempt_id"
        );
        assert!(
            e.get("timestamp").and_then(Value::as_str).is_some(),
            "timestamp"
        );
        assert!(e.get("run_id").and_then(Value::as_str).is_some(), "run_id");
        seqs.push(seq);
        assert!(ids.insert(event_id.to_string()), "duplicate event_id");
    }
    assert!(!seqs.is_empty(), "expected non-empty event list");
    let mut sorted = seqs.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        seqs.len(),
        "seq values must be unique per run+attempt"
    );
    let min = *sorted.first().expect("min seq");
    let max = *sorted.last().expect("max seq");
    assert_eq!(
        max - min + 1,
        sorted.len() as u64,
        "seq values should be gapless per run+attempt"
    );
}

#[test]
#[ignore = "requires live void-box daemon"]
fn events_resume_from_event_id() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path("VOID_BOX_TEST_SPEC_FILE", DefaultSpecKind::LongRunning);
    let run_id = unique_run_id("contract-events-resume");
    let (status_start, _) = http_post_json(&base, "/v1/runs", &start_payload(&run_id, &spec));
    assert_eq!(status_start, 200);

    let (status_all, json_all) = http_get_json(&base, &format!("/v1/runs/{run_id}/events"));
    assert_eq!(status_all, 200);
    let events = json_all.as_array().expect("events array");
    let first_id = events
        .first()
        .and_then(|e| e.get("event_id"))
        .and_then(Value::as_str)
        .expect("first event id");

    let (status_resume, json_resume) = http_get_json(
        &base,
        &format!("/v1/runs/{run_id}/events?from_event_id={first_id}"),
    );
    assert_eq!(status_resume, 200);
    let resumed = json_resume.as_array().expect("resumed array");
    if !resumed.is_empty() {
        let resumed_first = resumed[0]
            .get("event_id")
            .and_then(Value::as_str)
            .expect("event id");
        assert_ne!(resumed_first, first_id);
    }

    let (status_missing, json_missing) = http_get_json(
        &base,
        &format!("/v1/runs/{run_id}/events?from_event_id=evt_missing"),
    );
    assert_eq!(status_missing, 200);
    assert!(json_missing.as_array().is_some());
}

#[test]
#[ignore = "requires live void-box daemon"]
fn cancel_returns_terminal_response_shape() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path("VOID_BOX_TEST_SPEC_FILE", DefaultSpecKind::LongRunning);
    let run_id = unique_run_id("contract-cancel-shape");
    let (status_start, _) = http_post_json(&base, "/v1/runs", &start_payload(&run_id, &spec));
    assert_eq!(status_start, 200);

    let (status_cancel, json_cancel) = http_post_json(
        &base,
        &format!("/v1/runs/{run_id}/cancel"),
        &json!({"reason":"test cancel"}),
    );
    assert_eq!(status_cancel, 200, "body={json_cancel}");
    assert_eq!(
        json_cancel.get("run_id").and_then(Value::as_str),
        Some(run_id.as_str())
    );
    assert!(json_cancel.get("state").and_then(Value::as_str).is_some());
    assert!(json_cancel
        .get("terminal_event_id")
        .and_then(Value::as_str)
        .is_some());
}

#[test]
#[ignore = "requires live void-box daemon"]
fn cancel_idempotency() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path("VOID_BOX_TEST_SPEC_FILE", DefaultSpecKind::LongRunning);
    let run_id = unique_run_id("contract-cancel-idempotent");
    let (status_start, _) = http_post_json(&base, "/v1/runs", &start_payload(&run_id, &spec));
    assert_eq!(status_start, 200);

    let (status_1, json_1) = http_post_json(
        &base,
        &format!("/v1/runs/{run_id}/cancel"),
        &json!({"reason":"idempotency-1"}),
    );
    assert_eq!(status_1, 200, "body={json_1}");
    let first_terminal = json_1
        .get("terminal_event_id")
        .and_then(Value::as_str)
        .expect("terminal_event_id")
        .to_string();

    let (status_2, json_2) = http_post_json(
        &base,
        &format!("/v1/runs/{run_id}/cancel"),
        &json!({"reason":"idempotency-2"}),
    );
    assert_eq!(status_2, 200, "body={json_2}");
    assert_eq!(
        json_2.get("terminal_event_id").and_then(Value::as_str),
        Some(first_terminal.as_str())
    );
}

#[test]
#[ignore = "requires live void-box daemon"]
fn structured_error_not_found() {
    let base = require_env("VOID_BOX_BASE_URL");
    let (status, json) = http_get_json(&base, "/v1/runs/does-not-exist");
    assert!(status >= 400);
    assert_error_shape(&json);
    assert_eq!(json.get("code").and_then(Value::as_str), Some("NOT_FOUND"));
    assert_eq!(json.get("retryable").and_then(Value::as_bool), Some(false));
}

#[test]
#[ignore = "requires live void-box daemon"]
fn structured_error_invalid_policy() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path("VOID_BOX_TEST_SPEC_FILE", DefaultSpecKind::LongRunning);
    let run_id = unique_run_id("contract-bad-policy");
    let payload = json!({
        "run_id": run_id,
        "file": spec,
        "policy": {
            "max_parallel_microvms_per_run": 0,
            "max_stage_retries": 1,
            "stage_timeout_secs": 60,
            "cancel_grace_period_secs": 5
        }
    });
    let (status, json) = http_post_json(&base, "/v1/runs", &payload);
    assert!(status >= 400);
    assert_error_shape(&json);
    assert_eq!(
        json.get("code").and_then(Value::as_str),
        Some("INVALID_POLICY")
    );
    assert_eq!(json.get("retryable").and_then(Value::as_bool), Some(false));
}

#[test]
#[ignore = "requires live void-box daemon"]
fn structured_output_result_json_is_retrievable() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path(
        "VOID_BOX_STRUCTURED_OUTPUT_SPEC_FILE",
        DefaultSpecKind::StructuredOutputSuccess,
    );
    let run_id = unique_run_id("contract-structured-output");
    let (status_start, body_start) =
        http_post_json(&base, "/v1/runs", &start_payload(&run_id, &spec));
    assert_eq!(status_start, 200, "body={body_start}");

    let terminal = wait_until_terminal(&base, &run_id, 30);
    assert_eq!(
        terminal
            .get("status")
            .and_then(Value::as_str)
            .map(|s| s.to_ascii_lowercase()),
        Some("succeeded".to_string()),
        "terminal={terminal}"
    );

    let (status, body) =
        http_get_text(&base, &format!("/v1/runs/{run_id}/stages/main/output-file"));
    assert_eq!(status, 200, "body={body}");
    let parsed = serde_json::from_str::<Value>(&body)
        .unwrap_or_else(|e| panic!("structured output was not valid JSON: {e}; body={body}"));
    assert!(parsed.get("metrics").and_then(Value::as_object).is_some());
}

#[test]
#[ignore = "requires live void-box daemon"]
fn missing_result_json_is_typed_failure() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path(
        "VOID_BOX_MISSING_STRUCTURED_OUTPUT_SPEC_FILE",
        DefaultSpecKind::MissingStructuredOutput,
    );
    let run_id = unique_run_id("contract-missing-structured-output");
    let (status_start, body_start) =
        http_post_json(&base, "/v1/runs", &start_payload(&run_id, &spec));
    assert_eq!(status_start, 200, "body={body_start}");

    let terminal = wait_until_terminal(&base, &run_id, 30);
    assert_eq!(
        terminal
            .get("status")
            .and_then(Value::as_str)
            .map(|s| s.to_ascii_lowercase()),
        Some("failed".to_string()),
        "terminal={terminal}"
    );

    let (status, json) =
        http_get_json(&base, &format!("/v1/runs/{run_id}/stages/main/output-file"));
    assert!(status >= 400, "body={json}");
    assert_error_shape(&json);
    assert_eq!(
        json.get("code").and_then(Value::as_str),
        Some("STRUCTURED_OUTPUT_MISSING")
    );
}

#[test]
#[ignore = "requires live void-box daemon"]
fn malformed_result_json_is_typed_failure() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path(
        "VOID_BOX_MALFORMED_STRUCTURED_OUTPUT_SPEC_FILE",
        DefaultSpecKind::MalformedStructuredOutput,
    );
    let run_id = unique_run_id("contract-malformed-structured-output");
    let (status_start, body_start) =
        http_post_json(&base, "/v1/runs", &start_payload(&run_id, &spec));
    assert_eq!(status_start, 200, "body={body_start}");

    let terminal = wait_until_terminal(&base, &run_id, 30);
    assert_eq!(
        terminal
            .get("status")
            .and_then(Value::as_str)
            .map(|s| s.to_ascii_lowercase()),
        Some("failed".to_string()),
        "terminal={terminal}"
    );

    let (status, json) =
        http_get_json(&base, &format!("/v1/runs/{run_id}/stages/main/output-file"));
    assert!(status >= 400, "body={json}");
    assert_error_shape(&json);
    assert_eq!(
        json.get("code").and_then(Value::as_str),
        Some("STRUCTURED_OUTPUT_MALFORMED")
    );
}

#[test]
#[ignore = "requires live void-box daemon"]
fn manifest_lists_named_artifacts() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path(
        "VOID_BOX_STRUCTURED_OUTPUT_ARTIFACT_SPEC_FILE",
        DefaultSpecKind::StructuredOutputWithArtifact,
    );
    let run_id = unique_run_id("contract-artifact-manifest");
    let (status_start, body_start) =
        http_post_json(&base, "/v1/runs", &start_payload(&run_id, &spec));
    assert_eq!(status_start, 200, "body={body_start}");

    let terminal = wait_until_terminal(&base, &run_id, 30);
    assert_eq!(
        terminal
            .get("status")
            .and_then(Value::as_str)
            .map(|s| s.to_ascii_lowercase()),
        Some("succeeded".to_string()),
        "terminal={terminal}"
    );

    let (status, inspect) = http_get_json(&base, &format!("/v1/runs/{run_id}"));
    assert_eq!(status, 200, "body={inspect}");
    assert!(inspect.get("artifact_publication").is_some());
    assert_eq!(
        get_artifact_publication(&inspect)
            .get("status")
            .and_then(Value::as_str),
        Some("published")
    );
    let manifest = get_manifest_entries(&inspect);
    assert!(
        manifest
            .iter()
            .any(|entry| entry.get("name").and_then(Value::as_str) == Some("result.json")),
        "manifest missing result.json: {inspect}"
    );
    let artifact_entry = find_manifest_entry(&inspect, "report.md");
    assert_eq!(
        artifact_entry.get("stage").and_then(Value::as_str),
        Some("main")
    );
    assert!(
        artifact_entry
            .get("retrieval_path")
            .and_then(Value::as_str)
            .is_some(),
        "artifact entry missing retrieval_path: {artifact_entry}"
    );
}

#[test]
#[ignore = "requires live void-box daemon"]
fn named_artifact_endpoint_serves_manifested_file() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path(
        "VOID_BOX_STRUCTURED_OUTPUT_ARTIFACT_SPEC_FILE",
        DefaultSpecKind::StructuredOutputWithArtifact,
    );
    let run_id = unique_run_id("contract-named-artifact");
    let (status_start, body_start) =
        http_post_json(&base, "/v1/runs", &start_payload(&run_id, &spec));
    assert_eq!(status_start, 200, "body={body_start}");

    let _ = wait_until_terminal(&base, &run_id, 30);
    let (status_inspect, inspect) = http_get_json(&base, &format!("/v1/runs/{run_id}"));
    assert_eq!(status_inspect, 200, "body={inspect}");

    let path = manifest_retrieval_path(&inspect, "report.md");
    let (status, body) = http_get_text(&base, &path);
    assert_eq!(status, 200, "body={body}");
    assert!(
        body.contains("artifact content"),
        "unexpected artifact body={body}"
    );
}

#[test]
#[ignore = "requires live void-box daemon"]
fn active_run_listing_supports_reconciliation() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path("VOID_BOX_TEST_SPEC_FILE", DefaultSpecKind::LongRunning);
    let run_id = unique_run_id("contract-active-reconciliation");
    let (status_start, body_start) =
        http_post_json(&base, "/v1/runs", &start_payload(&run_id, &spec));
    assert_eq!(status_start, 200, "body={body_start}");

    let (status_active, active) = http_get_json(&base, "/v1/runs?state=active");
    assert_eq!(status_active, 200, "body={active}");
    let runs = active
        .get("runs")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("active runs payload missing runs array: {active}"));
    let matching = runs.iter().find(|run| {
        run.get("run_id").and_then(Value::as_str) == Some(run_id.as_str())
            || run.get("id").and_then(Value::as_str) == Some(run_id.as_str())
    });
    let matching =
        matching.unwrap_or_else(|| panic!("started run not present in active listing: {active}"));
    assert!(matching.get("attempt_id").and_then(Value::as_u64).is_some());
    assert!(matching
        .get("active_stage_count")
        .and_then(Value::as_u64)
        .is_some());
    assert!(matching
        .get("active_microvm_count")
        .and_then(Value::as_u64)
        .is_some());

    let (status_terminal, terminal) = http_get_json(&base, "/v1/runs?state=terminal");
    assert_eq!(status_terminal, 200, "body={terminal}");
    assert!(terminal.get("runs").and_then(Value::as_array).is_some());
}

#[test]
#[ignore = "requires live void-box daemon"]
fn already_terminal_start_behavior() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path("VOID_BOX_TEST_SPEC_FILE", DefaultSpecKind::LongRunning);
    let run_id = unique_run_id("contract-terminal-start");
    let payload = start_payload(&run_id, &spec);

    let (status_start, _) = http_post_json(&base, "/v1/runs", &payload);
    assert_eq!(status_start, 200);
    let (status_cancel, _) = http_post_json(
        &base,
        &format!("/v1/runs/{run_id}/cancel"),
        &json!({"reason":"terminalize"}),
    );
    assert_eq!(status_cancel, 200);

    let (status_restart, json_restart) = http_post_json(&base, "/v1/runs", &payload);
    assert!(status_restart >= 400, "body={json_restart}");
    assert_error_shape(&json_restart);
    assert_eq!(
        json_restart.get("code").and_then(Value::as_str),
        Some("ALREADY_TERMINAL")
    );
    assert_eq!(
        json_restart.get("retryable").and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
#[ignore = "requires live void-box daemon and timeout fixture"]
fn policy_timeout_enforced_failure() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path("VOID_BOX_TIMEOUT_SPEC_FILE", DefaultSpecKind::Timeout);
    let run_id = unique_run_id("contract-policy-timeout");
    let payload = start_payload_with_policy(
        &run_id,
        &spec,
        json!({
            "max_parallel_microvms_per_run": 2,
            "max_stage_retries": 1,
            "stage_timeout_secs": 1,
            "cancel_grace_period_secs": 5
        }),
    );
    let (status_start, body_start) = http_post_json(&base, "/v1/runs", &payload);
    assert_eq!(status_start, 200, "body={body_start}");

    let terminal = wait_until_terminal(&base, &run_id, 30);
    assert_no_spec_parse_failure(&base, &run_id);
    let status = terminal
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    assert_eq!(status, "failed", "terminal={terminal}");
}

#[test]
#[ignore = "requires live void-box daemon and parallel fixture"]
fn policy_parallel_limit_caps_active_microvms() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path("VOID_BOX_PARALLEL_SPEC_FILE", DefaultSpecKind::LongRunning);
    let run_id = unique_run_id("contract-policy-parallel");
    let payload = start_payload_with_policy(
        &run_id,
        &spec,
        json!({
            "max_parallel_microvms_per_run": 1,
            "max_stage_retries": 1,
            "stage_timeout_secs": 120,
            "cancel_grace_period_secs": 5
        }),
    );
    let (status_start, body_start) = http_post_json(&base, "/v1/runs", &payload);
    assert_eq!(status_start, 200, "body={body_start}");

    let mut max_seen = 0u64;
    for _ in 0..300 {
        let (status, run) = http_get_json(&base, &format!("/v1/runs/{run_id}"));
        assert_eq!(status, 200, "body={run}");
        if let Some(active) = run.get("active_microvm_count").and_then(Value::as_u64) {
            max_seen = max_seen.max(active);
        }
        if let Some(s) = run.get("status").and_then(Value::as_str) {
            if is_terminal_status(s) {
                break;
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert_no_spec_parse_failure(&base, &run_id);
    assert!(max_seen <= 1, "max active_microvm_count was {max_seen}");
}

#[test]
#[ignore = "requires live void-box daemon and retry fixture"]
fn policy_retry_cap_is_persisted() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path("VOID_BOX_RETRY_SPEC_FILE", DefaultSpecKind::LongRunning);
    let run_id = unique_run_id("contract-policy-retry-persist");
    let payload = start_payload_with_policy(
        &run_id,
        &spec,
        json!({
            "max_parallel_microvms_per_run": 1,
            "max_stage_retries": 0,
            "stage_timeout_secs": 60,
            "cancel_grace_period_secs": 5
        }),
    );
    let (status_start, body_start) = http_post_json(&base, "/v1/runs", &payload);
    assert_eq!(status_start, 200, "body={body_start}");

    let (status, run) = http_get_json(&base, &format!("/v1/runs/{run_id}"));
    assert_eq!(status, 200, "body={run}");
    assert_no_spec_parse_failure(&base, &run_id);
    let policy = run.get("policy").expect("policy present");
    assert_eq!(
        policy.get("max_stage_retries").and_then(Value::as_u64),
        Some(0)
    );
}

#[test]
#[ignore = "requires live void-box daemon and retry fixture"]
fn policy_retry_cap_reduces_event_churn() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path("VOID_BOX_RETRY_SPEC_FILE", DefaultSpecKind::LongRunning);

    let run_a = unique_run_id("contract-policy-retry-a");
    let payload_a = start_payload_with_policy(
        &run_a,
        &spec,
        json!({
            "max_parallel_microvms_per_run": 1,
            "max_stage_retries": 0,
            "stage_timeout_secs": 60,
            "cancel_grace_period_secs": 5
        }),
    );
    let (status_a, body_a) = http_post_json(&base, "/v1/runs", &payload_a);
    assert_eq!(status_a, 200, "body={body_a}");
    let _ = wait_until_terminal(&base, &run_a, 30);

    let run_b = unique_run_id("contract-policy-retry-b");
    let payload_b = start_payload_with_policy(
        &run_b,
        &spec,
        json!({
            "max_parallel_microvms_per_run": 1,
            "max_stage_retries": 2,
            "stage_timeout_secs": 60,
            "cancel_grace_period_secs": 5
        }),
    );
    let (status_b, body_b) = http_post_json(&base, "/v1/runs", &payload_b);
    assert_eq!(status_b, 200, "body={body_b}");
    let _ = wait_until_terminal(&base, &run_b, 30);

    let (status_events_a, events_a) = http_get_json(&base, &format!("/v1/runs/{run_a}/events"));
    let (status_events_b, events_b) = http_get_json(&base, &format!("/v1/runs/{run_b}/events"));
    assert_eq!(status_events_a, 200, "body={events_a}");
    assert_eq!(status_events_b, 200, "body={events_b}");
    let len_a = events_a.as_array().map(|a| a.len()).unwrap_or_default();
    let len_b = events_b.as_array().map(|a| a.len()).unwrap_or_default();
    assert_no_spec_parse_failure(&base, &run_a);
    assert_no_spec_parse_failure(&base, &run_b);
    assert!(
        len_b >= len_a,
        "expected retries=2 run to have >= event count than retries=0 (a={len_a}, b={len_b})"
    );
}

#[test]
#[ignore = "requires live void-box daemon and no-policy baseline fixture"]
fn policy_no_policy_regression_allows_completion() {
    let base = require_env("VOID_BOX_BASE_URL");
    let spec = resolve_spec_path(
        "VOID_BOX_NO_POLICY_SPEC_FILE",
        DefaultSpecKind::BaselineSuccess,
    );
    let run_id = unique_run_id("contract-policy-no-policy");
    let payload = start_payload_without_policy(&run_id, &spec);
    let (status_start, body_start) = http_post_json(&base, "/v1/runs", &payload);
    assert_eq!(status_start, 200, "body={body_start}");

    let terminal = wait_until_terminal(&base, &run_id, 30);
    assert_no_spec_parse_failure(&base, &run_id);
    let status = terminal
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    assert_eq!(status, "succeeded", "terminal={terminal}");
}
