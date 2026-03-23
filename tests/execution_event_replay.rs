use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use void_control::orchestration::{
    ControlEventEnvelope, ControlEventType, Execution, ExecutionAccumulator, ExecutionSnapshot,
    ExecutionStatus, FsExecutionStore,
};

#[test]
fn execution_state_advances_from_control_plane_events() {
    let execution = Execution::new("exec-1", "swarm", "optimize latency");
    let events = vec![
        event(ControlEventType::ExecutionCreated),
        event(ControlEventType::ExecutionSubmitted),
        event(ControlEventType::ExecutionStarted),
        event(ControlEventType::ExecutionCompleted),
    ];

    let snapshot = ExecutionSnapshot::replay(execution, &events);

    assert_eq!(snapshot.execution.status, ExecutionStatus::Completed);
}

#[test]
fn warning_events_do_not_advance_execution_state() {
    let execution = Execution::new("exec-2", "swarm", "optimize latency");
    let events = vec![
        event(ControlEventType::ExecutionCreated),
        event(ControlEventType::ExecutionSubmitted),
        event(ControlEventType::CandidateQueued),
        event(ControlEventType::CandidateDispatched),
        event(ControlEventType::CandidateOutputCollected),
        event(ControlEventType::ExecutionStalled),
    ];

    let snapshot = ExecutionSnapshot::replay(execution, &events);

    assert_eq!(snapshot.execution.status, ExecutionStatus::Pending);
}

#[test]
fn accumulator_is_reconstructible_from_event_log() {
    let execution = Execution::new("exec-3", "swarm", "optimize latency");
    let events = vec![
        event(ControlEventType::ExecutionCreated),
        event(ControlEventType::CandidateScored),
        event(ControlEventType::IterationCompleted),
    ];

    let snapshot = ExecutionSnapshot::replay(execution, &events);

    assert_eq!(snapshot.accumulator.scoring_history_len, 1);
    assert_eq!(snapshot.accumulator.completed_iterations, 1);
}

#[test]
fn execution_started_event_advances_state_to_running() {
    let execution = Execution::new("exec-4", "swarm", "advance");
    let events = vec![
        event(ControlEventType::ExecutionCreated),
        event(ControlEventType::ExecutionSubmitted),
        event(ControlEventType::ExecutionStarted),
    ];

    let snapshot = ExecutionSnapshot::replay(execution, &events);

    assert_eq!(snapshot.execution.status, ExecutionStatus::Running);
}

#[test]
fn store_round_trips_execution_and_events() {
    let root = temp_store_root("round_trip");
    let store = FsExecutionStore::new(root.clone());
    let execution = Execution::new("exec-store-1", "swarm", "persist state");
    let events = vec![
        event(ControlEventType::ExecutionCreated),
        event(ControlEventType::IterationStarted),
    ];

    store.create_execution(&execution).expect("create execution");
    for event in &events {
        store.append_event("exec-store-1", event).expect("append event");
    }
    store
        .save_accumulator(
            "exec-store-1",
            &ExecutionAccumulator {
                scoring_history_len: 2,
                completed_iterations: 1,
                ..ExecutionAccumulator::default()
            },
        )
        .expect("save accumulator");

    let snapshot = store.load_execution("exec-store-1").expect("load snapshot");

    assert_eq!(snapshot.execution.execution_id, "exec-store-1");
    assert_eq!(snapshot.events.len(), 2);
    assert_eq!(snapshot.accumulator.scoring_history_len, 2);
}

#[test]
fn store_can_reload_accumulator_after_restart() {
    let root = temp_store_root("restart");
    let execution = Execution::new("exec-store-2", "swarm", "reload accumulator");

    {
        let store = FsExecutionStore::new(root.clone());
        store.create_execution(&execution).expect("create execution");
        store
            .save_accumulator(
                "exec-store-2",
                &ExecutionAccumulator {
                    scoring_history_len: 3,
                    completed_iterations: 2,
                    ..ExecutionAccumulator::default()
                },
            )
            .expect("save accumulator");
    }

    let reloaded_store = FsExecutionStore::new(root);
    let snapshot = reloaded_store
        .load_execution("exec-store-2")
        .expect("reload execution");

    assert_eq!(snapshot.accumulator.scoring_history_len, 3);
    assert_eq!(snapshot.accumulator.completed_iterations, 2);
}

fn event(event_type: ControlEventType) -> ControlEventEnvelope {
    ControlEventEnvelope::new("exec-test", 1, event_type)
}

fn temp_store_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = env::temp_dir().join(format!("void-control-{label}-{nanos}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
