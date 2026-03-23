#![cfg(feature = "serde")]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use void_control::orchestration::{
    BudgetPolicy, ConcurrencyPolicy, ConvergencePolicy, EvaluationConfig, ExecutionSpec,
    GlobalConfig, OrchestrationPolicy, VariationConfig, VariationProposal, WorkflowTemplateRef,
};
use void_control::runtime::VoidBoxRuntimeClient;

#[test]
#[ignore = "requires live void-box daemon"]
fn bridge_submission_and_worker_loop_complete_execution_against_live_daemon() {
    let root = temp_root("bridge-live");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    let spec = structured_output_spec();
    let body = execution_request_json(&spec);

    let created = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions",
        Some(&body),
        &spec_dir,
        &execution_dir,
    )
    .expect("create execution");
    assert_eq!(created.status, 200);

    let execution_id = created.json["execution_id"]
        .as_str()
        .expect("execution_id")
        .to_string();

    let base_url =
        std::env::var("VOID_BOX_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:43100".to_string());

    let mut attempts = 0;
    loop {
        attempts += 1;
        void_control::bridge::process_pending_executions_once_for_test(
            GlobalConfig {
                max_concurrent_child_runs: 20,
            },
            VoidBoxRuntimeClient::new(base_url.clone(), 250),
            execution_dir.clone(),
        )
        .expect("process pending");

        let fetched = void_control::bridge::handle_bridge_request_with_dirs_for_test(
            "GET",
            &format!("/v1/executions/{execution_id}"),
            None,
            &spec_dir,
            &execution_dir,
        )
        .expect("get execution");

        let status = fetched.json["execution"]["status"]
            .as_str()
            .expect("status");
        if matches!(status, "Completed" | "Failed" | "Canceled") {
            assert_eq!(status, "Completed", "execution payload={}", fetched.json);
            assert!(
                fetched.json["progress"]["event_count"]
                    .as_u64()
                    .unwrap_or_default()
                    >= 6
            );

            let events = void_control::bridge::handle_bridge_request_with_dirs_for_test(
                "GET",
                &format!("/v1/executions/{execution_id}/events"),
                None,
                &spec_dir,
                &execution_dir,
            )
            .expect("get execution events");
            assert_eq!(events.status, 200);
            let items = events.json["events"].as_array().expect("events array");
            assert!(items
                .iter()
                .any(|event| event["event_type"] == "ExecutionStarted"));
            assert!(items
                .iter()
                .any(|event| event["event_type"] == "CandidateOutputCollected"));
            assert!(items
                .iter()
                .any(|event| event["event_type"] == "ExecutionCompleted"));
            break;
        }

        assert!(attempts < 20, "execution did not reach terminal state");
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

#[test]
#[ignore = "requires live void-box daemon"]
fn bridge_multiple_executions_complete_against_live_daemon() {
    let root = temp_root("bridge-live-multi");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    let spec = structured_output_spec();
    let body = execution_request_json(&spec);
    let base_url =
        std::env::var("VOID_BOX_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:43100".to_string());

    let first = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions",
        Some(&body),
        &spec_dir,
        &execution_dir,
    )
    .expect("create first execution");
    let second = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions",
        Some(&body),
        &spec_dir,
        &execution_dir,
    )
    .expect("create second execution");

    let first_id = first.json["execution_id"]
        .as_str()
        .expect("first execution_id")
        .to_string();
    let second_id = second.json["execution_id"]
        .as_str()
        .expect("second execution_id")
        .to_string();

    let mut first_done = false;
    let mut second_done = false;
    for _ in 0..20 {
        void_control::bridge::process_pending_executions_once_for_test(
            GlobalConfig {
                max_concurrent_child_runs: 20,
            },
            VoidBoxRuntimeClient::new(base_url.clone(), 250),
            execution_dir.clone(),
        )
        .expect("process pending");

        for (execution_id, done) in [(&first_id, &mut first_done), (&second_id, &mut second_done)] {
            if *done {
                continue;
            }
            let fetched = void_control::bridge::handle_bridge_request_with_dirs_for_test(
                "GET",
                &format!("/v1/executions/{execution_id}"),
                None,
                &spec_dir,
                &execution_dir,
            )
            .expect("get execution");
            let status = fetched.json["execution"]["status"]
                .as_str()
                .expect("status");
            if status == "Completed" {
                *done = true;
            } else {
                assert_eq!(status, "Running", "execution payload={}", fetched.json);
            }
        }

        if first_done && second_done {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
    }

    assert!(first_done, "first execution did not complete");
    assert!(second_done, "second execution did not complete");
}

#[test]
#[ignore = "requires live void-box daemon"]
fn bridge_pause_resume_and_cancel_work_against_live_daemon() {
    let root = temp_root("bridge-live-control");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    let spec = long_running_spec();
    let body = execution_request_json(&spec);
    let base_url =
        std::env::var("VOID_BOX_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:43100".to_string());

    let created = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/executions",
        Some(&body),
        &spec_dir,
        &execution_dir,
    )
    .expect("create execution");
    let execution_id = created.json["execution_id"]
        .as_str()
        .expect("execution_id")
        .to_string();

    let pause_execution_dir = execution_dir.clone();
    let pause_spec_dir = spec_dir.clone();
    let pause_execution_id = execution_id.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let _ = void_control::bridge::handle_bridge_request_with_dirs_for_test(
            "POST",
            &format!("/v1/executions/{pause_execution_id}/pause"),
            None,
            &pause_spec_dir,
            &pause_execution_dir,
        );
    });

    void_control::bridge::process_pending_executions_once_for_test(
        GlobalConfig {
            max_concurrent_child_runs: 20,
        },
        VoidBoxRuntimeClient::new(base_url.clone(), 250),
        execution_dir.clone(),
    )
    .expect("pause processing pass");

    let paused = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "GET",
        &format!("/v1/executions/{execution_id}"),
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("get paused execution");
    assert_eq!(paused.json["execution"]["status"], "Paused");

    let resumed = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        &format!("/v1/executions/{execution_id}/resume"),
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("resume");
    assert_eq!(resumed.json["status"], "Running");

    let cancel_execution_dir = execution_dir.clone();
    let cancel_spec_dir = spec_dir.clone();
    let cancel_execution_id = execution_id.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let _ = void_control::bridge::handle_bridge_request_with_dirs_for_test(
            "POST",
            &format!("/v1/executions/{cancel_execution_id}/cancel"),
            None,
            &cancel_spec_dir,
            &cancel_execution_dir,
        );
    });

    let mut canceled = None;
    for _ in 0..10 {
        void_control::bridge::process_pending_executions_once_for_test(
            GlobalConfig {
                max_concurrent_child_runs: 20,
            },
            VoidBoxRuntimeClient::new(base_url.clone(), 250),
            execution_dir.clone(),
        )
        .expect("cancel processing pass");

        let fetched = void_control::bridge::handle_bridge_request_with_dirs_for_test(
            "GET",
            &format!("/v1/executions/{execution_id}"),
            None,
            &spec_dir,
            &execution_dir,
        )
        .expect("get canceled execution");
        if fetched.json["execution"]["status"] == "Canceled" {
            canceled = Some(fetched);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let canceled = canceled.expect("execution should reach canceled state");
    assert_eq!(canceled.json["execution"]["status"], "Canceled");

    let events = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "GET",
        &format!("/v1/executions/{execution_id}/events"),
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("get events");
    let items = events.json["events"].as_array().expect("events array");
    assert!(items
        .iter()
        .any(|event| event["event_type"] == "ExecutionPaused"));
    assert!(items
        .iter()
        .any(|event| event["event_type"] == "ExecutionResumed"));
    assert!(items
        .iter()
        .any(|event| event["event_type"] == "ExecutionCanceled"));
}

fn structured_output_spec() -> ExecutionSpec {
    let path = fallback_structured_output_spec_path();
    fs::write(
        &path,
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
"#,
    )
    .expect("write fixture");

    ExecutionSpec {
        mode: "swarm".to_string(),
        goal: "optimize latency".to_string(),
        workflow: WorkflowTemplateRef {
            template: path.to_string_lossy().to_string(),
        },
        policy: OrchestrationPolicy {
            budget: BudgetPolicy {
                max_iterations: Some(1),
                max_child_runs: None,
                max_wall_clock_secs: Some(60),
                max_cost_usd_millis: None,
            },
            concurrency: ConcurrencyPolicy {
                max_concurrent_candidates: 1,
            },
            convergence: ConvergencePolicy {
                strategy: "exhaustive".to_string(),
                min_score: None,
                max_iterations_without_improvement: None,
            },
            max_candidate_failures_per_iteration: 1,
            missing_output_policy: "mark_failed".to_string(),
            iteration_failure_policy: "fail_execution".to_string(),
        },
        evaluation: EvaluationConfig {
            scoring_type: "weighted_metrics".to_string(),
            weights: BTreeMap::from([
                ("latency_p99_ms".to_string(), -0.6),
                ("cost_usd".to_string(), -0.4),
            ]),
            pass_threshold: Some(0.7),
            ranking: "highest_score".to_string(),
            tie_breaking: "cost_usd".to_string(),
        },
        variation: VariationConfig::explicit(
            1,
            vec![VariationProposal {
                overrides: BTreeMap::from([("agent.prompt".to_string(), "a".to_string())]),
            }],
        ),
        swarm: true,
    }
}

fn long_running_spec() -> ExecutionSpec {
    let path = fallback_long_running_spec_path();
    fs::write(
        &path,
        r#"api_version: v1
kind: workflow
name: long-running

sandbox:
  mode: local
  network: false

workflow:
  steps:
    - name: wait
      run:
        program: sleep
        args: ["5"]
    - name: produce
      depends_on: [wait]
      run:
        program: sh
        args:
          - -lc
          - |
            cat > result.json <<'JSON'
            {"status":"success","summary":"ok","metrics":{"duration":5},"artifacts":[]}
            JSON
  output_step: produce
"#,
    )
    .expect("write fixture");

    ExecutionSpec {
        mode: "swarm".to_string(),
        goal: "exercise pause cancel".to_string(),
        workflow: WorkflowTemplateRef {
            template: path.to_string_lossy().to_string(),
        },
        policy: OrchestrationPolicy {
            budget: BudgetPolicy {
                max_iterations: Some(1),
                max_child_runs: None,
                max_wall_clock_secs: Some(60),
                max_cost_usd_millis: None,
            },
            concurrency: ConcurrencyPolicy {
                max_concurrent_candidates: 1,
            },
            convergence: ConvergencePolicy {
                strategy: "exhaustive".to_string(),
                min_score: None,
                max_iterations_without_improvement: None,
            },
            max_candidate_failures_per_iteration: 1,
            missing_output_policy: "mark_failed".to_string(),
            iteration_failure_policy: "fail_execution".to_string(),
        },
        evaluation: EvaluationConfig {
            scoring_type: "weighted_metrics".to_string(),
            weights: BTreeMap::from([("duration".to_string(), -1.0)]),
            pass_threshold: Some(0.0),
            ranking: "highest_score".to_string(),
            tie_breaking: "duration".to_string(),
        },
        variation: VariationConfig::explicit(
            1,
            vec![VariationProposal {
                overrides: BTreeMap::from([("agent.prompt".to_string(), "a".to_string())]),
            }],
        ),
        swarm: true,
    }
}

fn execution_request_json(spec: &ExecutionSpec) -> String {
    serde_json::to_string(&json!({
        "mode": spec.mode,
        "goal": spec.goal,
        "workflow": { "template": spec.workflow.template },
        "policy": {
            "budget": {
                "max_iterations": spec.policy.budget.max_iterations,
                "max_wall_clock_secs": spec.policy.budget.max_wall_clock_secs
            },
            "concurrency": {
                "max_concurrent_candidates": spec.policy.concurrency.max_concurrent_candidates
            },
            "convergence": {
                "strategy": spec.policy.convergence.strategy
            },
            "max_candidate_failures_per_iteration": spec.policy.max_candidate_failures_per_iteration,
            "missing_output_policy": spec.policy.missing_output_policy,
            "iteration_failure_policy": spec.policy.iteration_failure_policy
        },
        "evaluation": {
            "scoring_type": spec.evaluation.scoring_type,
            "weights": spec.evaluation.weights,
            "pass_threshold": spec.evaluation.pass_threshold,
            "ranking": spec.evaluation.ranking,
            "tie_breaking": spec.evaluation.tie_breaking
        },
        "variation": {
            "source": "explicit",
            "candidates_per_iteration": spec.variation.candidates_per_iteration,
            "explicit": spec.variation.explicit.iter().map(|proposal| json!({"overrides": proposal.overrides})).collect::<Vec<_>>()
        },
        "swarm": spec.swarm
    }))
    .expect("serialize spec")
}

fn fallback_structured_output_spec_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "void-control-bridge-live-structured-output-{nanos}.yaml"
    ))
}

fn fallback_long_running_spec_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "void-control-bridge-live-long-running-{nanos}.yaml"
    ))
}

fn temp_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("void-control-bridge-{label}-{nanos}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
