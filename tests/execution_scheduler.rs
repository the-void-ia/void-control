use std::collections::BTreeMap;

use void_control::orchestration::{
    CandidateOutput, ExecutionAccumulator, ExecutionService, ExecutionSpec, ExecutionStatus,
    FsExecutionStore, GlobalConfig, OrchestrationPolicy, QueuedCandidate, SchedulerDecision,
    StructuredOutputResult,
    VariationConfig, VariationProposal,
};
use void_control::runtime::MockRuntime;

#[test]
fn mock_runtime_can_complete_runs_with_structured_outputs() {
    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "run-1",
        output("cand-a", &[("latency_p99_ms", 100.0), ("cost_usd", 0.02)]),
    );

    let started = runtime.start(test_start_request("run-1")).expect("start");
    let inspection = runtime.inspect(&started.handle).expect("inspect");
    let output = runtime
        .take_structured_output("run-1");
    let StructuredOutputResult::Found(output) = output else {
        panic!("expected structured output")
    };

    assert_eq!(inspection.state, void_control::contract::RunState::Succeeded);
    assert_eq!(output.metrics["latency_p99_ms"], 100.0);
}

#[test]
fn mock_runtime_can_simulate_failure_timeout_and_missing_output() {
    let mut runtime = MockRuntime::new();
    runtime.seed_failure("run-fail");
    runtime.seed_missing_output("run-missing");

    let fail = runtime.start(test_start_request("run-fail")).expect("start fail");
    let missing = runtime
        .start(test_start_request("run-missing"))
        .expect("start missing");

    assert_eq!(
        runtime.inspect(&fail.handle).expect("inspect fail").state,
        void_control::contract::RunState::Failed
    );
    assert_eq!(
        runtime.inspect(&missing.handle).expect("inspect missing").state,
        void_control::contract::RunState::Succeeded
    );
    assert!(matches!(
        runtime.take_structured_output("run-missing"),
        StructuredOutputResult::Missing
    ));
}

#[test]
fn preserves_plan_candidates_order_within_execution() {
    let mut scheduler = void_control::orchestration::GlobalScheduler::new(2);
    scheduler.enqueue(QueuedCandidate::new("exec-1", "cand-1", 1));
    scheduler.enqueue(QueuedCandidate::new("exec-1", "cand-2", 2));

    let first = scheduler.next_dispatch().expect("first dispatch");
    scheduler.mark_running(&first);
    scheduler.release(&first.execution_id, &first.candidate_id);
    let second = scheduler.next_dispatch().expect("second dispatch");

    assert_eq!(first.candidate_id, "cand-1");
    assert_eq!(second.candidate_id, "cand-2");
}

#[test]
fn dispatches_across_executions_fifo_by_candidate_creation_time() {
    let mut scheduler = void_control::orchestration::GlobalScheduler::new(1);
    scheduler.enqueue(QueuedCandidate::new("exec-1", "cand-late", 2));
    scheduler.enqueue(QueuedCandidate::new("exec-2", "cand-early", 1));

    let grant = scheduler.next_dispatch().expect("dispatch");

    assert_eq!(grant.execution_id, "exec-2");
    assert_eq!(grant.candidate_id, "cand-early");
}

#[test]
fn releases_slots_immediately_on_completion() {
    let mut scheduler = void_control::orchestration::GlobalScheduler::new(1);
    scheduler.enqueue(QueuedCandidate::new("exec-1", "cand-1", 1));
    scheduler.enqueue(QueuedCandidate::new("exec-2", "cand-2", 2));

    let first = scheduler.next_dispatch().expect("first dispatch");
    scheduler.mark_running(&first);
    assert!(scheduler.next_dispatch().is_none());
    scheduler.release(&first.execution_id, &first.candidate_id);

    let second = scheduler.next_dispatch().expect("second dispatch");
    assert_eq!(second.candidate_id, "cand-2");
}

#[test]
fn paused_execution_keeps_queue_but_releases_slots() {
    let mut scheduler = void_control::orchestration::GlobalScheduler::new(2);
    scheduler.enqueue(QueuedCandidate::new("exec-1", "cand-1", 1));
    scheduler.enqueue(QueuedCandidate::new("exec-1", "cand-2", 2));

    let grant = scheduler.next_dispatch().expect("dispatch");
    scheduler.mark_running(&grant);
    scheduler.pause_execution("exec-1");

    assert_eq!(scheduler.execution_queue_depth("exec-1"), 1);
    assert_eq!(scheduler.active_slots(), 0);
    assert!(scheduler.next_dispatch().is_none());
}

#[test]
fn per_execution_concurrency_cap_blocks_dispatch_until_release() {
    let mut scheduler = void_control::orchestration::GlobalScheduler::new(4);
    scheduler.register_execution("exec-1", false, 1, 1);
    scheduler.enqueue(QueuedCandidate::new("exec-1", "cand-1", 1));
    scheduler.enqueue(QueuedCandidate::new("exec-2", "cand-2", 2));

    let grant = scheduler.next_dispatch().expect("dispatch");
    assert_eq!(grant.execution_id, "exec-2");
    assert_eq!(grant.candidate_id, "cand-2");

    scheduler.release("exec-1", "running");
    let second = scheduler.next_dispatch().expect("second dispatch");
    assert_eq!(second.execution_id, "exec-1");
    assert_eq!(second.candidate_id, "cand-1");
}

#[test]
fn exhausted_budget_prevents_queue_entry() {
    let mut scheduler = void_control::orchestration::GlobalScheduler::new(1);
    let mut accumulator = ExecutionAccumulator::default();
    accumulator.completed_iterations = 1;

    let decision = scheduler.enqueue_if_budget_allows(
        QueuedCandidate::new("exec-1", "cand-1", 1),
        &accumulator,
        1,
    );

    assert_eq!(decision, SchedulerDecision::RejectedBudgetExceeded);
}

#[test]
fn runs_single_iteration_and_completes_with_best_result() {
    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        output("candidate-1", &[("latency_p99_ms", 120.0), ("cost_usd", 0.04)]),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        output("candidate-2", &[("latency_p99_ms", 90.0), ("cost_usd", 0.03)]),
    );

    let store = FsExecutionStore::new(temp_store_dir("single"));
    let mut service = ExecutionService::new(GlobalConfig { max_concurrent_child_runs: 2 }, runtime, store);
    let execution = service.run_to_completion(test_spec(1)).expect("run execution");

    assert_eq!(execution.status, ExecutionStatus::Completed);
    assert_eq!(execution.result_best_candidate_id.as_deref(), Some("candidate-2"));
}

#[test]
fn runs_multiple_iterations_until_threshold() {
    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        output("candidate-1", &[("latency_p99_ms", 100.0), ("cost_usd", 0.02)]),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        output("candidate-2", &[("latency_p99_ms", 95.0), ("cost_usd", 0.20)]),
    );
    runtime.seed_success(
        "exec-run-candidate-3",
        output("candidate-3", &[("latency_p99_ms", 70.0), ("cost_usd", 0.02)]),
    );
    runtime.seed_success(
        "exec-run-candidate-4",
        output("candidate-4", &[("latency_p99_ms", 72.0), ("cost_usd", 0.02)]),
    );

    let store = FsExecutionStore::new(temp_store_dir("threshold"));
    let mut service = ExecutionService::new(GlobalConfig { max_concurrent_child_runs: 2 }, runtime, store);
    let execution = service
        .run_to_completion(test_spec_with_threshold(0.9, 2))
        .expect("run execution");

    assert_eq!(execution.status, ExecutionStatus::Completed);
    assert_eq!(execution.completed_iterations, 2);
}

#[test]
fn short_circuits_iteration_after_failure_limit() {
    let mut runtime = MockRuntime::new();
    runtime.seed_failure("exec-run-candidate-1");
    runtime.seed_success(
        "exec-run-candidate-2",
        output("candidate-2", &[("latency_p99_ms", 95.0), ("cost_usd", 0.03)]),
    );

    let store = FsExecutionStore::new(temp_store_dir("fail-limit"));
    let mut service = ExecutionService::new(GlobalConfig { max_concurrent_child_runs: 2 }, runtime, store);
    let execution = service
        .run_to_completion(test_spec_with_failure_limit(1))
        .expect("run execution");

    assert_eq!(execution.status, ExecutionStatus::Failed);
    assert_eq!(execution.failure_counts.total_candidate_failures, 1);
}

#[test]
fn marks_execution_failed_when_all_candidates_fail_and_policy_says_fail() {
    let mut runtime = MockRuntime::new();
    runtime.seed_failure("exec-run-candidate-1");
    runtime.seed_failure("exec-run-candidate-2");

    let store = FsExecutionStore::new(temp_store_dir("all-fail"));
    let mut service = ExecutionService::new(GlobalConfig { max_concurrent_child_runs: 2 }, runtime, store);
    let execution = service
        .run_to_completion(test_spec_with_failure_limit(2))
        .expect("run execution");

    assert_eq!(execution.status, ExecutionStatus::Failed);
}

fn test_start_request(run_id: &str) -> void_control::contract::StartRequest {
    void_control::contract::StartRequest {
        run_id: run_id.to_string(),
        workflow_spec: "workflow".to_string(),
        launch_context: None,
        policy: void_control::contract::ExecutionPolicy {
            max_parallel_microvms_per_run: 1,
            max_stage_retries: 1,
            stage_timeout_secs: 60,
            cancel_grace_period_secs: 5,
        },
    }
}

fn test_spec(max_iterations: u32) -> ExecutionSpec {
    test_spec_inner(max_iterations, None, 10)
}

fn test_spec_with_threshold(min_score: f64, max_iterations: u32) -> ExecutionSpec {
    let mut spec = test_spec_inner(max_iterations, Some(min_score), 10);
    spec.policy.convergence.strategy = "threshold".to_string();
    spec.policy.convergence.min_score = Some(min_score);
    spec
}

fn test_spec_with_failure_limit(limit: u32) -> ExecutionSpec {
    let mut spec = test_spec_inner(1, None, limit);
    spec.policy.max_candidate_failures_per_iteration = limit;
    spec
}

fn test_spec_inner(
    max_iterations: u32,
    min_score: Option<f64>,
    max_candidate_failures_per_iteration: u32,
) -> ExecutionSpec {
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
                strategy: if min_score.is_some() {
                    "threshold".to_string()
                } else {
                    "exhaustive".to_string()
                },
                min_score,
                max_iterations_without_improvement: None,
            },
            max_candidate_failures_per_iteration,
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
    let dir = std::env::temp_dir().join(format!("void-control-scheduler-{label}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
