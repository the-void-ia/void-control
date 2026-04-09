#![cfg(feature = "serde")]

use serde_json::json;

#[test]
fn dry_run_endpoint_returns_plan_without_creating_execution() {
    let body = json!({
        "mode": "swarm",
        "goal": "optimize latency",
        "workflow": { "template": "fixtures/sample.vbrun" },
        "policy": {
            "budget": {
                "max_iterations": 3,
                "max_wall_clock_secs": 60
            },
            "concurrency": {
                "max_concurrent_candidates": 2
            },
            "convergence": {
                "strategy": "exhaustive"
            },
            "max_candidate_failures_per_iteration": 10,
            "missing_output_policy": "mark_failed",
            "iteration_failure_policy": "fail_execution"
        },
        "evaluation": {
            "scoring_type": "weighted_metrics",
            "weights": {
                "latency_p99_ms": -0.6,
                "cost_usd": -0.4
            },
            "pass_threshold": 0.7,
            "ranking": "highest_score",
            "tie_breaking": "cost_usd"
        },
        "variation": {
            "source": "explicit",
            "candidates_per_iteration": 2,
            "explicit": [
                { "overrides": { "agent.prompt": "a" } },
                { "overrides": { "agent.prompt": "b" } }
            ]
        },
        "swarm": true
    })
    .to_string();

    let response = void_control::bridge::handle_bridge_request_for_test(
        "POST",
        "/v1/executions/dry-run",
        Some(&body),
    )
    .expect("response");

    assert_eq!(response.status, 200);
    assert_eq!(response.json["valid"], true);
    assert_eq!(response.json["plan"]["max_child_runs"], 6);
}

#[test]
fn dry_run_endpoint_returns_validation_errors() {
    let body = json!({
        "mode": "swarm",
        "goal": "optimize latency",
        "workflow": { "template": "fixtures/sample.vbrun" },
        "policy": {
            "budget": {},
            "concurrency": {
                "max_concurrent_candidates": 2
            },
            "convergence": {
                "strategy": "threshold"
            },
            "max_candidate_failures_per_iteration": 10,
            "missing_output_policy": "mark_failed",
            "iteration_failure_policy": "fail_execution"
        },
        "evaluation": {
            "scoring_type": "weighted_metrics",
            "weights": {
                "latency_p99_ms": -0.6,
                "cost_usd": -0.4
            },
            "pass_threshold": 0.7,
            "ranking": "highest_score",
            "tie_breaking": "cost_usd"
        },
        "variation": {
            "source": "explicit",
            "candidates_per_iteration": 2,
            "explicit": [
                { "overrides": { "agent.prompt": "a" } }
            ]
        },
        "swarm": true
    })
    .to_string();

    let response = void_control::bridge::handle_bridge_request_for_test(
        "POST",
        "/v1/executions/dry-run",
        Some(&body),
    )
    .expect("response");

    assert_eq!(response.status, 400);
    assert!(response.json["errors"].as_array().is_some());
}

#[test]
fn create_list_and_get_execution_routes_round_trip() {
    let root = temp_root("create-round-trip");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    let body = valid_spec_body();

    let created = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions",
        Some(&body),
        &spec_dir,
        &execution_dir,
    )
    .expect("create");

    assert_eq!(created.status, 200);
    assert_eq!(created.json["status"], "Pending");
    let execution_id = created.json["execution_id"]
        .as_str()
        .expect("execution_id")
        .to_string();

    let listed = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "GET",
        "/v1/executions",
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("list");
    assert_eq!(listed.status, 200);
    assert_eq!(
        listed.json["executions"]
            .as_array()
            .map(|items| items.len()),
        Some(1)
    );

    let fetched = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "GET",
        &format!("/v1/executions/{execution_id}"),
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("get");
    assert_eq!(fetched.status, 200);
    assert_eq!(fetched.json["execution"]["execution_id"], execution_id);
    assert_eq!(fetched.json["progress"]["event_count"], 2);
    assert_eq!(
        fetched.json["progress"]["event_type_counts"]["ExecutionCreated"],
        1
    );
    assert_eq!(
        fetched.json["progress"]["event_type_counts"]["ExecutionSubmitted"],
        1
    );
    assert_eq!(fetched.json["progress"]["candidate_queue_count"], 0);
    assert_eq!(
        fetched.json["result"]["best_candidate_id"],
        serde_json::Value::Null
    );
    assert_eq!(fetched.json["result"]["completed_iterations"], 0);
    assert_eq!(fetched.json["result"]["total_candidate_failures"], 0);
}

#[test]
fn create_execution_route_accepts_search_specs() {
    let root = temp_root("create-search");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    let body = valid_spec_body_for_mode("search");

    let created = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions",
        Some(&body),
        &spec_dir,
        &execution_dir,
    )
    .expect("create");

    assert_eq!(created.status, 200);
    assert_eq!(created.json["status"], "Pending");
    assert_eq!(created.json["mode"], "search");
}

#[test]
fn create_execution_route_accepts_yaml_specs() {
    let root = temp_root("create-yaml");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    let body = r#"
mode: swarm
goal: optimize transform latency
workflow:
  template: examples/runtime-templates/transform_optimizer_agent.yaml
policy:
  budget:
    max_iterations: 2
    max_wall_clock_secs: 600
  concurrency:
    max_concurrent_candidates: 4
  convergence:
    strategy: exhaustive
  max_candidate_failures_per_iteration: 10
  missing_output_policy: mark_incomplete
  iteration_failure_policy: continue
evaluation:
  scoring_type: weighted_metrics
  weights:
    latency_p99_ms: -0.55
    error_rate: -0.30
    cpu_pct: -0.15
  pass_threshold: 0.7
  ranking: highest_score
  tie_breaking: latency_p99_ms
variation:
  source: explicit
  candidates_per_iteration: 2
  explicit:
    - overrides:
        sandbox.env.TRANSFORM_STRATEGY: baseline
    - overrides:
        sandbox.env.TRANSFORM_STRATEGY: batch-fusion
swarm: true
"#;

    let created = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions",
        Some(body),
        &spec_dir,
        &execution_dir,
    )
    .expect("create");

    assert_eq!(created.status, 200);
    assert_eq!(created.json["status"], "Pending");
    assert_eq!(created.json["mode"], "swarm");
    assert_eq!(created.json["goal"], "optimize transform latency");
}

#[test]
fn dry_run_route_wraps_runtime_yaml_specs() {
    let root = temp_root("dry-run-runtime-yaml");
    let spec_dir = root.join("specs");
    let body = runtime_spec_body();

    let response = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions/dry-run",
        Some(body),
        &spec_dir,
        &root.join("executions"),
    )
    .expect("dry-run");

    assert_eq!(response.status, 200);
    assert_eq!(response.json["valid"], true);
    assert_eq!(response.json["plan"]["candidates_per_iteration"], 1);
    assert_eq!(response.json["plan"]["max_iterations"], 1);
    assert_eq!(response.json["plan"]["max_child_runs"], 1);
}

#[test]
fn create_execution_route_wraps_runtime_yaml_specs() {
    let root = temp_root("create-runtime-yaml");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    let body = runtime_spec_body();

    let created = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions",
        Some(body),
        &spec_dir,
        &execution_dir,
    )
    .expect("create");

    assert_eq!(created.status, 200);
    assert_eq!(created.json["status"], "Pending");
    assert_eq!(created.json["mode"], "swarm");
    assert_eq!(created.json["goal"], "run snapshot-pipeline");

    let execution_id = created.json["execution_id"].as_str().expect("execution_id");
    let store = void_control::orchestration::FsExecutionStore::new(execution_dir);
    let spec = store.load_spec(execution_id).expect("load wrapped spec");
    assert_eq!(spec.mode, "swarm");
    assert_eq!(spec.variation.candidates_per_iteration, 1);
    assert!(std::path::Path::new(&spec.workflow.template).exists());
    assert!(spec
        .workflow
        .template
        .starts_with(spec_dir.to_string_lossy().as_ref()));
}

#[test]
fn get_execution_events_route_returns_persisted_event_stream() {
    let root = temp_root("execution-events");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    let body = valid_spec_body();

    let created = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions",
        Some(&body),
        &spec_dir,
        &execution_dir,
    )
    .expect("create");
    let execution_id = created.json["execution_id"]
        .as_str()
        .expect("execution_id")
        .to_string();

    let events = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "GET",
        &format!("/v1/executions/{execution_id}/events"),
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("events");

    assert_eq!(events.status, 200);
    let items = events.json["events"].as_array().expect("events array");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["event_type"], "ExecutionCreated");
    assert_eq!(items[1]["event_type"], "ExecutionSubmitted");
}

#[test]
fn get_execution_route_reports_current_candidate_status_counts() {
    let root = temp_root("execution-progress-counts");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    let body = valid_spec_body();

    let created = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions",
        Some(&body),
        &spec_dir,
        &execution_dir,
    )
    .expect("create");
    let execution_id = created.json["execution_id"]
        .as_str()
        .expect("execution_id")
        .to_string();

    let store = void_control::orchestration::FsExecutionStore::new(execution_dir.clone());
    let mut planner = void_control::orchestration::ExecutionService::new(
        void_control::orchestration::GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        void_control::runtime::MockRuntime::new(),
        store,
    );
    planner.plan_execution(&execution_id).expect("plan");

    let fetched = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "GET",
        &format!("/v1/executions/{execution_id}"),
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("get");

    assert_eq!(fetched.status, 200);
    assert_eq!(fetched.json["progress"]["queued_candidate_count"], 2);
    assert_eq!(fetched.json["progress"]["running_candidate_count"], 0);
    assert_eq!(fetched.json["progress"]["completed_candidate_count"], 0);
    assert_eq!(fetched.json["progress"]["failed_candidate_count"], 0);
    assert_eq!(fetched.json["progress"]["canceled_candidate_count"], 0);
    let candidates = fetched.json["candidates"]
        .as_array()
        .expect("candidates array");
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0]["candidate_id"], "candidate-1");
    assert_eq!(candidates[0]["iteration"], 0);
    assert_eq!(candidates[0]["status"], "Queued");
    assert!(candidates[0]["metrics"].is_object());
}

#[test]
fn get_execution_route_returns_not_found_for_missing_execution() {
    let root = temp_root("missing-execution");
    let response = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "GET",
        "/v1/executions/does-not-exist",
        None,
        &root.join("specs"),
        &root.join("executions"),
    )
    .expect("response");

    assert_eq!(response.status, 404);
    assert_eq!(response.json["code"], "NOT_FOUND");
}

#[test]
fn pause_resume_and_cancel_execution_routes_update_persisted_status() {
    let root = temp_root("status-transitions");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    seed_execution(&execution_dir, "exec-running", "Running");

    let paused = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions/exec-running/pause",
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("pause");
    assert_eq!(paused.status, 200);
    assert_eq!(paused.json["status"], "Paused");

    let resumed = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions/exec-running/resume",
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("resume");
    assert_eq!(resumed.status, 200);
    assert_eq!(resumed.json["status"], "Running");

    let canceled = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions/exec-running/cancel",
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("cancel");
    assert_eq!(canceled.status, 200);
    assert_eq!(canceled.json["status"], "Canceled");
}

#[test]
fn pause_route_rejects_invalid_transition() {
    let root = temp_root("invalid-transition");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    seed_execution(&execution_dir, "exec-complete", "Completed");

    let response = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions/exec-complete/pause",
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("pause");

    assert_eq!(response.status, 400);
    assert_eq!(response.json["code"], "INVALID_STATE");
}

#[test]
fn patch_policy_updates_mutable_budget_and_concurrency_fields() {
    let root = temp_root("policy-patch");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    let body = valid_spec_body();

    let created = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions",
        Some(&body),
        &spec_dir,
        &execution_dir,
    )
    .expect("create");
    let execution_id = created.json["execution_id"].as_str().expect("execution_id");

    let patch = serde_json::json!({
        "budget": {
            "max_iterations": 5
        },
        "concurrency": {
            "max_concurrent_candidates": 4
        }
    })
    .to_string();

    let patched = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "PATCH",
        &format!("/v1/executions/{execution_id}/policy"),
        Some(&patch),
        &spec_dir,
        &execution_dir,
    )
    .expect("patch");

    assert_eq!(patched.status, 200);
    assert_eq!(patched.json["max_iterations"], 5);
    assert_eq!(patched.json["max_concurrent_candidates"], 4);
}

#[test]
fn patch_policy_rejects_immutable_convergence_fields() {
    let root = temp_root("policy-immutable");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    let body = valid_spec_body();

    let created = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions",
        Some(&body),
        &spec_dir,
        &execution_dir,
    )
    .expect("create");
    let execution_id = created.json["execution_id"].as_str().expect("execution_id");

    let patch = serde_json::json!({
        "convergence": {
            "strategy": "threshold",
            "min_score": 0.9
        }
    })
    .to_string();

    let patched = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "PATCH",
        &format!("/v1/executions/{execution_id}/policy"),
        Some(&patch),
        &spec_dir,
        &execution_dir,
    )
    .expect("patch");

    assert_eq!(patched.status, 400);
    assert_eq!(patched.json["code"], "INVALID_POLICY");
}

fn valid_spec_body() -> String {
    valid_spec_body_for_mode("swarm")
}

fn runtime_spec_body() -> &'static str {
    r#"
api_version: v1
kind: pipeline
name: snapshot-pipeline
sandbox:
  mode: auto
llm:
  provider: claude
stages:
  - id: analyzer
    agent:
      prompt: summarize the snapshot
"#
}

fn valid_spec_body_for_mode(mode: &str) -> String {
    json!({
        "mode": mode,
        "goal": "optimize latency",
        "workflow": { "template": "fixtures/sample.vbrun" },
        "policy": {
            "budget": {
                "max_iterations": 1,
                "max_wall_clock_secs": 60
            },
            "concurrency": {
                "max_concurrent_candidates": 2
            },
            "convergence": {
                "strategy": "exhaustive"
            },
            "max_candidate_failures_per_iteration": 10,
            "missing_output_policy": "mark_incomplete",
            "iteration_failure_policy": "continue"
        },
        "evaluation": {
            "scoring_type": "weighted_metrics",
            "weights": {
                "latency_p99_ms": -0.6,
                "cost_usd": -0.4
            },
            "pass_threshold": 0.7,
            "ranking": "highest_score",
            "tie_breaking": "cost_usd"
        },
        "variation": {
            "source": "explicit",
            "candidates_per_iteration": 2,
            "explicit": [
                { "overrides": { "agent.prompt": "a" } },
                { "overrides": { "agent.prompt": "b" } }
            ]
        },
        "swarm": true
    })
    .to_string()
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("void-control-bridge-{label}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn seed_execution(root: &std::path::Path, execution_id: &str, status: &str) {
    let dir = root.join(execution_id);
    std::fs::create_dir_all(&dir).expect("execution dir");
    std::fs::write(
        dir.join("execution.txt"),
        format!("{execution_id}\nswarm\ngoal\n{status}"),
    )
    .expect("write execution");
}
