use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use void_control::orchestration::{
    CandidateStatus, ControlEventEnvelope, ControlEventType, Execution, ExecutionAccumulator,
    ExecutionCandidate, ExecutionStatus, FsExecutionStore, ReconciliationService,
};

#[test]
fn reloads_non_terminal_executions_after_restart() {
    let root = temp_store_root("reload-active");
    let store = FsExecutionStore::new(root.clone());
    let mut execution = Execution::new("exec-reload", "swarm", "reload state");
    execution.status = ExecutionStatus::Running;

    store.create_execution(&execution).expect("create execution");
    store
        .append_event(
            "exec-reload",
            &ControlEventEnvelope::new("exec-reload", 1, ControlEventType::ExecutionCreated),
        )
        .expect("append created");
    store
        .append_event(
            "exec-reload",
            &ControlEventEnvelope::new("exec-reload", 2, ControlEventType::IterationStarted),
        )
        .expect("append running");
    store
        .save_accumulator(
            "exec-reload",
            &ExecutionAccumulator {
                scoring_history_len: 1,
                completed_iterations: 1,
                ..ExecutionAccumulator::default()
            },
        )
        .expect("save accumulator");

    let reconciler = ReconciliationService::new(FsExecutionStore::new(root));
    let active = reconciler.reload_active_executions().expect("reload");

    assert_eq!(active.len(), 1);
    assert_eq!(active[0].execution.execution_id, "exec-reload");
    assert_eq!(active[0].accumulator.completed_iterations, 1);
}

#[test]
fn paused_execution_remains_paused_after_restart() {
    let root = temp_store_root("paused");
    let store = FsExecutionStore::new(root.clone());
    let mut execution = Execution::new("exec-paused", "swarm", "stay paused");
    execution.status = ExecutionStatus::Paused;

    store.create_execution(&execution).expect("create execution");

    let reconciler = ReconciliationService::new(FsExecutionStore::new(root));
    let active = reconciler.reload_active_executions().expect("reload");

    assert_eq!(active.len(), 1);
    assert_eq!(active[0].execution.status, ExecutionStatus::Paused);
}

#[test]
fn completed_execution_is_not_reloaded_as_active() {
    let root = temp_store_root("completed");
    let store = FsExecutionStore::new(root.clone());
    let mut execution = Execution::new("exec-complete", "swarm", "done");
    execution.status = ExecutionStatus::Completed;

    store.create_execution(&execution).expect("create execution");

    let reconciler = ReconciliationService::new(FsExecutionStore::new(root));
    let active = reconciler.reload_active_executions().expect("reload");

    assert!(active.is_empty());
}

#[test]
fn reloads_queued_candidates_fifo_across_active_executions() {
    let root = temp_store_root("queued-candidates");
    let store = FsExecutionStore::new(root.clone());

    let mut exec_a = Execution::new("exec-a", "swarm", "a");
    exec_a.status = ExecutionStatus::Running;
    store.create_execution(&exec_a).expect("create a");
    let mut a1 = ExecutionCandidate::new("exec-a", "cand-a1", 2, 0, CandidateStatus::Queued);
    a1.overrides.insert("agent.prompt".to_string(), "a1".to_string());
    store.save_candidate(&a1).expect("save a1");

    let mut exec_b = Execution::new("exec-b", "swarm", "b");
    exec_b.status = ExecutionStatus::Running;
    store.create_execution(&exec_b).expect("create b");
    let mut b1 = ExecutionCandidate::new("exec-b", "cand-b1", 1, 0, CandidateStatus::Queued);
    b1.overrides.insert("agent.prompt".to_string(), "b1".to_string());
    store.save_candidate(&b1).expect("save b1");

    let reconciler = ReconciliationService::new(FsExecutionStore::new(root));
    let queued = reconciler.reload_queued_candidates().expect("reload queued");

    assert_eq!(queued.len(), 2);
    assert_eq!(queued[0].execution_id, "exec-b");
    assert_eq!(queued[0].candidate_id, "cand-b1");
    assert_eq!(queued[1].execution_id, "exec-a");
    assert_eq!(queued[1].candidate_id, "cand-a1");
}

#[test]
fn paused_execution_candidates_are_excluded_from_reloaded_queue() {
    let root = temp_store_root("paused-queued-candidates");
    let store = FsExecutionStore::new(root.clone());

    let mut paused = Execution::new("exec-paused", "swarm", "paused");
    paused.status = ExecutionStatus::Paused;
    store.create_execution(&paused).expect("create paused");
    store
        .save_candidate(&ExecutionCandidate::new(
            "exec-paused",
            "cand-paused",
            1,
            0,
            CandidateStatus::Queued,
        ))
        .expect("save paused candidate");

    let mut running = Execution::new("exec-running", "swarm", "running");
    running.status = ExecutionStatus::Running;
    store.create_execution(&running).expect("create running");
    store
        .save_candidate(&ExecutionCandidate::new(
            "exec-running",
            "cand-running",
            2,
            0,
            CandidateStatus::Queued,
        ))
        .expect("save running candidate");

    let reconciler = ReconciliationService::new(FsExecutionStore::new(root));
    let queued = reconciler.reload_queued_candidates().expect("reload queued");

    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].execution_id, "exec-running");
    assert_eq!(queued[0].candidate_id, "cand-running");
}

fn temp_store_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = env::temp_dir().join(format!("void-control-reconcile-{label}-{nanos}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
