#![cfg(feature = "serde")]

use std::cell::RefCell;
use std::collections::BTreeMap;

use void_control::contract::{
    ContractError, ContractErrorCode, RunState, RuntimeInspection, StartRequest, StartResult,
};
use void_control::orchestration::{
    CandidateOutput, CandidateStatus, ExecutionCandidate, ExecutionService, ExecutionSpec,
    ExecutionStatus, FsExecutionStore, GlobalConfig, OrchestrationPolicy, VariationConfig,
    VariationProposal,
};
use void_control::runtime::MockRuntime;

#[test]
fn submitted_pending_execution_can_be_processed_to_completion() {
    let root = temp_store_dir("worker");
    let store = FsExecutionStore::new(root);
    let spec = spec(1);
    let execution = ExecutionService::<MockRuntime>::submit_execution(&store, "exec-worker", &spec)
        .expect("submit");
    assert_eq!(execution.status, ExecutionStatus::Pending);

    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        output(
            "candidate-1",
            &[("latency_p99_ms", 90.0), ("cost_usd", 0.03)],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        output(
            "candidate-2",
            &[("latency_p99_ms", 85.0), ("cost_usd", 0.02)],
        ),
    );

    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store.clone(),
    );
    let processed = service.process_execution("exec-worker").expect("process");

    assert_eq!(processed.status, ExecutionStatus::Completed);
    assert_eq!(
        processed.result_best_candidate_id.as_deref(),
        Some("candidate-2")
    );

    let snapshot = store.load_execution("exec-worker").expect("reload");
    assert_eq!(snapshot.execution.status, ExecutionStatus::Completed);
}

#[test]
fn bridge_worker_helper_processes_pending_executions() {
    let root = temp_store_dir("bridge-worker");
    let store = FsExecutionStore::new(root.clone());
    let spec = spec(1);
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-bridge-worker", &spec)
        .expect("submit");

    tick_bridge_worker_until_terminal(root.clone(), "exec-bridge-worker");

    let snapshot = store.load_execution("exec-bridge-worker").expect("reload");
    assert_eq!(snapshot.execution.status, ExecutionStatus::Completed);
}

#[test]
fn bridge_worker_resumes_running_candidate_even_without_queued_candidates() {
    let root = temp_store_dir("bridge-worker-running-only");
    let store = FsExecutionStore::new(root.clone());
    let spec = single_candidate_spec();
    ExecutionService::<StepwiseRuntime>::submit_execution(
        &store,
        "exec-bridge-running-only",
        &spec,
    )
    .expect("submit");

    void_control::bridge::process_pending_executions_once_for_test(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        StepwiseRuntime::with_outputs(
            1,
            [(
                "exec-run-candidate-1",
                output(
                    "candidate-1",
                    &[("latency_p99_ms", 90.0), ("cost_usd", 0.03)],
                ),
            )],
        ),
        root.clone(),
    )
    .expect("first bridge tick");

    let first = store
        .load_execution("exec-bridge-running-only")
        .expect("reload");
    assert_eq!(first.candidates.len(), 1);
    assert_eq!(first.candidates[0].status, CandidateStatus::Running);

    void_control::bridge::process_pending_executions_once_for_test(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        StepwiseRuntime::with_outputs(
            0,
            [(
                "exec-run-candidate-1",
                output(
                    "candidate-1",
                    &[("latency_p99_ms", 90.0), ("cost_usd", 0.03)],
                ),
            )],
        ),
        root,
    )
    .expect("second bridge tick");

    let second = store
        .load_execution("exec-bridge-running-only")
        .expect("reload");
    assert_eq!(second.candidates[0].status, CandidateStatus::Completed);
    assert_eq!(second.execution.status, ExecutionStatus::Completed);
}

#[test]
fn bridge_worker_collects_output_from_running_service_candidate() {
    let root = temp_store_dir("bridge-worker-running-output-ready");
    let store = FsExecutionStore::new(root.clone());
    let spec = single_candidate_spec();
    ExecutionService::<RunningOutputReadyRuntime>::submit_execution(
        &store,
        "exec-bridge-running-output-ready",
        &spec,
    )
    .expect("submit");

    void_control::bridge::process_pending_executions_once_for_test(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        RunningOutputReadyRuntime::new(output(
            "candidate-1",
            &[("latency_p99_ms", 90.0), ("cost_usd", 0.03)],
        )),
        root.clone(),
    )
    .expect("first bridge tick");

    let first = store
        .load_execution("exec-bridge-running-output-ready")
        .expect("reload");
    assert_eq!(first.candidates[0].status, CandidateStatus::Completed);
    assert_eq!(first.execution.status, ExecutionStatus::Completed);
}

#[test]
fn bridge_worker_keeps_running_service_candidate_when_output_not_ready_yet() {
    let root = temp_store_dir("bridge-worker-running-output-missing");
    let store = FsExecutionStore::new(root.clone());
    let spec = single_candidate_spec();
    ExecutionService::<RunningOutputMissingRuntime>::submit_execution(
        &store,
        "exec-bridge-running-output-missing",
        &spec,
    )
    .expect("submit");

    void_control::bridge::process_pending_executions_once_for_test(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        RunningOutputMissingRuntime,
        root,
    )
    .expect("bridge tick");

    let snapshot = store
        .load_execution("exec-bridge-running-output-missing")
        .expect("reload");
    assert_eq!(snapshot.execution.status, ExecutionStatus::Running);
    assert_eq!(snapshot.candidates.len(), 1);
    assert_eq!(snapshot.candidates[0].status, CandidateStatus::Running);
    assert_eq!(
        snapshot
            .events
            .iter()
            .filter(|event| {
                event.event_type
                    == void_control::orchestration::ControlEventType::CandidateOutputCollected
            })
            .count(),
        0
    );
}

#[test]
fn bridge_worker_keeps_running_service_candidate_when_output_path_is_not_found_yet() {
    let root = temp_store_dir("bridge-worker-running-output-not-found");
    let store = FsExecutionStore::new(root.clone());
    let spec = single_candidate_spec();
    ExecutionService::<RunningOutputNotFoundRuntime>::submit_execution(
        &store,
        "exec-bridge-running-output-not-found",
        &spec,
    )
    .expect("submit");

    void_control::bridge::process_pending_executions_once_for_test(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        RunningOutputNotFoundRuntime,
        root,
    )
    .expect("bridge tick");

    let snapshot = store
        .load_execution("exec-bridge-running-output-not-found")
        .expect("reload");
    assert_eq!(snapshot.execution.status, ExecutionStatus::Running);
    assert_eq!(snapshot.candidates.len(), 1);
    assert_eq!(snapshot.candidates[0].status, CandidateStatus::Running);
}

#[test]
fn bridge_worker_dispatches_multiple_candidates_up_to_execution_concurrency() {
    let root = temp_store_dir("bridge-worker-parallel");
    let store = FsExecutionStore::new(root.clone());
    let spec = spec_with_candidate_count(1, 4, 4);
    ExecutionService::<StepwiseRuntime>::submit_execution(&store, "exec-bridge-parallel", &spec)
        .expect("submit");

    void_control::bridge::process_pending_executions_once_for_test(
        GlobalConfig {
            max_concurrent_child_runs: 4,
        },
        StepwiseRuntime::new(1),
        root,
    )
    .expect("bridge tick");

    let snapshot = store
        .load_execution("exec-bridge-parallel")
        .expect("reload");
    assert_eq!(
        snapshot
            .candidates
            .iter()
            .filter(|candidate| candidate.status == CandidateStatus::Running)
            .count(),
        4
    );
    assert_eq!(
        snapshot
            .events
            .iter()
            .filter(|event| {
                event.event_type
                    == void_control::orchestration::ControlEventType::CandidateDispatched
            })
            .count(),
        4
    );
}

#[test]
fn planning_execution_persists_queued_candidates_without_dispatching() {
    let root = temp_store_dir("worker-plan-only");
    let store = FsExecutionStore::new(root);
    let spec = spec(1);
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-plan-only", &spec)
        .expect("submit");

    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        MockRuntime::new(),
        store.clone(),
    );
    let execution = service.plan_execution("exec-plan-only").expect("plan");

    assert_eq!(execution.status, ExecutionStatus::Running);
    let snapshot = store.load_execution("exec-plan-only").expect("reload");
    assert_eq!(snapshot.execution.status, ExecutionStatus::Running);
    assert_eq!(snapshot.candidates.len(), 2);
    assert_eq!(snapshot.candidates[0].status, CandidateStatus::Queued);
    assert_eq!(snapshot.candidates[1].status, CandidateStatus::Queued);
    assert_eq!(snapshot.candidates[0].runtime_run_id, None);
    assert_eq!(snapshot.candidates[1].runtime_run_id, None);
    let event_types: Vec<_> = snapshot
        .events
        .iter()
        .map(|event| event.event_type)
        .collect();
    assert!(event_types.contains(&void_control::orchestration::ControlEventType::ExecutionStarted));
    assert!(event_types.contains(&void_control::orchestration::ControlEventType::IterationStarted));
    assert_eq!(
        event_types
            .iter()
            .filter(
                |&&event| event == void_control::orchestration::ControlEventType::CandidateQueued
            )
            .count(),
        2
    );
}

#[test]
fn processing_reuses_preplanned_candidates_without_duplication() {
    let root = temp_store_dir("worker-plan-then-process");
    let store = FsExecutionStore::new(root);
    let spec = spec(1);
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-plan-then-process", &spec)
        .expect("submit");

    let mut planner = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        MockRuntime::new(),
        store.clone(),
    );
    planner
        .plan_execution("exec-plan-then-process")
        .expect("plan");

    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        output(
            "candidate-1",
            &[("latency_p99_ms", 90.0), ("cost_usd", 0.03)],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        output(
            "candidate-2",
            &[("latency_p99_ms", 85.0), ("cost_usd", 0.02)],
        ),
    );
    let mut worker = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store.clone(),
    );
    let execution = worker
        .process_execution("exec-plan-then-process")
        .expect("process");

    assert_eq!(execution.status, ExecutionStatus::Completed);
    let snapshot = store
        .load_execution("exec-plan-then-process")
        .expect("reload");
    assert_eq!(snapshot.candidates.len(), 2);
    assert_eq!(snapshot.candidates[0].created_seq, 1);
    assert_eq!(snapshot.candidates[1].created_seq, 2);
    assert_eq!(snapshot.candidates[0].status, CandidateStatus::Completed);
    assert_eq!(snapshot.candidates[1].status, CandidateStatus::Completed);
    assert_eq!(
        snapshot
            .events
            .iter()
            .filter(|event| {
                event.event_type == void_control::orchestration::ControlEventType::CandidateQueued
            })
            .count(),
        2
    );
}

#[test]
fn dispatch_execution_once_runs_only_one_queued_candidate() {
    let root = temp_store_dir("worker-dispatch-once");
    let store = FsExecutionStore::new(root);
    let spec = spec(1);
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-dispatch-once", &spec)
        .expect("submit");

    let mut planner = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        MockRuntime::new(),
        store.clone(),
    );
    planner.plan_execution("exec-dispatch-once").expect("plan");

    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        output(
            "candidate-1",
            &[("latency_p99_ms", 90.0), ("cost_usd", 0.03)],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        output(
            "candidate-2",
            &[("latency_p99_ms", 85.0), ("cost_usd", 0.02)],
        ),
    );
    let mut worker = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store.clone(),
    );
    let execution = worker
        .dispatch_execution_once("exec-dispatch-once")
        .expect("dispatch once");

    assert_eq!(execution.status, ExecutionStatus::Running);
    let snapshot = store.load_execution("exec-dispatch-once").expect("reload");
    assert_eq!(snapshot.execution.status, ExecutionStatus::Running);
    assert_eq!(snapshot.candidates.len(), 2);
    assert_eq!(snapshot.candidates[0].status, CandidateStatus::Completed);
    assert_eq!(snapshot.candidates[1].status, CandidateStatus::Queued);
    assert_eq!(
        snapshot
            .events
            .iter()
            .filter(|event| {
                event.event_type
                    == void_control::orchestration::ControlEventType::CandidateDispatched
            })
            .count(),
        1
    );
    assert_eq!(
        snapshot
            .events
            .iter()
            .filter(|event| {
                event.event_type
                    == void_control::orchestration::ControlEventType::CandidateOutputCollected
            })
            .count(),
        1
    );
}

#[test]
fn dispatch_execution_once_persists_running_candidate_for_nonterminal_run() {
    let root = temp_store_dir("worker-dispatch-running");
    let store = FsExecutionStore::new(root);
    let spec = spec(1);
    ExecutionService::<StepwiseRuntime>::submit_execution(&store, "exec-dispatch-running", &spec)
        .expect("submit");

    let mut planner = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        StepwiseRuntime::new(1),
        store.clone(),
    );
    planner
        .plan_execution("exec-dispatch-running")
        .expect("plan");

    let output = output(
        "candidate-1",
        &[("latency_p99_ms", 90.0), ("cost_usd", 0.03)],
    );
    let runtime = StepwiseRuntime::with_outputs(1, [("exec-run-candidate-1", output)]);
    let mut worker = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store.clone(),
    );
    let execution = worker
        .dispatch_execution_once("exec-dispatch-running")
        .expect("dispatch once");

    assert_eq!(execution.status, ExecutionStatus::Running);
    let snapshot = store
        .load_execution("exec-dispatch-running")
        .expect("reload");
    assert_eq!(snapshot.execution.status, ExecutionStatus::Running);
    assert_eq!(snapshot.candidates[0].status, CandidateStatus::Running);
    assert_eq!(
        snapshot.candidates[0].runtime_run_id.as_deref(),
        Some("step:exec-run-candidate-1")
    );
    assert_eq!(snapshot.candidates[1].status, CandidateStatus::Queued);
    assert_eq!(
        snapshot
            .events
            .iter()
            .filter(|event| {
                event.event_type
                    == void_control::orchestration::ControlEventType::CandidateDispatched
            })
            .count(),
        1
    );
    assert_eq!(
        snapshot
            .events
            .iter()
            .filter(|event| {
                event.event_type
                    == void_control::orchestration::ControlEventType::CandidateOutputCollected
            })
            .count(),
        0
    );
}

#[test]
fn dispatch_execution_once_reconciles_persisted_running_candidate_on_later_tick() {
    let root = temp_store_dir("worker-dispatch-reconcile");
    let store = FsExecutionStore::new(root);
    let spec = spec(1);
    ExecutionService::<StepwiseRuntime>::submit_execution(&store, "exec-dispatch-reconcile", &spec)
        .expect("submit");

    let mut planner = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        StepwiseRuntime::new(1),
        store.clone(),
    );
    planner
        .plan_execution("exec-dispatch-reconcile")
        .expect("plan");

    let output = output(
        "candidate-1",
        &[("latency_p99_ms", 90.0), ("cost_usd", 0.03)],
    );
    let runtime = StepwiseRuntime::with_outputs(1, [("exec-run-candidate-1", output)]);
    let mut worker = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store.clone(),
    );
    worker
        .dispatch_execution_once("exec-dispatch-reconcile")
        .expect("first dispatch");
    let second = worker
        .dispatch_execution_once("exec-dispatch-reconcile")
        .expect("second dispatch");

    assert_eq!(second.status, ExecutionStatus::Running);
    let snapshot = store
        .load_execution("exec-dispatch-reconcile")
        .expect("reload");
    assert_eq!(snapshot.candidates[0].status, CandidateStatus::Completed);
    assert_eq!(snapshot.candidates[1].status, CandidateStatus::Queued);
    assert_eq!(snapshot.candidates[0].succeeded, Some(true));
    assert_eq!(
        snapshot
            .events
            .iter()
            .filter(|event| {
                event.event_type
                    == void_control::orchestration::ControlEventType::CandidateDispatched
            })
            .count(),
        1
    );
    assert_eq!(
        snapshot
            .events
            .iter()
            .filter(|event| {
                event.event_type
                    == void_control::orchestration::ControlEventType::CandidateOutputCollected
            })
            .count(),
        1
    );
}

#[test]
fn dispatch_execution_once_prefers_structured_output_over_failed_terminal_state() {
    let root = temp_store_dir("worker-dispatch-terminal-failed-with-output");
    let store = FsExecutionStore::new(root);
    let spec = spec(1);
    ExecutionService::<TerminalFailedRuntime>::submit_execution(
        &store,
        "exec-dispatch-terminal-failed-with-output",
        &spec,
    )
    .expect("submit");

    let mut planner = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        TerminalFailedRuntime::new(None),
        store.clone(),
    );
    planner
        .plan_execution("exec-dispatch-terminal-failed-with-output")
        .expect("plan");

    let output = output(
        "candidate-1",
        &[
            ("latency_p99_ms", 90.0),
            ("error_rate", 0.01),
            ("cpu_pct", 44.0),
        ],
    );
    let mut worker = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        TerminalFailedRuntime::new(Some(output)),
        store.clone(),
    );
    let execution = worker
        .dispatch_execution_once("exec-dispatch-terminal-failed-with-output")
        .expect("dispatch once");

    assert_eq!(execution.status, ExecutionStatus::Running);
    let snapshot = store
        .load_execution("exec-dispatch-terminal-failed-with-output")
        .expect("reload");
    assert_eq!(snapshot.candidates[0].status, CandidateStatus::Completed);
    assert_eq!(snapshot.candidates[0].succeeded, Some(true));
    assert_eq!(
        snapshot.candidates[0].metrics.get("latency_p99_ms"),
        Some(&90.0)
    );
    assert_eq!(
        snapshot
            .events
            .iter()
            .filter(|event| {
                event.event_type
                    == void_control::orchestration::ControlEventType::CandidateOutputCollected
            })
            .count(),
        1
    );
}

#[test]
fn process_execution_skips_already_claimed_execution() {
    let root = temp_store_dir("worker-claimed");
    let store = FsExecutionStore::new(root);
    let spec = spec(1);
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-claimed", &spec)
        .expect("submit");
    assert!(store
        .claim_execution("exec-claimed", "other-worker")
        .expect("claim"));

    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        MockRuntime::new(),
        store.clone(),
    );
    let err = service
        .process_execution("exec-claimed")
        .expect_err("claimed execution should not process");
    assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);

    let snapshot = store.load_execution("exec-claimed").expect("reload");
    assert_eq!(snapshot.execution.status, ExecutionStatus::Pending);
    assert_eq!(
        store.load_claim("exec-claimed").expect("claim").as_deref(),
        Some("other-worker")
    );
}

#[test]
fn stale_claim_is_recovered_and_processing_can_proceed() {
    let root = temp_store_dir("worker-stale-claim");
    let store = FsExecutionStore::new(root.clone());
    let spec = spec(1);
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-stale-claim", &spec)
        .expect("submit");

    let execution_dir = root.join("exec-stale-claim");
    std::fs::write(execution_dir.join("claim.txt"), "dead-worker|1").expect("seed stale claim");

    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        output(
            "candidate-1",
            &[("latency_p99_ms", 90.0), ("cost_usd", 0.03)],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        output(
            "candidate-2",
            &[("latency_p99_ms", 85.0), ("cost_usd", 0.02)],
        ),
    );

    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store.clone(),
    );
    let processed = service
        .process_execution("exec-stale-claim")
        .expect("process");
    assert_eq!(processed.status, ExecutionStatus::Completed);
    assert_eq!(store.load_claim("exec-stale-claim").expect("claim"), None);
}

#[test]
fn refresh_claim_keeps_owned_claim_valid() {
    let root = temp_store_dir("worker-refresh-claim");
    let store = FsExecutionStore::new(root.clone());
    let spec = spec(1);
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-refresh-claim", &spec)
        .expect("submit");

    assert!(store
        .claim_execution("exec-refresh-claim", "worker-a")
        .expect("claim"));
    store
        .refresh_claim("exec-refresh-claim", "worker-a")
        .expect("refresh");
    assert_eq!(
        store
            .load_claim("exec-refresh-claim")
            .expect("load claim")
            .as_deref(),
        Some("worker-a")
    );
    store
        .release_claim("exec-refresh-claim")
        .expect("release claim");
}

#[test]
fn candidate_records_round_trip_through_store() {
    let root = temp_store_dir("worker-candidates");
    let store = FsExecutionStore::new(root);
    let spec = spec(1);
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-candidates", &spec)
        .expect("submit");

    let queued = ExecutionCandidate::new(
        "exec-candidates",
        "candidate-1",
        1,
        0,
        CandidateStatus::Queued,
    );
    let mut queued = queued;
    queued
        .overrides
        .insert("agent.prompt".to_string(), "a".to_string());
    let mut running = ExecutionCandidate::new(
        "exec-candidates",
        "candidate-2",
        2,
        0,
        CandidateStatus::Running,
    );
    running.runtime_run_id = Some("run-2".to_string());
    running
        .overrides
        .insert("agent.prompt".to_string(), "b".to_string());

    store.save_candidate(&queued).expect("save queued");
    store.save_candidate(&running).expect("save running");

    let snapshot = store.load_execution("exec-candidates").expect("reload");
    assert_eq!(snapshot.candidates.len(), 2);
    assert_eq!(snapshot.candidates[0].candidate_id, "candidate-1");
    assert_eq!(snapshot.candidates[0].status, CandidateStatus::Queued);
    assert_eq!(
        snapshot.candidates[0]
            .overrides
            .get("agent.prompt")
            .map(String::as_str),
        Some("a")
    );
    assert_eq!(snapshot.candidates[1].candidate_id, "candidate-2");
    assert_eq!(snapshot.candidates[1].status, CandidateStatus::Running);
    assert_eq!(
        snapshot.candidates[1].runtime_run_id.as_deref(),
        Some("run-2")
    );
    assert_eq!(
        snapshot.candidates[1]
            .overrides
            .get("agent.prompt")
            .map(String::as_str),
        Some("b")
    );
}

#[test]
fn process_execution_persists_terminal_candidate_records() {
    let root = temp_store_dir("worker-candidate-lifecycle");
    let store = FsExecutionStore::new(root);
    let spec = spec(1);
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-candidate-lifecycle", &spec)
        .expect("submit");

    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        output(
            "candidate-1",
            &[("latency_p99_ms", 90.0), ("cost_usd", 0.03)],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        output(
            "candidate-2",
            &[("latency_p99_ms", 85.0), ("cost_usd", 0.02)],
        ),
    );

    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store.clone(),
    );
    service
        .process_execution("exec-candidate-lifecycle")
        .expect("process");

    let snapshot = store
        .load_execution("exec-candidate-lifecycle")
        .expect("reload");
    assert_eq!(snapshot.candidates.len(), 2);
    assert_eq!(snapshot.candidates[0].candidate_id, "candidate-1");
    assert_eq!(snapshot.candidates[0].status, CandidateStatus::Completed);
    assert_eq!(
        snapshot.candidates[0].runtime_run_id.as_deref(),
        Some("exec-run-candidate-1")
    );
    assert_eq!(
        snapshot.candidates[0]
            .overrides
            .get("agent.prompt")
            .map(String::as_str),
        Some("a")
    );
    assert_eq!(snapshot.candidates[0].succeeded, Some(true));
    assert_eq!(
        snapshot.candidates[0].metrics.get("latency_p99_ms"),
        Some(&90.0)
    );
    assert_eq!(snapshot.candidates[1].candidate_id, "candidate-2");
    assert_eq!(snapshot.candidates[1].status, CandidateStatus::Completed);
    assert_eq!(
        snapshot.candidates[1].runtime_run_id.as_deref(),
        Some("exec-run-candidate-2")
    );
    assert_eq!(
        snapshot.candidates[1]
            .overrides
            .get("agent.prompt")
            .map(String::as_str),
        Some("b")
    );
    assert_eq!(snapshot.candidates[1].succeeded, Some(true));
    assert_eq!(
        snapshot.candidates[1].metrics.get("latency_p99_ms"),
        Some(&85.0)
    );
}

#[test]
fn process_execution_persists_mixed_candidate_terminal_states() {
    let root = temp_store_dir("worker-candidate-mixed");
    let store = FsExecutionStore::new(root);
    let spec = spec(1);
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-candidate-mixed", &spec)
        .expect("submit");

    let mut runtime = MockRuntime::new();
    runtime.seed_failure("exec-run-candidate-1");
    runtime.seed_success(
        "exec-run-candidate-2",
        output(
            "candidate-2",
            &[("latency_p99_ms", 85.0), ("cost_usd", 0.02)],
        ),
    );

    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store.clone(),
    );
    let execution = service
        .process_execution("exec-candidate-mixed")
        .expect("process");

    assert_eq!(execution.status, ExecutionStatus::Completed);
    let snapshot = store
        .load_execution("exec-candidate-mixed")
        .expect("reload");
    assert_eq!(snapshot.candidates.len(), 2);
    assert_eq!(snapshot.candidates[0].candidate_id, "candidate-1");
    assert_eq!(snapshot.candidates[0].status, CandidateStatus::Failed);
    assert_eq!(snapshot.candidates[0].succeeded, Some(false));
    assert_eq!(snapshot.candidates[1].candidate_id, "candidate-2");
    assert_eq!(snapshot.candidates[1].status, CandidateStatus::Completed);
    assert_eq!(snapshot.candidates[1].succeeded, Some(true));
    assert_eq!(snapshot.candidates[1].metrics.get("cost_usd"), Some(&0.02));
}

#[test]
fn process_execution_releases_claim_after_completion() {
    let root = temp_store_dir("worker-release");
    let store = FsExecutionStore::new(root);
    let spec = spec(1);
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-release", &spec)
        .expect("submit");

    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        output(
            "candidate-1",
            &[("latency_p99_ms", 90.0), ("cost_usd", 0.03)],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        output(
            "candidate-2",
            &[("latency_p99_ms", 85.0), ("cost_usd", 0.02)],
        ),
    );

    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store.clone(),
    );
    let processed = service.process_execution("exec-release").expect("process");
    assert_eq!(processed.status, ExecutionStatus::Completed);
    assert_eq!(store.load_claim("exec-release").expect("claim"), None);

    let snapshot = store.load_execution("exec-release").expect("reload");
    assert_eq!(
        snapshot.execution.result_best_candidate_id.as_deref(),
        Some("candidate-2")
    );
    assert_eq!(snapshot.execution.completed_iterations, 1);
    assert_eq!(
        snapshot.execution.failure_counts.total_candidate_failures,
        0
    );
}

#[test]
fn process_execution_persists_lifecycle_events() {
    let root = temp_store_dir("worker-events");
    let store = FsExecutionStore::new(root);
    let spec = spec(1);
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-events", &spec)
        .expect("submit");

    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        output(
            "candidate-1",
            &[("latency_p99_ms", 90.0), ("cost_usd", 0.03)],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        output(
            "candidate-2",
            &[("latency_p99_ms", 85.0), ("cost_usd", 0.02)],
        ),
    );

    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store.clone(),
    );
    service.process_execution("exec-events").expect("process");

    let snapshot = store.load_execution("exec-events").expect("reload");
    let event_types: Vec<_> = snapshot
        .events
        .iter()
        .map(|event| event.event_type)
        .collect();
    assert!(
        event_types.contains(&void_control::orchestration::ControlEventType::ExecutionSubmitted)
    );
    assert!(event_types.contains(&void_control::orchestration::ControlEventType::ExecutionStarted));
    assert!(event_types.contains(&void_control::orchestration::ControlEventType::CandidateQueued));
    assert!(
        event_types.contains(&void_control::orchestration::ControlEventType::CandidateDispatched)
    );
    assert!(event_types
        .contains(&void_control::orchestration::ControlEventType::CandidateOutputCollected));
    assert!(
        event_types.contains(&void_control::orchestration::ControlEventType::ExecutionCompleted)
    );
}

#[test]
fn pause_interrupts_active_processing_and_persists_paused_status() {
    let root = temp_store_dir("worker-pause-active");
    let store = FsExecutionStore::new(root);
    let spec = spec(1);
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-pause-active", &spec)
        .expect("submit");

    let pause_store = store.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(150));
        let _ = ExecutionService::<MockRuntime>::update_execution_status(
            &pause_store,
            "exec-pause-active",
            void_control::orchestration::ExecutionAction::Pause,
        );
    });

    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        MockRuntime::new(),
        store.clone(),
    );
    let err = service
        .process_execution("exec-pause-active")
        .expect_err("pause should interrupt processing");
    assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);

    let snapshot = store.load_execution("exec-pause-active").expect("reload");
    assert_eq!(snapshot.execution.status, ExecutionStatus::Paused);
    assert!(snapshot.events.iter().any(|event| {
        event.event_type == void_control::orchestration::ControlEventType::ExecutionPaused
    }));
}

#[test]
fn cancel_interrupts_active_processing_and_returns_canceled_execution() {
    let root = temp_store_dir("worker-cancel-active");
    let store = FsExecutionStore::new(root);
    let spec = spec(1);
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-cancel-active", &spec)
        .expect("submit");

    let cancel_store = store.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(150));
        let _ = ExecutionService::<MockRuntime>::update_execution_status(
            &cancel_store,
            "exec-cancel-active",
            void_control::orchestration::ExecutionAction::Cancel,
        );
    });

    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        MockRuntime::new(),
        store.clone(),
    );
    let execution = service
        .process_execution("exec-cancel-active")
        .expect("cancel should return terminal execution");
    assert_eq!(execution.status, ExecutionStatus::Canceled);

    let snapshot = store.load_execution("exec-cancel-active").expect("reload");
    assert_eq!(snapshot.execution.status, ExecutionStatus::Canceled);
    assert!(snapshot.events.iter().any(|event| {
        event.event_type == void_control::orchestration::ControlEventType::ExecutionCanceled
    }));
}

#[test]
fn resumed_execution_can_be_processed_by_worker_loop() {
    let root = temp_store_dir("worker-resume");
    let store = FsExecutionStore::new(root.clone());
    let spec = spec(1);
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-resume", &spec)
        .expect("submit");
    let mut paused = store.load_execution("exec-resume").expect("load").execution;
    paused.status = ExecutionStatus::Paused;
    store.save_execution(&paused).expect("save paused");

    ExecutionService::<MockRuntime>::update_execution_status(
        &store,
        "exec-resume",
        void_control::orchestration::ExecutionAction::Resume,
    )
    .expect("resume");

    tick_bridge_worker_until_terminal(root, "exec-resume");

    let snapshot = store.load_execution("exec-resume").expect("reload");
    assert_eq!(snapshot.execution.status, ExecutionStatus::Completed);
    assert!(snapshot.events.iter().any(|event| {
        event.event_type == void_control::orchestration::ControlEventType::ExecutionResumed
    }));
}

#[test]
fn paused_execution_does_not_block_other_queued_work_in_bridge_scheduler() {
    let root = temp_store_dir("worker-paused-fairness");
    let store = FsExecutionStore::new(root.clone());
    let spec = spec(1);

    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-paused", &spec)
        .expect("submit paused");
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-running", &spec)
        .expect("submit running");

    let mut planner = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        MockRuntime::new(),
        store.clone(),
    );
    planner.plan_execution("exec-paused").expect("plan paused");
    planner
        .plan_execution("exec-running")
        .expect("plan running");

    let mut paused = store
        .load_execution("exec-paused")
        .expect("load paused")
        .execution;
    paused.status = ExecutionStatus::Paused;
    store.save_execution(&paused).expect("save paused");

    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-3",
        output(
            "candidate-1",
            &[("latency_p99_ms", 90.0), ("cost_usd", 0.03)],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-4",
        output(
            "candidate-2",
            &[("latency_p99_ms", 85.0), ("cost_usd", 0.02)],
        ),
    );

    void_control::bridge::process_pending_executions_once_for_test(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        root.clone(),
    )
    .expect("process pending");

    let paused_snapshot = store.load_execution("exec-paused").expect("reload paused");
    assert_eq!(paused_snapshot.execution.status, ExecutionStatus::Paused);
    assert!(paused_snapshot
        .candidates
        .iter()
        .all(|candidate| candidate.status == CandidateStatus::Queued));

    let running_snapshot = store
        .load_execution("exec-running")
        .expect("reload running");
    assert!(matches!(
        running_snapshot.execution.status,
        ExecutionStatus::Running | ExecutionStatus::Completed
    ));
    assert!(running_snapshot
        .candidates
        .iter()
        .any(|candidate| candidate.status == CandidateStatus::Completed));
}

#[test]
fn bridge_scheduler_dispatches_earliest_queued_execution_first() {
    let root = temp_store_dir("worker-bridge-fifo");
    let store = FsExecutionStore::new(root.clone());
    let spec = spec(1);

    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-early", &spec)
        .expect("submit early");
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-late", &spec)
        .expect("submit late");

    let mut planner = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        MockRuntime::new(),
        store.clone(),
    );
    planner.plan_execution("exec-early").expect("plan early");
    planner.plan_execution("exec-late").expect("plan late");

    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        output(
            "candidate-1",
            &[("latency_p99_ms", 90.0), ("cost_usd", 0.03)],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        output(
            "candidate-2",
            &[("latency_p99_ms", 85.0), ("cost_usd", 0.02)],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-3",
        output(
            "candidate-1",
            &[("latency_p99_ms", 88.0), ("cost_usd", 0.03)],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-4",
        output(
            "candidate-2",
            &[("latency_p99_ms", 84.0), ("cost_usd", 0.02)],
        ),
    );

    void_control::bridge::process_pending_executions_once_for_test(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        root.clone(),
    )
    .expect("process pending");

    let early = store.load_execution("exec-early").expect("reload early");
    let late = store.load_execution("exec-late").expect("reload late");
    let early_completed = early
        .candidates
        .iter()
        .filter(|candidate| candidate.status == CandidateStatus::Completed)
        .count();
    let late_completed = late
        .candidates
        .iter()
        .filter(|candidate| candidate.status == CandidateStatus::Completed)
        .count();

    assert_eq!(early.execution.status, ExecutionStatus::Running);
    assert_eq!(late.execution.status, ExecutionStatus::Running);
    assert_eq!(early.candidates[0].status, CandidateStatus::Completed);
    assert_eq!(late.candidates[0].status, CandidateStatus::Completed);
    assert!(
        early_completed >= late_completed,
        "earlier execution should not make less progress than later execution in the same bridge tick"
    );
}

fn spec(max_iterations: u32) -> ExecutionSpec {
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

fn spec_with_candidate_count(
    max_iterations: u32,
    max_concurrent_candidates: u32,
    candidate_count: usize,
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
                max_concurrent_candidates,
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
            candidate_count as u32,
            (0..candidate_count)
                .map(|idx| VariationProposal {
                    overrides: BTreeMap::from([(
                        "agent.prompt".to_string(),
                        format!("candidate-{idx}"),
                    )]),
                })
                .collect(),
        ),
        swarm: true,
        supervision: None,
    }
}

fn single_candidate_spec() -> ExecutionSpec {
    ExecutionSpec {
        mode: "swarm".to_string(),
        goal: "optimize".to_string(),
        workflow: void_control::orchestration::WorkflowTemplateRef {
            template: "fixtures/sample.vbrun".to_string(),
        },
        policy: OrchestrationPolicy {
            budget: void_control::orchestration::BudgetPolicy {
                max_iterations: Some(1),
                max_child_runs: None,
                max_wall_clock_secs: Some(60),
                max_cost_usd_millis: None,
            },
            concurrency: void_control::orchestration::ConcurrencyPolicy {
                max_concurrent_candidates: 1,
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
            1,
            vec![VariationProposal {
                overrides: BTreeMap::from([("agent.prompt".to_string(), "a".to_string())]),
            }],
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
    let dir = std::env::temp_dir().join(format!("void-control-worker-{label}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn tick_bridge_worker_until_terminal(root: std::path::PathBuf, execution_id: &str) {
    let store = FsExecutionStore::new(root.clone());
    for _ in 0..6 {
        let mut runtime = MockRuntime::new();
        runtime.seed_success(
            "exec-run-candidate-1",
            output(
                "candidate-1",
                &[("latency_p99_ms", 90.0), ("cost_usd", 0.03)],
            ),
        );
        runtime.seed_success(
            "exec-run-candidate-2",
            output(
                "candidate-2",
                &[("latency_p99_ms", 85.0), ("cost_usd", 0.02)],
            ),
        );
        void_control::bridge::process_pending_executions_once_for_test(
            GlobalConfig {
                max_concurrent_child_runs: 2,
            },
            runtime,
            root.clone(),
        )
        .expect("process pending");
        let snapshot = store.load_execution(execution_id).expect("reload");
        if matches!(
            snapshot.execution.status,
            ExecutionStatus::Completed | ExecutionStatus::Failed | ExecutionStatus::Canceled
        ) {
            return;
        }
    }
    panic!("execution did not reach terminal state");
}

struct StepwiseRuntime {
    terminal_after_inspects: usize,
    inspect_counts: RefCell<BTreeMap<String, usize>>,
    outputs: BTreeMap<String, CandidateOutput>,
}

struct RunningOutputReadyRuntime {
    output: Option<CandidateOutput>,
}

struct RunningOutputMissingRuntime;

struct RunningOutputNotFoundRuntime;

struct TerminalFailedRuntime {
    output: Option<CandidateOutput>,
}

impl RunningOutputReadyRuntime {
    fn new(output: CandidateOutput) -> Self {
        Self {
            output: Some(output),
        }
    }
}

impl TerminalFailedRuntime {
    fn new(output: Option<CandidateOutput>) -> Self {
        Self { output }
    }
}

impl StepwiseRuntime {
    fn new(terminal_after_inspects: usize) -> Self {
        Self {
            terminal_after_inspects,
            inspect_counts: RefCell::new(BTreeMap::new()),
            outputs: BTreeMap::new(),
        }
    }

    fn with_outputs<const N: usize>(
        terminal_after_inspects: usize,
        outputs: [(&str, CandidateOutput); N],
    ) -> Self {
        Self {
            terminal_after_inspects,
            inspect_counts: RefCell::new(BTreeMap::new()),
            outputs: outputs
                .into_iter()
                .map(|(run_id, output)| (run_id.to_string(), output))
                .collect(),
        }
    }
}

impl void_control::orchestration::service::ExecutionRuntime for StepwiseRuntime {
    fn start_run(&mut self, request: StartRequest) -> Result<StartResult, ContractError> {
        Ok(StartResult {
            handle: format!("step:{}", request.run_id),
            attempt_id: 1,
            state: RunState::Running,
        })
    }

    fn inspect_run(&self, handle: &str) -> Result<RuntimeInspection, ContractError> {
        let run_id = handle.strip_prefix("step:").ok_or_else(|| {
            ContractError::new(
                ContractErrorCode::NotFound,
                format!("unknown handle '{handle}'"),
                false,
            )
        })?;
        let mut counts = self.inspect_counts.borrow_mut();
        let count = counts.entry(run_id.to_string()).or_insert(0);
        *count += 1;
        let terminal = *count > self.terminal_after_inspects;
        Ok(RuntimeInspection {
            run_id: run_id.to_string(),
            attempt_id: 1,
            state: if terminal {
                RunState::Succeeded
            } else {
                RunState::Running
            },
            active_stage_count: if terminal { 0 } else { 1 },
            active_microvm_count: if terminal { 0 } else { 1 },
            started_at: "0Z".to_string(),
            updated_at: "0Z".to_string(),
            terminal_reason: None,
            exit_code: None,
        })
    }

    fn take_structured_output(
        &mut self,
        run_id: &str,
    ) -> void_control::orchestration::service::StructuredOutputResult {
        self.outputs
            .get(run_id)
            .cloned()
            .map(void_control::orchestration::service::StructuredOutputResult::Found)
            .unwrap_or(void_control::orchestration::service::StructuredOutputResult::Missing)
    }

    fn inline_poll_budget(&self) -> usize {
        1
    }

    fn persisted_run_handle(&self, persisted_run_id: &str) -> String {
        if persisted_run_id.starts_with("step:") {
            persisted_run_id.to_string()
        } else {
            format!("step:{persisted_run_id}")
        }
    }
}

impl void_control::orchestration::service::ExecutionRuntime for TerminalFailedRuntime {
    fn start_run(&mut self, request: StartRequest) -> Result<StartResult, ContractError> {
        Ok(StartResult {
            handle: format!("tf:{}", request.run_id),
            attempt_id: 1,
            state: RunState::Running,
        })
    }

    fn inspect_run(&self, handle: &str) -> Result<RuntimeInspection, ContractError> {
        let run_id = handle.strip_prefix("tf:").ok_or_else(|| {
            ContractError::new(
                ContractErrorCode::NotFound,
                format!("unknown handle '{handle}'"),
                false,
            )
        })?;
        Ok(RuntimeInspection {
            run_id: run_id.to_string(),
            attempt_id: 1,
            state: RunState::Failed,
            active_stage_count: 0,
            active_microvm_count: 0,
            started_at: "0Z".to_string(),
            updated_at: "0Z".to_string(),
            terminal_reason: Some("forced terminal failure".to_string()),
            exit_code: Some(1),
        })
    }

    fn take_structured_output(
        &mut self,
        _run_id: &str,
    ) -> void_control::orchestration::service::StructuredOutputResult {
        self.output
            .take()
            .map(void_control::orchestration::service::StructuredOutputResult::Found)
            .unwrap_or(void_control::orchestration::service::StructuredOutputResult::Missing)
    }
}

impl void_control::orchestration::service::ExecutionRuntime for RunningOutputReadyRuntime {
    fn start_run(&mut self, request: StartRequest) -> Result<StartResult, ContractError> {
        Ok(StartResult {
            handle: format!("ready:{}", request.run_id),
            attempt_id: 1,
            state: RunState::Running,
        })
    }

    fn inspect_run(&self, handle: &str) -> Result<RuntimeInspection, ContractError> {
        let run_id = handle.strip_prefix("ready:").ok_or_else(|| {
            ContractError::new(
                ContractErrorCode::NotFound,
                format!("unknown handle '{handle}'"),
                false,
            )
        })?;
        Ok(RuntimeInspection {
            run_id: run_id.to_string(),
            attempt_id: 1,
            state: RunState::Running,
            active_stage_count: 0,
            active_microvm_count: 0,
            started_at: "0Z".to_string(),
            updated_at: "0Z".to_string(),
            terminal_reason: None,
            exit_code: None,
        })
    }

    fn take_structured_output(
        &mut self,
        _run_id: &str,
    ) -> void_control::orchestration::service::StructuredOutputResult {
        self.output
            .take()
            .map(void_control::orchestration::service::StructuredOutputResult::Found)
            .unwrap_or(void_control::orchestration::service::StructuredOutputResult::Missing)
    }

    fn inline_poll_budget(&self) -> usize {
        1
    }

    fn persisted_run_handle(&self, persisted_run_id: &str) -> String {
        if persisted_run_id.starts_with("ready:") {
            persisted_run_id.to_string()
        } else {
            format!("ready:{persisted_run_id}")
        }
    }
}

impl void_control::orchestration::service::ExecutionRuntime for RunningOutputMissingRuntime {
    fn start_run(&mut self, request: StartRequest) -> Result<StartResult, ContractError> {
        Ok(StartResult {
            handle: format!("missing:{}", request.run_id),
            attempt_id: 1,
            state: RunState::Running,
        })
    }

    fn inspect_run(&self, handle: &str) -> Result<RuntimeInspection, ContractError> {
        let run_id = handle.strip_prefix("missing:").ok_or_else(|| {
            ContractError::new(
                ContractErrorCode::NotFound,
                format!("unknown handle '{handle}'"),
                false,
            )
        })?;
        Ok(RuntimeInspection {
            run_id: run_id.to_string(),
            attempt_id: 1,
            state: RunState::Running,
            active_stage_count: 0,
            active_microvm_count: 0,
            started_at: "0Z".to_string(),
            updated_at: "0Z".to_string(),
            terminal_reason: None,
            exit_code: None,
        })
    }

    fn take_structured_output(
        &mut self,
        _run_id: &str,
    ) -> void_control::orchestration::service::StructuredOutputResult {
        void_control::orchestration::service::StructuredOutputResult::Error(ContractError::new(
            ContractErrorCode::StructuredOutputMissing,
            "output not ready yet",
            false,
        ))
    }

    fn inline_poll_budget(&self) -> usize {
        1
    }

    fn persisted_run_handle(&self, persisted_run_id: &str) -> String {
        if persisted_run_id.starts_with("missing:") {
            persisted_run_id.to_string()
        } else {
            format!("missing:{persisted_run_id}")
        }
    }
}

impl void_control::orchestration::service::ExecutionRuntime for RunningOutputNotFoundRuntime {
    fn start_run(&mut self, request: StartRequest) -> Result<StartResult, ContractError> {
        Ok(StartResult {
            handle: format!("notfound:{}", request.run_id),
            attempt_id: 1,
            state: RunState::Running,
        })
    }

    fn inspect_run(&self, handle: &str) -> Result<RuntimeInspection, ContractError> {
        let run_id = handle.strip_prefix("notfound:").ok_or_else(|| {
            ContractError::new(
                ContractErrorCode::NotFound,
                format!("unknown handle '{handle}'"),
                false,
            )
        })?;
        Ok(RuntimeInspection {
            run_id: run_id.to_string(),
            attempt_id: 1,
            state: RunState::Running,
            active_stage_count: 0,
            active_microvm_count: 0,
            started_at: "0Z".to_string(),
            updated_at: "0Z".to_string(),
            terminal_reason: None,
            exit_code: None,
        })
    }

    fn take_structured_output(
        &mut self,
        _run_id: &str,
    ) -> void_control::orchestration::service::StructuredOutputResult {
        void_control::orchestration::service::StructuredOutputResult::Error(ContractError::new(
            ContractErrorCode::NotFound,
            "output path not published yet",
            false,
        ))
    }

    fn inline_poll_budget(&self) -> usize {
        1
    }

    fn persisted_run_handle(&self, persisted_run_id: &str) -> String {
        if persisted_run_id.starts_with("notfound:") {
            persisted_run_id.to_string()
        } else {
            format!("notfound:{persisted_run_id}")
        }
    }
}
