use std::collections::BTreeMap;

use void_control::orchestration::{
    CandidateOutput, ExecutionService, ExecutionSpec, ExecutionStatus, FsExecutionStore,
    GlobalConfig, OrchestrationPolicy, VariationConfig, VariationProposal,
};
use void_control::runtime::MockRuntime;

#[test]
fn missing_output_can_mark_failed() {
    let mut runtime = MockRuntime::new();
    runtime.seed_missing_output("exec-run-candidate-1");
    runtime.seed_success(
        "exec-run-candidate-2",
        output(
            "candidate-2",
            &[("latency_p99_ms", 80.0), ("cost_usd", 0.03)],
        ),
    );

    let store = FsExecutionStore::new(temp_store_dir("missing-failed"));
    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store,
    );
    let execution = service
        .run_to_completion(spec_with_missing_output_policy("mark_failed"))
        .expect("run execution");

    assert_eq!(execution.status, ExecutionStatus::Failed);
    assert_eq!(execution.failure_counts.total_candidate_failures, 1);
}

#[test]
fn missing_output_can_mark_incomplete_without_failure_count() {
    let mut runtime = MockRuntime::new();
    runtime.seed_missing_output("exec-run-candidate-1");
    runtime.seed_success(
        "exec-run-candidate-2",
        output(
            "candidate-2",
            &[("latency_p99_ms", 80.0), ("cost_usd", 0.03)],
        ),
    );

    let store = FsExecutionStore::new(temp_store_dir("missing-incomplete"));
    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store,
    );
    let execution = service
        .run_to_completion(spec_with_continue_missing_output())
        .expect("run execution");

    assert_eq!(execution.status, ExecutionStatus::Completed);
    assert_eq!(execution.failure_counts.total_candidate_failures, 0);
    assert_eq!(
        execution.result_best_candidate_id.as_deref(),
        Some("candidate-2")
    );
}

#[test]
fn iteration_failure_policy_continue_advances_despite_all_failures() {
    let mut runtime = MockRuntime::new();
    runtime.seed_failure("exec-run-candidate-1");
    runtime.seed_failure("exec-run-candidate-2");
    runtime.seed_success(
        "exec-run-candidate-3",
        output(
            "candidate-3",
            &[("latency_p99_ms", 75.0), ("cost_usd", 0.02)],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-4",
        output(
            "candidate-4",
            &[("latency_p99_ms", 78.0), ("cost_usd", 0.02)],
        ),
    );

    let store = FsExecutionStore::new(temp_store_dir("continue"));
    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store,
    );
    let execution = service
        .run_to_completion(spec_with_iteration_failure_policy("continue", 2))
        .expect("run execution");

    assert_eq!(execution.status, ExecutionStatus::Completed);
    assert_eq!(execution.completed_iterations, 2);
}

#[test]
fn iteration_failure_policy_retry_retries_once() {
    let mut runtime = MockRuntime::new();
    runtime.seed_failure("exec-run-candidate-1");
    runtime.seed_failure("exec-run-candidate-2");
    runtime.seed_success(
        "exec-run-candidate-3",
        output(
            "candidate-3",
            &[("latency_p99_ms", 74.0), ("cost_usd", 0.02)],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-4",
        output(
            "candidate-4",
            &[("latency_p99_ms", 76.0), ("cost_usd", 0.02)],
        ),
    );

    let store = FsExecutionStore::new(temp_store_dir("retry"));
    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store,
    );
    let execution = service
        .run_to_completion(spec_with_iteration_failure_policy("retry_iteration", 1))
        .expect("run execution");

    assert_eq!(execution.status, ExecutionStatus::Completed);
    assert_eq!(execution.completed_iterations, 1);
}

#[test]
fn malformed_output_is_counted_as_candidate_failure() {
    let mut runtime = MockRuntime::new();
    runtime.seed_malformed_output("exec-run-candidate-1");
    runtime.seed_success(
        "exec-run-candidate-2",
        output(
            "candidate-2",
            &[("latency_p99_ms", 80.0), ("cost_usd", 0.03)],
        ),
    );

    let store = FsExecutionStore::new(temp_store_dir("malformed-output"));
    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store,
    );
    let execution = service
        .run_to_completion(spec_with_missing_output_policy("mark_failed"))
        .expect("run execution");

    assert_eq!(execution.status, ExecutionStatus::Failed);
    assert_eq!(execution.failure_counts.total_candidate_failures, 1);
}

fn spec_with_missing_output_policy(policy_name: &str) -> ExecutionSpec {
    let mut spec = base_spec(1);
    spec.policy.max_candidate_failures_per_iteration = 1;
    spec.policy.missing_output_policy = policy_name.to_string();
    spec
}

fn spec_with_continue_missing_output() -> ExecutionSpec {
    let mut spec = base_spec(1);
    spec.policy.missing_output_policy = "mark_incomplete".to_string();
    spec.policy.max_candidate_failures_per_iteration = 10;
    spec
}

fn spec_with_iteration_failure_policy(policy_name: &str, max_iterations: u32) -> ExecutionSpec {
    let mut spec = base_spec(max_iterations);
    spec.policy.iteration_failure_policy = policy_name.to_string();
    spec.policy.max_candidate_failures_per_iteration = 10;
    spec
}

fn base_spec(max_iterations: u32) -> ExecutionSpec {
    ExecutionSpec {
        mode: "swarm".to_string(),
        goal: "optimize latency".to_string(),
        workflow: void_control::orchestration::WorkflowTemplateRef {
            template: "fixtures/sample.vbrun".to_string(),
        },
        policy: OrchestrationPolicy {
            budget: void_control::orchestration::BudgetPolicy {
                max_iterations: Some(max_iterations),
                max_child_runs: None,
                max_wall_clock_secs: Some(60),
                max_cost_usd_millis: None,
            },
            concurrency: void_control::orchestration::ConcurrencyPolicy {
                max_concurrent_candidates: 2,
            },
            convergence: void_control::orchestration::ConvergencePolicy {
                strategy: "exhaustive".to_string(),
                min_score: None,
                max_iterations_without_improvement: None,
            },
            max_candidate_failures_per_iteration: 10,
            missing_output_policy: "mark_failed".to_string(),
            iteration_failure_policy: "fail_execution".to_string(),
        },
        evaluation: void_control::orchestration::EvaluationConfig {
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
            2,
            vec![
                VariationProposal {
                    overrides: BTreeMap::from([("agent.prompt".to_string(), "a".to_string())]),
                },
                VariationProposal {
                    overrides: BTreeMap::from([("agent.prompt".to_string(), "b".to_string())]),
                },
            ],
        ),
        swarm: true,
        supervision: None,
    }
}

fn output(candidate_id: &str, metrics: &[(&str, f64)]) -> CandidateOutput {
    CandidateOutput::new(
        candidate_id.to_string(),
        true,
        metrics.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
    )
}

fn temp_store_dir(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("void-control-artifacts-{label}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
