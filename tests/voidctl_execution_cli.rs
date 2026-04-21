#![cfg(feature = "serde")]

use std::collections::VecDeque;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::json;

#[derive(Debug, Clone)]
struct FakeResponse {
    status: u16,
    body: serde_json::Value,
}

#[derive(Debug, Clone)]
struct RecordedRequest {
    method: String,
    path: String,
    body: String,
}

fn temp_inputs_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("voidctl-test-{}-{name}", std::process::id()));
    path
}

fn spawn_fake_bridge(
    responses: Vec<FakeResponse>,
) -> (
    String,
    Arc<Mutex<Vec<RecordedRequest>>>,
    thread::JoinHandle<()>,
) {
    let request_count = responses.len();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake bridge");
    let address = listener.local_addr().expect("listener address");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let requests_clone = Arc::clone(&requests);
    let responses = Arc::new(Mutex::new(VecDeque::from(responses)));
    let responses_clone = Arc::clone(&responses);

    let handle = thread::spawn(move || {
        for _ in 0..request_count {
            let mut stream = match listener.accept() {
                Ok((stream, _)) => stream,
                Err(_) => break,
            };

            let mut buffer = Vec::new();
            let mut header_end = None;
            loop {
                let mut chunk = [0u8; 1024];
                let read = match stream.read(&mut chunk) {
                    Ok(read) => read,
                    Err(_) => return,
                };
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
                    header_end = Some(position + 4);
                    break;
                }
            }

            let Some(header_end) = header_end else {
                break;
            };
            let head = String::from_utf8(buffer[..header_end].to_vec()).expect("utf8 headers");
            let mut content_length = 0usize;
            for line in head.lines() {
                let line = line.trim();
                let Some((name, value)) = line.split_once(':') else {
                    continue;
                };
                if name.eq_ignore_ascii_case("Content-Length") {
                    content_length = value.trim().parse::<usize>().expect("content length");
                }
            }
            while buffer.len() < header_end + content_length {
                let mut chunk = [0u8; 1024];
                let read = match stream.read(&mut chunk) {
                    Ok(read) => read,
                    Err(_) => return,
                };
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
            }
            let body = String::from_utf8(buffer[header_end..header_end + content_length].to_vec())
                .expect("utf8 body");

            let mut lines = head.lines();
            let Some(request_line) = lines.next() else {
                break;
            };
            let parts = request_line.split_whitespace().collect::<Vec<_>>();
            if parts.len() < 2 {
                break;
            }
            let record = RecordedRequest {
                method: parts[0].to_string(),
                path: parts[1].to_string(),
                body,
            };
            requests_clone.lock().expect("lock requests").push(record);

            let response = responses_clone
                .lock()
                .expect("lock responses")
                .pop_front()
                .unwrap_or_else(|| FakeResponse {
                    status: 500,
                    body: json!({ "message": "unexpected request" }),
                });
            let status_line = match response.status {
                200 => "HTTP/1.1 200 OK",
                400 => "HTTP/1.1 400 Bad Request",
                404 => "HTTP/1.1 404 Not Found",
                500 => "HTTP/1.1 500 Internal Server Error",
                _ => "HTTP/1.1 200 OK",
            };
            let body = response.body.to_string();
            let reply = format!(
                "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(reply.as_bytes());
        }
    });

    (format!("http://{address}"), requests, handle)
}

fn voidctl_command(base_url: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_voidctl"));
    command.env("VOID_CONTROL_BRIDGE_BASE_URL", base_url);
    command
}

#[test]
fn submit_from_stdin_posts_spec_and_prints_execution_summary() {
    let (base_url, requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 200,
        body: json!({
            "execution_id": "exec-stdin-1",
            "status": "Pending",
            "mode": "swarm",
            "goal": "generated spec",
            "completed_iterations": 0,
            "result_best_candidate_id": null
        }),
    }]);

    let mut child = voidctl_command(&base_url)
        .args(["execution", "submit", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn voidctl");
    let spec = "mode: swarm\ngoal: generated spec\n";
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(spec.as_bytes())
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait output");
    server.join().expect("join fake bridge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("execution_id=exec-stdin-1"));
    assert!(stdout.contains("status=Pending"));
    assert!(stdout.contains("goal=generated spec"));

    let requests = requests.lock().expect("lock requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/executions");
    assert_eq!(requests[0].body, spec);
}

#[test]
fn dry_run_validation_failure_returns_non_zero_and_prints_errors() {
    let (base_url, _requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 400,
        body: json!({
            "valid": false,
            "errors": ["missing goal", "missing workflow template"]
        }),
    }]);

    let output = voidctl_command(&base_url)
        .args(["execution", "dry-run", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .take()
                .expect("stdin")
                .write_all(b"mode: swarm\n")
                .expect("write stdin");
            child.wait_with_output()
        })
        .expect("wait output");
    server.join().expect("join fake bridge");

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stdout.contains("valid=false"));
    assert!(stdout.contains("error=missing goal"));
    assert!(stdout.contains("error=missing workflow template"));
    assert!(stderr.contains("fatal: dry-run validation failed"));
}

#[test]
fn watch_prints_events_and_stops_when_execution_is_terminal() {
    let (base_url, requests, server) = spawn_fake_bridge(vec![
        FakeResponse {
            status: 200,
            body: json!({
                "execution": {
                    "execution_id": "exec-watch-1",
                    "status": "Completed",
                    "result_best_candidate_id": "candidate-3"
                },
                "result": {
                    "completed_iterations": 1
                },
                "progress": {
                    "queued_candidate_count": 0,
                    "running_candidate_count": 0,
                    "completed_candidate_count": 3,
                    "failed_candidate_count": 0
                }
            }),
        },
        FakeResponse {
            status: 200,
            body: json!({
                "events": [
                    { "seq": 7, "event_type": "CandidateScored" },
                    { "seq": 8, "event_type": "ExecutionCompleted" }
                ]
            }),
        },
    ]);

    let output = voidctl_command(&base_url)
        .args(["execution", "watch", "exec-watch-1"])
        .output()
        .expect("watch output");
    server.join().expect("join fake bridge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("execution_id=exec-watch-1 status=Completed"));
    assert!(stdout.contains("event seq=7 type=CandidateScored"));
    assert!(stdout.contains("event seq=8 type=ExecutionCompleted"));

    let requests = requests.lock().expect("lock requests");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].path, "/v1/executions/exec-watch-1");
    assert_eq!(requests[1].path, "/v1/executions/exec-watch-1/events");
}

#[test]
fn inspect_prints_execution_summary_and_candidates() {
    let (base_url, _requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 200,
        body: json!({
            "execution": {
                "execution_id": "exec-inspect-1",
                "status": "Running",
                "mode": "swarm",
                "goal": "inspect me"
            },
            "progress": {
                "queued_candidate_count": 1,
                "running_candidate_count": 1,
                "completed_candidate_count": 0,
                "failed_candidate_count": 0,
                "canceled_candidate_count": 0
            },
            "candidates": [
                {
                    "candidate_id": "candidate-1",
                    "status": "Running",
                    "runtime_run_id": "run-1",
                    "metrics": { "cpu_pct": 88.0 }
                }
            ]
        }),
    }]);

    let output = voidctl_command(&base_url)
        .args(["execution", "inspect", "exec-inspect-1"])
        .output()
        .expect("inspect output");
    server.join().expect("join fake bridge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("execution_id=exec-inspect-1"));
    assert!(stdout.contains("goal=inspect me"));
    assert!(stdout.contains("candidate_id=candidate-1 status=Running runtime_run_id=run-1"));
    assert!(stdout.contains("\"cpu_pct\":88.0"));
}

#[test]
fn events_prints_event_sequence_and_type() {
    let (base_url, _requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 200,
        body: json!({
            "events": [
                { "seq": 3, "event_type": "ExecutionSubmitted" },
                { "seq": 4, "event_type": "CandidateQueued" }
            ]
        }),
    }]);

    let output = voidctl_command(&base_url)
        .args(["execution", "events", "exec-events-1"])
        .output()
        .expect("events output");
    server.join().expect("join fake bridge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("seq=3 event_type=ExecutionSubmitted"));
    assert!(stdout.contains("seq=4 event_type=CandidateQueued"));
}

#[test]
fn result_prints_winner_and_candidate_metrics() {
    let (base_url, _requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 200,
        body: json!({
            "execution": {
                "execution_id": "exec-result-1",
                "status": "Completed",
                "mode": "swarm",
                "goal": "optimize",
                "result_best_candidate_id": "candidate-2"
            },
            "result": {
                "completed_iterations": 1,
                "best_candidate_id": "candidate-2"
            },
            "progress": {
                "queued_candidate_count": 0,
                "running_candidate_count": 0,
                "completed_candidate_count": 2,
                "failed_candidate_count": 0,
                "canceled_candidate_count": 0
            },
            "candidates": [
                {
                    "candidate_id": "candidate-1",
                    "status": "Completed",
                    "succeeded": true,
                    "runtime_run_id": "run-1",
                    "metrics": { "latency_p99_ms": 2.5 }
                },
                {
                    "candidate_id": "candidate-2",
                    "status": "Completed",
                    "succeeded": true,
                    "runtime_run_id": "run-2",
                    "metrics": { "latency_p99_ms": 1.2 }
                }
            ]
        }),
    }]);

    let output = voidctl_command(&base_url)
        .args(["execution", "result", "exec-result-1"])
        .output()
        .expect("result output");
    server.join().expect("join fake bridge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("winner_candidate_id=candidate-2 runtime_run_id=run-2"));
    assert!(stdout.contains("candidate_id=candidate-1 status=Completed succeeded=true"));
    assert!(stdout.contains("\"latency_p99_ms\":1.2"));
}

#[test]
fn runtime_uses_best_candidate_when_not_explicitly_requested() {
    let (base_url, _requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 200,
        body: json!({
            "execution": {
                "execution_id": "exec-runtime-1",
                "status": "Completed",
                "result_best_candidate_id": "candidate-2"
            },
            "candidates": [
                {
                    "candidate_id": "candidate-1",
                    "status": "Completed",
                    "runtime_run_id": "run-1"
                },
                {
                    "candidate_id": "candidate-2",
                    "status": "Completed",
                    "runtime_run_id": "run-2"
                }
            ]
        }),
    }]);

    let output = voidctl_command(&base_url)
        .args(["execution", "runtime", "exec-runtime-1"])
        .output()
        .expect("runtime output");
    server.join().expect("join fake bridge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("execution_id=exec-runtime-1"));
    assert!(stdout.contains("candidate_id=candidate-2"));
    assert!(stdout.contains("runtime_run_id=run-2"));
}

#[test]
fn template_list_prints_available_templates() {
    let (base_url, requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 200,
        body: json!({
            "templates": [
                {
                    "id": "single-agent-basic",
                    "name": "Single Agent",
                    "execution_kind": "single_agent",
                    "description": "Run one agent once and return the result."
                },
                {
                    "id": "warm-agent-basic",
                    "name": "Warm Agent",
                    "execution_kind": "warm_agent",
                    "description": "Start one long-running service-mode agent."
                }
            ]
        }),
    }]);

    let output = voidctl_command(&base_url)
        .args(["template", "list"])
        .output()
        .expect("template list output");
    server.join().expect("join fake bridge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("template_id=single-agent-basic"));
    assert!(stdout.contains("execution_kind=single_agent"));
    assert!(stdout.contains("template_id=warm-agent-basic"));

    let requests = requests.lock().expect("lock requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/v1/templates");
}

#[test]
fn template_get_prints_template_details() {
    let (base_url, requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 200,
        body: json!({
            "template": {
                "id": "single-agent-basic",
                "name": "Single Agent",
                "execution_kind": "single_agent",
                "description": "Run one agent once and return the result."
            },
            "inputs": {
                "goal": { "type": "string", "required": true, "description": "Goal" },
                "prompt": { "type": "string", "required": true, "description": "Prompt" }
            },
            "defaults": {
                "workflow_template": "examples/runtime-templates/claude_mcp_diagnostic_agent.yaml"
            }
        }),
    }]);

    let output = voidctl_command(&base_url)
        .args(["template", "get", "single-agent-basic"])
        .output()
        .expect("template get output");
    server.join().expect("join fake bridge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("template_id=single-agent-basic"));
    assert!(stdout.contains("execution_kind=single_agent"));
    assert!(stdout
        .contains("workflow_template=examples/runtime-templates/claude_mcp_diagnostic_agent.yaml"));
    assert!(stdout.contains("input=goal"));
    assert!(stdout.contains("input=prompt"));

    let requests = requests.lock().expect("lock requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/v1/templates/single-agent-basic");
}

#[test]
fn template_dry_run_from_stdin_posts_inputs_and_prints_compiled_summary() {
    let (base_url, requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 200,
        body: json!({
            "template": {
                "id": "single-agent-basic",
                "execution_kind": "single_agent"
            },
            "inputs": {
                "goal": "Summarize this repo",
                "prompt": "Read the repo and summarize risks",
                "provider": "claude"
            },
            "compiled": {
                "goal": "Summarize this repo",
                "workflow_template": "examples/runtime-templates/claude_mcp_diagnostic_agent.yaml",
                "mode": "swarm",
                "variation_source": "explicit",
                "candidates_per_iteration": 1,
                "overrides": {
                    "agent.prompt": "Read the repo and summarize risks",
                    "llm.provider": "claude"
                }
            }
        }),
    }]);

    let mut child = voidctl_command(&base_url)
        .args(["template", "dry-run", "single-agent-basic", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn voidctl");
    let inputs = r#"{"inputs":{"goal":"Summarize this repo","prompt":"Read the repo and summarize risks","provider":"claude"}}"#;
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(inputs.as_bytes())
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait output");
    server.join().expect("join fake bridge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("template_id=single-agent-basic"));
    assert!(stdout.contains("execution_kind=single_agent"));
    assert!(stdout
        .contains("workflow_template=examples/runtime-templates/claude_mcp_diagnostic_agent.yaml"));
    assert!(stdout.contains("agent.prompt=Read the repo and summarize risks"));
    assert!(stdout.contains("llm.provider=claude"));

    let requests = requests.lock().expect("lock requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/templates/single-agent-basic/dry-run");
    assert_eq!(requests[0].body, inputs);
}

#[test]
fn template_execute_from_stdin_posts_inputs_and_prints_execution_summary() {
    let (base_url, requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 200,
        body: json!({
            "execution_id": "exec-template-1",
            "template": {
                "id": "warm-agent-basic",
                "execution_kind": "warm_agent"
            },
            "status": "Pending",
            "goal": "Keep a warm agent ready"
        }),
    }]);

    let mut child = voidctl_command(&base_url)
        .args(["template", "execute", "warm-agent-basic", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn voidctl");
    let inputs = r#"{"inputs":{"goal":"Keep a warm agent ready","prompt":"Stay alive for follow-up repo work."}}"#;
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(inputs.as_bytes())
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait output");
    server.join().expect("join fake bridge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("execution_id=exec-template-1"));
    assert!(stdout.contains("template_id=warm-agent-basic"));
    assert!(stdout.contains("execution_kind=warm_agent"));
    assert!(stdout.contains("status=Pending"));
    assert!(stdout.contains("goal=Keep a warm agent ready"));

    let requests = requests.lock().expect("lock requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/templates/warm-agent-basic/execute");
    assert_eq!(requests[0].body, inputs);
}

#[test]
fn template_get_failure_returns_non_zero_and_prints_bridge_message() {
    let (base_url, _requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 404,
        body: json!({
            "message": "template 'missing-template' not found"
        }),
    }]);

    let output = voidctl_command(&base_url)
        .args(["template", "get", "missing-template"])
        .output()
        .expect("template get output");
    server.join().expect("join fake bridge");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("fatal: template 'missing-template' not found"));
}

#[test]
fn template_dry_run_failure_returns_non_zero_and_prints_bridge_message() {
    let (base_url, _requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 400,
        body: json!({
            "message": "missing required input 'prompt'"
        }),
    }]);

    let mut child = voidctl_command(&base_url)
        .args(["template", "dry-run", "single-agent-basic", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn voidctl");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(br#"{"inputs":{"goal":"Summarize this repo"}}"#)
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait output");
    server.join().expect("join fake bridge");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("fatal: missing required input 'prompt'"));
}

#[test]
fn template_execute_failure_returns_non_zero_and_prints_bridge_message() {
    let (base_url, _requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 400,
        body: json!({
            "message": "invalid template request body: expected value"
        }),
    }]);

    let mut child = voidctl_command(&base_url)
        .args(["template", "execute", "warm-agent-basic", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn voidctl");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"not-json")
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait output");
    server.join().expect("join fake bridge");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("fatal: invalid template request body: expected value"));
}

#[test]
fn interactive_template_get_prints_error_for_bridge_failure() {
    let (base_url, _requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 404,
        body: json!({
            "message": "template 'missing-template' not found"
        }),
    }]);

    let mut child = voidctl_command(&base_url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn voidctl");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"/template get missing-template\n/exit\n")
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait output");
    server.join().expect("join fake bridge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("error: template 'missing-template' not found"));
}

#[test]
fn interactive_batch_dry_run_posts_spec_and_prints_compiled_summary() {
    let (base_url, requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 200,
        body: json!({
            "kind": "batch",
            "compiled_primitive": "swarm",
            "compiled": {
                "goal": "repo-background-work",
                "workflow_template": "examples/runtime-templates/warm_agent_basic.yaml",
                "mode": "swarm",
                "variation_source": "explicit",
                "candidates_per_iteration": 2,
                "candidate_overrides": [
                    { "agent.prompt": "Fix failing auth tests" },
                    { "agent.prompt": "Improve retry logging" }
                ],
                "overrides": {
                    "agent.prompt": "Fix failing auth tests"
                }
            }
        }),
    }]);

    let inputs_path = temp_inputs_path("batch.json");
    fs::write(
        &inputs_path,
        r#"{"api_version":"v1","kind":"batch","worker":{"template":"examples/runtime-templates/warm_agent_basic.yaml"},"mode":{"parallelism":2},"jobs":[{"prompt":"Fix failing auth tests"},{"prompt":"Improve retry logging"}]}"#,
    )
    .expect("write inputs");

    let mut child = voidctl_command(&base_url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn voidctl");
    let command = format!("/batch dry-run {}\n/exit\n", inputs_path.display());
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(command.as_bytes())
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait output");
    server.join().expect("join fake bridge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("kind=batch"));
    assert!(stdout.contains("compiled_primitive=swarm"));
    assert!(stdout.contains("workflow_template=examples/runtime-templates/warm_agent_basic.yaml"));

    let requests = requests.lock().expect("lock requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/batch/dry-run");
}

#[test]
fn interactive_yolo_run_alias_posts_to_yolo_route() {
    let (base_url, requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 200,
        body: json!({
            "kind": "batch",
            "run_id": "exec-yolo-2",
            "execution_id": "exec-yolo-2",
            "compiled_primitive": "swarm",
            "status": "Pending",
            "goal": "run 1 background jobs"
        }),
    }]);

    let inputs_path = temp_inputs_path("yolo.json");
    fs::write(
        &inputs_path,
        r#"{"api_version":"v1","kind":"yolo","worker":{"template":"examples/runtime-templates/warm_agent_basic.yaml"},"jobs":[{"prompt":"Review migration safety"}]}"#,
    )
    .expect("write inputs");

    let mut child = voidctl_command(&base_url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn voidctl");
    let command = format!("/yolo run {}\n/exit\n", inputs_path.display());
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(command.as_bytes())
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait output");
    server.join().expect("join fake bridge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("run_id=exec-yolo-2"));
    assert!(stdout.contains("compiled_primitive=swarm"));

    let requests = requests.lock().expect("lock requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/yolo/run");
}

#[test]
fn batch_dry_run_from_stdin_posts_spec_and_prints_compiled_summary() {
    let (base_url, requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 200,
        body: json!({
            "kind": "batch",
            "compiled_primitive": "swarm",
            "compiled": {
                "goal": "repo-background-work",
                "workflow_template": "examples/runtime-templates/warm_agent_basic.yaml",
                "mode": "swarm",
                "variation_source": "explicit",
                "candidates_per_iteration": 2,
                "candidate_overrides": [
                    { "agent.prompt": "Fix failing auth tests" },
                    { "agent.prompt": "Improve retry logging" },
                    { "agent.prompt": "Review DB migration safety" }
                ],
                "overrides": {
                    "agent.prompt": "Fix failing auth tests"
                }
            }
        }),
    }]);

    let mut child = voidctl_command(&base_url)
        .args(["batch", "dry-run", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn voidctl");
    let spec = r#"{"api_version":"v1","kind":"batch","worker":{"template":"examples/runtime-templates/warm_agent_basic.yaml"},"mode":{"parallelism":2},"jobs":[{"prompt":"Fix failing auth tests"},{"prompt":"Improve retry logging"},{"prompt":"Review DB migration safety"}]}"#;
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(spec.as_bytes())
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait output");
    server.join().expect("join fake bridge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("kind=batch"));
    assert!(stdout.contains("compiled_primitive=swarm"));
    assert!(stdout.contains("workflow_template=examples/runtime-templates/warm_agent_basic.yaml"));

    let requests = requests.lock().expect("lock requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/batch/dry-run");
    assert_eq!(requests[0].body, spec);
}

#[test]
fn batch_run_from_stdin_posts_spec_and_prints_run_summary() {
    let (base_url, requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 200,
        body: json!({
            "kind": "batch",
            "run_id": "exec-batch-1",
            "execution_id": "exec-batch-1",
            "compiled_primitive": "swarm",
            "status": "Pending",
            "goal": "repo-background-work"
        }),
    }]);

    let mut child = voidctl_command(&base_url)
        .args(["batch", "run", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn voidctl");
    let spec = r#"{"api_version":"v1","kind":"batch","worker":{"template":"examples/runtime-templates/warm_agent_basic.yaml"},"jobs":[{"prompt":"Fix failing auth tests"},{"prompt":"Improve retry logging"}]}"#;
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(spec.as_bytes())
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait output");
    server.join().expect("join fake bridge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("kind=batch"));
    assert!(stdout.contains("run_id=exec-batch-1"));
    assert!(stdout.contains("compiled_primitive=swarm"));
    assert!(stdout.contains("status=Pending"));

    let requests = requests.lock().expect("lock requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1/batch/run");
    assert_eq!(requests[0].body, spec);
}

#[test]
fn yolo_run_alias_posts_to_yolo_route() {
    let (base_url, requests, server) = spawn_fake_bridge(vec![FakeResponse {
        status: 200,
        body: json!({
            "kind": "batch",
            "run_id": "exec-yolo-1",
            "execution_id": "exec-yolo-1",
            "compiled_primitive": "swarm",
            "status": "Pending",
            "goal": "run 1 background jobs"
        }),
    }]);

    let mut child = voidctl_command(&base_url)
        .args(["yolo", "run", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn voidctl");
    let spec = r#"{"api_version":"v1","kind":"yolo","worker":{"template":"examples/runtime-templates/warm_agent_basic.yaml"},"jobs":[{"prompt":"Review migration safety"}]}"#;
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(spec.as_bytes())
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait output");
    server.join().expect("join fake bridge");

    assert!(output.status.success());
    let requests = requests.lock().expect("lock requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/v1/yolo/run");
}
