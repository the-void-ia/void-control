#![cfg(feature = "serde")]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use void_control::contract::{
    ContractError, RuntimeInspection, StartRequest, StartResult, RunState,
};
use void_control::orchestration::{
    CandidateOutput, CandidateSpec, CandidateStatus, CommunicationIntent, CommunicationIntentAudience,
    CommunicationIntentKind, CommunicationIntentPriority, ExecutionCandidate, ExecutionService, ExecutionSpec,
    FsExecutionStore, GlobalConfig, InboxEntry, InboxSnapshot, OrchestrationPolicy, RoutedMessage, RoutedMessageStatus,
    StructuredOutputResult, VariationConfig, VariationProposal, WorkflowTemplateRef,
};
use void_control::orchestration::service::ExecutionRuntime;
use void_control::runtime::MockRuntime;
use void_control::runtime::{LaunchInjectionAdapter, ProviderLaunchAdapter};

#[test]
fn fs_store_round_trips_message_box_logs() {
    let root = temp_store_root("message-box-logs");
    let store = FsExecutionStore::new(root.clone());

    let intent_one = CommunicationIntent {
        intent_id: "intent-1".to_string(),
        from_candidate_id: "candidate-1".to_string(),
        iteration: 0,
        kind: CommunicationIntentKind::Proposal,
        audience: CommunicationIntentAudience::Leader,
        payload: json_payload("summary-one", "hint-one"),
        priority: CommunicationIntentPriority::Normal,
        ttl_iterations: 1,
        caused_by: None,
        context: None,
    };
    let intent_two = CommunicationIntent {
        intent_id: "intent-2".to_string(),
        from_candidate_id: "candidate-2".to_string(),
        iteration: 1,
        kind: CommunicationIntentKind::Signal,
        audience: CommunicationIntentAudience::Broadcast,
        payload: json_payload("summary-two", "hint-two"),
        priority: CommunicationIntentPriority::High,
        ttl_iterations: 2,
        caused_by: Some("intent-1".to_string()),
        context: Some(json_context("family-a")),
    };
    let message_one = RoutedMessage {
        message_id: "message-1".to_string(),
        intent_id: "intent-1".to_string(),
        to: "leader".to_string(),
        delivery_iteration: 1,
        routing_reason: "leader_feedback_channel".to_string(),
        status: RoutedMessageStatus::Routed,
    };
    let message_two = RoutedMessage {
        message_id: "message-2".to_string(),
        intent_id: "intent-2".to_string(),
        to: "broadcast".to_string(),
        delivery_iteration: 2,
        routing_reason: "broadcast_fanout".to_string(),
        status: RoutedMessageStatus::Delivered,
    };

    store
        .append_intent("exec-message-box", &intent_one)
        .expect("append first intent");
    store
        .append_intent("exec-message-box", &intent_two)
        .expect("append second intent");
    store
        .append_routed_message("exec-message-box", &message_one)
        .expect("append first message");
    store
        .append_routed_message("exec-message-box", &message_two)
        .expect("append second message");

    let loaded_intents = store
        .load_intents("exec-message-box")
        .expect("load intents");
    let loaded_messages = store
        .load_routed_messages("exec-message-box")
        .expect("load messages");

    assert_eq!(loaded_intents, vec![intent_one, intent_two]);
    assert_eq!(loaded_messages, vec![message_one, message_two]);

    let intent_log = fs::read_to_string(root.join("exec-message-box").join("intents.log"))
        .expect("read intents log");
    assert_eq!(intent_log.lines().count(), 2);
    let message_log = fs::read_to_string(root.join("exec-message-box").join("messages.log"))
        .expect("read messages log");
    assert_eq!(message_log.lines().count(), 2);
}

#[test]
fn fs_store_round_trips_inbox_snapshot() {
    let root = temp_store_root("message-box-inbox");
    let store = FsExecutionStore::new(root.clone());

    let snapshot = InboxSnapshot {
        execution_id: "exec-message-box".to_string(),
        candidate_id: "candidate-3".to_string(),
        iteration: 1,
        entries: vec![
            InboxEntry {
                message_id: "message-1".to_string(),
                intent_id: "intent-1".to_string(),
                from_candidate_id: "candidate-1".to_string(),
                kind: CommunicationIntentKind::Proposal,
                payload: json_payload("summary-one", "hint-one"),
            },
            InboxEntry {
                message_id: "message-2".to_string(),
                intent_id: "intent-2".to_string(),
                from_candidate_id: "candidate-2".to_string(),
                kind: CommunicationIntentKind::Evaluation,
                payload: json_payload("summary-two", "hint-two"),
            },
        ],
    };

    store
        .save_inbox_snapshot(&snapshot)
        .expect("save inbox snapshot");

    let loaded = store
        .load_inbox_snapshot("exec-message-box", 1, "candidate-3")
        .expect("load inbox snapshot");

    assert_eq!(loaded, snapshot);

    let path = root
        .join("exec-message-box")
        .join("inboxes")
        .join("1")
        .join("candidate-3.json");
    let raw = fs::read_to_string(path).expect("read snapshot file");
    assert!(raw.contains("\"candidate_id\": \"candidate-3\""));
}

#[test]
fn fs_store_rejects_unsafe_inbox_snapshot_paths() {
    let root = temp_store_root("message-box-paths");
    let store = FsExecutionStore::new(root.clone());

    let snapshot = InboxSnapshot {
        execution_id: "exec-message-box".to_string(),
        candidate_id: "../escape".to_string(),
        iteration: 1,
        entries: Vec::new(),
    };

    let err = store
        .save_inbox_snapshot(&snapshot)
        .expect_err("reject traversal candidate id");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);

    let err = store
        .load_inbox_snapshot("exec-message-box", 1, "/absolute")
        .expect_err("reject absolute candidate id");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);

    let err = store
        .load_inbox_snapshot("exec-message-box", 1, "nested/id")
        .expect_err("reject nested candidate id");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);

    let inbox_dir = root.join("exec-message-box").join("inboxes");
    assert!(!inbox_dir.exists(), "unsafe path should not create inbox dirs");
}

#[test]
fn fs_store_ignores_truncated_ndjson_tail_when_loading_intents() {
    let root = temp_store_root("message-box-ndjson");
    let store = FsExecutionStore::new(root.clone());

    let intent = CommunicationIntent {
        intent_id: "intent-1".to_string(),
        from_candidate_id: "candidate-1".to_string(),
        iteration: 0,
        kind: CommunicationIntentKind::Proposal,
        audience: CommunicationIntentAudience::Leader,
        payload: json_payload("summary-one", "hint-one"),
        priority: CommunicationIntentPriority::Normal,
        ttl_iterations: 1,
        caused_by: None,
        context: None,
    };

    store
        .append_intent("exec-message-box", &intent)
        .expect("append valid intent");

    let log_path = root.join("exec-message-box").join("intents.log");
    fs::write(&log_path, format!("{}\n{{\"intent_id\":", serde_json::to_string(&intent).expect("serialize intent")))
        .expect("truncate tail");

    let loaded = store
        .load_intents("exec-message-box")
        .expect("load with truncated tail");

    assert_eq!(loaded, vec![intent]);
}

#[test]
fn service_launches_through_adapter_and_injects_inbox_content() {
    let runtime_requests = Rc::new(RefCell::new(Vec::new()));
    let adapter_calls = Rc::new(RefCell::new(Vec::<(String, InboxSnapshot)>::new()));

    let runtime = RecordingRuntime::new(runtime_requests.clone());
    let adapter = RecordingLaunchAdapter::new(adapter_calls.clone());

    let root = temp_store_root("message-box-launch-adapter");
    let store = FsExecutionStore::new(root);
    let spec = launch_spec();
    let snapshot = InboxSnapshot {
        execution_id: "exec-message-box".to_string(),
        candidate_id: "candidate-1".to_string(),
        iteration: 0,
        entries: vec![InboxEntry {
            message_id: "message-1".to_string(),
            intent_id: "intent-1".to_string(),
            from_candidate_id: "candidate-source".to_string(),
            kind: CommunicationIntentKind::Proposal,
            payload: json_payload("summary-one", "hint-one"),
        }],
    };

    ExecutionService::<RecordingRuntime>::submit_execution(&store, "exec-message-box", &spec)
        .expect("submit execution");
    store
        .save_candidate(&ExecutionCandidate::new(
            "exec-message-box",
            "candidate-1",
            1,
            0,
            CandidateStatus::Queued,
        ))
        .expect("seed queued candidate");
    store
        .save_inbox_snapshot(&snapshot)
        .expect("seed inbox snapshot");

    let mut service = ExecutionService::with_launch_adapter(
        GlobalConfig {
            max_concurrent_child_runs: 1,
        },
        runtime,
        store,
        Box::new(adapter),
    );

    let _ = service
        .dispatch_execution_once("exec-message-box")
        .expect("dispatch once");

    assert_eq!(adapter_calls.borrow().len(), 1);
    assert_eq!(adapter_calls.borrow()[0].0, "candidate-1");
    assert_eq!(adapter_calls.borrow()[0].1, snapshot);

    let requests = runtime_requests.borrow();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].workflow_spec, "workflow-template");
    let launch_context = requests[0]
        .launch_context
        .as_ref()
        .expect("launch context");
    let decoded: InboxSnapshot = serde_json::from_str(launch_context).expect("decode launch context");
    assert_eq!(decoded, snapshot);
}

#[test]
fn service_persists_routes_and_delivers_message_box_artifacts_across_iterations() {
    let root = temp_store_root("message-box-routing");
    let store = FsExecutionStore::new(root.clone());
    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        CandidateOutput::new(
            "candidate-1",
            true,
            BTreeMap::from([("latency_p99_ms".to_string(), 95.0)]),
        )
        .with_intents(vec![CommunicationIntent {
            intent_id: "intent-1".to_string(),
            from_candidate_id: "placeholder".to_string(),
            iteration: 0,
            kind: CommunicationIntentKind::Proposal,
            audience: CommunicationIntentAudience::Leader,
            payload: json_payload("try cache fallback", "cache"),
            priority: CommunicationIntentPriority::Normal,
            ttl_iterations: 1,
            caused_by: None,
            context: None,
        }]),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        CandidateOutput::new(
            "candidate-2",
            true,
            BTreeMap::from([("latency_p99_ms".to_string(), 80.0)]),
        )
        .with_intents(vec![CommunicationIntent {
            intent_id: "intent-2".to_string(),
            from_candidate_id: "placeholder".to_string(),
            iteration: 0,
            kind: CommunicationIntentKind::Signal,
            audience: CommunicationIntentAudience::Broadcast,
            payload: json_payload("jitter reduced spikes", "jitter"),
            priority: CommunicationIntentPriority::High,
            ttl_iterations: 1,
            caused_by: Some("intent-1".to_string()),
            context: None,
        }]),
    );
    runtime.seed_success(
        "exec-run-candidate-3",
        CandidateOutput::new(
            "candidate-3",
            true,
            BTreeMap::from([("latency_p99_ms".to_string(), 70.0)]),
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-4",
        CandidateOutput::new(
            "candidate-4",
            true,
            BTreeMap::from([("latency_p99_ms".to_string(), 72.0)]),
        ),
    );

    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store,
    );
    let execution = service
        .run_to_completion(two_iteration_swarm_spec())
        .expect("run execution");

    let store = FsExecutionStore::new(root);
    let snapshot = store
        .load_execution(&execution.execution_id)
        .expect("load execution");
    let intents = store
        .load_intents(&execution.execution_id)
        .expect("load intents");
    let messages = store
        .load_routed_messages(&execution.execution_id)
        .expect("load messages");
    let inbox_one = store
        .load_inbox_snapshot(&execution.execution_id, 1, "candidate-1")
        .expect("load candidate-1 inbox");
    let inbox_two = store
        .load_inbox_snapshot(&execution.execution_id, 1, "candidate-2")
        .expect("load candidate-2 inbox");

    assert_eq!(intents.len(), 2);
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.status == RoutedMessageStatus::Routed)
            .count(),
        2
    );
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.status == RoutedMessageStatus::Delivered)
            .count(),
        3
    );
    assert_eq!(inbox_one.entries.len(), 2);
    assert_eq!(inbox_two.entries.len(), 1);
    assert_event_count(
        &snapshot.events,
        void_control::orchestration::ControlEventType::CommunicationIntentEmitted,
        2,
    );
    assert_event_count(
        &snapshot.events,
        void_control::orchestration::ControlEventType::MessageRouted,
        2,
    );
    assert_event_count(
        &snapshot.events,
        void_control::orchestration::ControlEventType::MessageDelivered,
        3,
    );
}

struct RecordingRuntime {
    starts: Rc<RefCell<Vec<StartRequest>>>,
}

impl RecordingRuntime {
    fn new(starts: Rc<RefCell<Vec<StartRequest>>>) -> Self {
        Self { starts }
    }
}

impl ExecutionRuntime for RecordingRuntime {
    fn start_run(&mut self, request: StartRequest) -> Result<StartResult, ContractError> {
        self.starts.borrow_mut().push(request.clone());
        Ok(StartResult {
            handle: format!("run-handle:{}", request.run_id),
            attempt_id: 1,
            state: RunState::Running,
        })
    }

    fn inspect_run(&self, handle: &str) -> Result<RuntimeInspection, ContractError> {
        Ok(RuntimeInspection {
            run_id: handle
                .strip_prefix("run-handle:")
                .unwrap_or(handle)
                .to_string(),
            attempt_id: 1,
            state: RunState::Succeeded,
            active_stage_count: 0,
            active_microvm_count: 0,
            started_at: "now".to_string(),
            updated_at: "now".to_string(),
            terminal_reason: None,
            exit_code: None,
        })
    }

    fn take_structured_output(&mut self, _run_id: &str) -> StructuredOutputResult {
        StructuredOutputResult::Missing
    }
}

struct RecordingLaunchAdapter {
    calls: Rc<RefCell<Vec<(String, InboxSnapshot)>>>,
}

impl RecordingLaunchAdapter {
    fn new(calls: Rc<RefCell<Vec<(String, InboxSnapshot)>>>) -> Self {
        Self { calls }
    }
}

impl ProviderLaunchAdapter for RecordingLaunchAdapter {
    fn prepare_launch_request(
        &self,
        request: StartRequest,
        candidate: &CandidateSpec,
        inbox: &InboxSnapshot,
    ) -> StartRequest {
        self.calls
            .borrow_mut()
            .push((candidate.candidate_id.clone(), inbox.clone()));
        LaunchInjectionAdapter.prepare_launch_request(request, candidate, inbox)
    }
}

fn launch_spec() -> ExecutionSpec {
    ExecutionSpec {
        mode: "swarm".to_string(),
        goal: "launch adapter test".to_string(),
        workflow: WorkflowTemplateRef {
            template: "workflow-template".to_string(),
        },
        policy: OrchestrationPolicy::default(),
        evaluation: void_control::orchestration::EvaluationConfig {
            scoring_type: "weighted".to_string(),
            weights: Default::default(),
            pass_threshold: None,
            ranking: "descending".to_string(),
            tie_breaking: "lexicographic".to_string(),
        },
        variation: VariationConfig::explicit(
            1,
            vec![VariationProposal {
                overrides: BTreeMap::new(),
            }],
        ),
        swarm: true,
    }
}

fn two_iteration_swarm_spec() -> ExecutionSpec {
    ExecutionSpec {
        mode: "swarm".to_string(),
        goal: "message routing".to_string(),
        workflow: WorkflowTemplateRef {
            template: "workflow-template".to_string(),
        },
        policy: OrchestrationPolicy {
            budget: void_control::orchestration::BudgetPolicy {
                max_iterations: Some(2),
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
            scoring_type: "weighted".to_string(),
            weights: BTreeMap::from([("latency_p99_ms".to_string(), -1.0)]),
            pass_threshold: None,
            ranking: "descending".to_string(),
            tie_breaking: "lexicographic".to_string(),
        },
        variation: VariationConfig::explicit(
            2,
            vec![
                VariationProposal {
                    overrides: BTreeMap::from([("agent.prompt".to_string(), "baseline".to_string())]),
                },
                VariationProposal {
                    overrides: BTreeMap::from([("agent.prompt".to_string(), "v1".to_string())]),
                },
                VariationProposal {
                    overrides: BTreeMap::from([("agent.prompt".to_string(), "v2".to_string())]),
                },
                VariationProposal {
                    overrides: BTreeMap::from([("agent.prompt".to_string(), "v3".to_string())]),
                },
            ],
        ),
        swarm: true,
    }
}

fn json_payload(summary_text: &str, strategy_hint: &str) -> serde_json::Value {
    serde_json::json!({
        "summary_text": summary_text,
        "strategy_hint": strategy_hint,
    })
}

fn json_context(family_hint: &str) -> serde_json::Value {
    serde_json::json!({
        "family_hint": family_hint,
    })
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

fn assert_event_count(
    events: &[void_control::orchestration::ControlEventEnvelope],
    event_type: void_control::orchestration::ControlEventType,
    expected: usize,
) {
    let actual = events
        .iter()
        .filter(|event| event.event_type == event_type)
        .count();
    assert_eq!(actual, expected, "{event_type:?}");
}
