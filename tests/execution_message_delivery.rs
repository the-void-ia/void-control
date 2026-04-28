#![cfg(feature = "serde")]

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use void_control::contract::{
    ContractError, RunState, RuntimeInspection, StartRequest, StartResult,
};
use void_control::orchestration::service::ExecutionRuntime;
use void_control::orchestration::{
    CandidateOutput, CandidateStatus, CommunicationIntent, CommunicationIntentAudience,
    CommunicationIntentKind, CommunicationIntentPriority, EvaluationConfig, ExecutionCandidate,
    ExecutionService, ExecutionSpec, FsExecutionStore, GlobalConfig, InboxEntry, InboxSnapshot,
    OrchestrationPolicy, StructuredOutputResult, VariationConfig, VariationProposal,
    WorkflowTemplateRef,
};
use void_control::runtime::{
    DeliveryCapability, HttpSidecarAdapter, MessageDeliveryAdapter, VoidBoxRunRef,
};

fn run_ref() -> VoidBoxRunRef {
    VoidBoxRunRef {
        daemon_base_url: "unix:///tmp/voidbox-test.sock".to_string(),
        run_id: "run-123".to_string(),
    }
}

fn inbox_entry() -> InboxEntry {
    InboxEntry {
        message_id: "message-1".to_string(),
        intent_id: "intent-1".to_string(),
        from_candidate_id: "candidate-a".to_string(),
        kind: CommunicationIntentKind::Proposal,
        payload: serde_json::json!({
            "summary_text": "keep moving forward"
        }),
    }
}

fn intent(
    intent_id: &str,
    audience: CommunicationIntentAudience,
    summary_text: &str,
) -> CommunicationIntent {
    CommunicationIntent {
        intent_id: intent_id.to_string(),
        from_candidate_id: "candidate-source".to_string(),
        iteration: 0,
        kind: CommunicationIntentKind::Proposal,
        audience,
        payload: serde_json::json!({
            "summary_text": summary_text,
            "strategy_hint": summary_text,
        }),
        priority: CommunicationIntentPriority::Normal,
        ttl_iterations: 1,
        caused_by: None,
        context: None,
    }
}

#[test]
fn runtime_exports_delivery_types_through_public_module() {
    let _capability = DeliveryCapability::LaunchInjection;
    let _run_ref = run_ref();
    let _adapter = HttpSidecarAdapter::new();
}

#[test]
fn http_sidecar_adapter_declares_launch_injection_and_live_poll() {
    let adapter = HttpSidecarAdapter::new();

    assert_eq!(
        adapter.capabilities(),
        vec![
            DeliveryCapability::LaunchInjection,
            DeliveryCapability::LivePoll,
        ]
    );
}

#[tokio::test]
async fn message_delivery_adapter_push_live_defaults_to_unsupported() {
    let adapter = HttpSidecarAdapter::new();
    let err = adapter
        .push_live(&run_ref(), &inbox_entry())
        .await
        .expect_err("push_live should be unsupported by default");

    assert_eq!(err.kind(), ErrorKind::Unsupported);
}

#[test]
fn http_sidecar_adapter_generates_generic_messaging_skill_content() {
    let adapter = HttpSidecarAdapter::new();
    let skill = adapter.messaging_skill(&run_ref());

    assert!(skill.contains("Collaboration Protocol"));
    assert!(skill.contains("GET http://10.0.2.2:8090/v1/inbox"));
    assert!(skill.contains("POST http://10.0.2.2:8090/v1/intents"));
    assert!(skill.contains("proposal: concrete solution or approach"));
    assert!(skill.contains("signal: observation other agents should know"));
    assert!(skill.contains("evaluation: assessment of another agent's proposal"));
    assert!(skill.contains("broadcast: all agents"));
    assert!(skill.contains("leader: coordinator only"));
}

#[tokio::test]
async fn service_with_delivery_adapter_injects_snapshot_and_persists_drained_intents() {
    let starts: Arc<Mutex<Vec<StartRequest>>> = Arc::new(Mutex::new(Vec::new()));
    let injected = Arc::new(Mutex::new(Vec::new()));
    let drained = Arc::new(Mutex::new(Vec::new()));
    drained.lock().expect("lock drained").push(vec![
        intent(
            "sidecar-1",
            CommunicationIntentAudience::Broadcast,
            "keep cache warm",
        ),
        intent(
            "sidecar-dup",
            CommunicationIntentAudience::Leader,
            "prefer sidecar",
        ),
    ]);

    let runtime = RecordingDeliveryRuntime::new(
        starts.clone(),
        CandidateOutput::new("candidate-1", true, BTreeMap::new()).with_intents(vec![intent(
            "output-dup",
            CommunicationIntentAudience::Leader,
            "prefer sidecar",
        )]),
    );
    let adapter = RecordingDeliveryAdapter::new(injected.clone(), drained.clone());

    let root = temp_store_root("message-delivery-service");
    let store = FsExecutionStore::new(root.clone());
    let spec = launch_spec();
    let snapshot = InboxSnapshot {
        execution_id: "exec-delivery".to_string(),
        candidate_id: "candidate-1".to_string(),
        iteration: 0,
        entries: vec![inbox_entry()],
    };

    ExecutionService::<RecordingDeliveryRuntime>::submit_execution(&store, "exec-delivery", &spec)
        .expect("submit execution");
    store
        .save_candidate(&ExecutionCandidate::new(
            "exec-delivery",
            "candidate-1",
            1,
            0,
            CandidateStatus::Queued,
        ))
        .expect("seed queued candidate");
    store
        .save_inbox_snapshot(&snapshot)
        .expect("save inbox snapshot");

    let mut service = ExecutionService::with_delivery_adapter(
        GlobalConfig {
            max_concurrent_child_runs: 1,
        },
        runtime,
        store.clone(),
        Box::new(adapter),
    );

    service
        .dispatch_execution_once("exec-delivery")
        .await
        .expect("dispatch once");

    let requests = starts.lock().expect("starts poisoned");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].launch_context, None);

    let injected_calls = injected.lock().expect("lock injected");
    assert_eq!(injected_calls.len(), 1);
    assert_eq!(injected_calls[0].0.run_id, "exec-run-candidate-1");
    assert_eq!(injected_calls[0].1, snapshot);

    let intents = store
        .load_intents("exec-delivery")
        .expect("load merged intents");
    assert_eq!(intents.len(), 2);
    assert_eq!(intents[0].intent_id, "sidecar-1");
    assert_eq!(intents[1].intent_id, "sidecar-dup");
}

#[tokio::test]
async fn service_with_delivery_adapter_requires_runtime_run_refs() {
    let root = temp_store_root("message-delivery-missing-run-ref");
    let store = FsExecutionStore::new(root);
    let spec = launch_spec();
    let snapshot = InboxSnapshot {
        execution_id: "exec-delivery".to_string(),
        candidate_id: "candidate-1".to_string(),
        iteration: 0,
        entries: vec![inbox_entry()],
    };

    ExecutionService::<NoRunRefRuntime>::submit_execution(&store, "exec-delivery", &spec)
        .expect("submit execution");
    store
        .save_candidate(&ExecutionCandidate::new(
            "exec-delivery",
            "candidate-1",
            1,
            0,
            CandidateStatus::Queued,
        ))
        .expect("seed queued candidate");
    store
        .save_inbox_snapshot(&snapshot)
        .expect("save inbox snapshot");

    let mut service = ExecutionService::with_delivery_adapter(
        GlobalConfig {
            max_concurrent_child_runs: 1,
        },
        NoRunRefRuntime,
        store,
        Box::new(RecordingDeliveryAdapter::new(
            Arc::new(Mutex::new(Vec::new())),
            Arc::new(Mutex::new(Vec::new())),
        )),
    );

    let err = service
        .dispatch_execution_once("exec-delivery")
        .await
        .expect_err("runtime without delivery refs should fail");

    assert_eq!(err.kind(), ErrorKind::Unsupported);
}

#[tokio::test]
async fn sidecar_leader_and_broadcast_intents_route_differently_across_iterations() {
    let starts: Arc<Mutex<Vec<StartRequest>>> = Arc::new(Mutex::new(Vec::new()));
    let injected = Arc::new(Mutex::new(Vec::new()));
    let drained = Arc::new(Mutex::new(BTreeMap::from([(
        "exec-run-candidate-1".to_string(),
        vec![
            intent(
                "sidecar-leader",
                CommunicationIntentAudience::Leader,
                "@leader investigate cache contention",
            ),
            intent(
                "sidecar-broadcast",
                CommunicationIntentAudience::Broadcast,
                "@broadcast align on cache warmup",
            ),
        ],
    )])));

    let runtime = SeededDeliveryRuntime::new(
        starts.clone(),
        BTreeMap::from([
            (
                "exec-run-candidate-1".to_string(),
                CandidateOutput::new("candidate-1", true, BTreeMap::new()),
            ),
            (
                "exec-run-candidate-2".to_string(),
                CandidateOutput::new("candidate-2", true, BTreeMap::new()),
            ),
            (
                "exec-run-candidate-3".to_string(),
                CandidateOutput::new("candidate-3", true, BTreeMap::new()),
            ),
            (
                "exec-run-candidate-4".to_string(),
                CandidateOutput::new("candidate-4", true, BTreeMap::new()),
            ),
        ]),
    );
    let adapter = MappingDeliveryAdapter::new(injected, drained);

    let root = temp_store_root("message-delivery-routing");
    let store = FsExecutionStore::new(root.clone());
    let mut service = ExecutionService::with_delivery_adapter(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store,
        Box::new(adapter),
    );

    let execution = service
        .run_to_completion(two_iteration_swarm_spec())
        .await
        .expect("run execution");

    let store = FsExecutionStore::new(root);
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
            .filter(|message| { message.intent_id == "sidecar-leader" && message.to == "leader" })
            .count(),
        2
    );
    assert_eq!(
        messages
            .iter()
            .filter(|message| {
                message.intent_id == "sidecar-broadcast" && message.to == "broadcast"
            })
            .count(),
        3
    );
    assert_eq!(
        inbox_one
            .entries
            .iter()
            .map(|entry| entry.intent_id.as_str())
            .collect::<Vec<_>>(),
        vec!["sidecar-broadcast", "sidecar-leader"]
    );
    assert_eq!(
        inbox_two
            .entries
            .iter()
            .map(|entry| entry.intent_id.as_str())
            .collect::<Vec<_>>(),
        vec!["sidecar-broadcast"]
    );
}

#[tokio::test]
async fn candidate_completes_when_sidecar_intent_drain_fails_after_terminal_output() {
    let starts: Arc<Mutex<Vec<StartRequest>>> = Arc::new(Mutex::new(Vec::new()));
    let runtime = RecordingDeliveryRuntime::new(
        starts,
        CandidateOutput::new("candidate-1", true, BTreeMap::new()).with_intents(vec![intent(
            "output-only",
            CommunicationIntentAudience::Leader,
            "fallback to structured output intents",
        )]),
    );

    let root = temp_store_root("message-delivery-drain-failure");
    let store = FsExecutionStore::new(root.clone());
    let mut spec = launch_spec();
    spec.policy.budget.max_iterations = Some(1);
    let mut service = ExecutionService::with_delivery_adapter(
        GlobalConfig {
            max_concurrent_child_runs: 1,
        },
        runtime,
        store.clone(),
        Box::new(FailingDrainDeliveryAdapter),
    );

    let execution = service
        .run_to_completion(spec)
        .await
        .expect("execution should still complete");

    let candidate = store
        .load_candidates(&execution.execution_id)
        .expect("load candidates")
        .into_iter()
        .find(|candidate| candidate.candidate_id == "candidate-1")
        .expect("candidate should exist");
    assert_eq!(candidate.status, CandidateStatus::Completed);
    assert_eq!(
        candidate.runtime_run_id.as_deref(),
        Some("exec-run-candidate-1")
    );

    let intents = store
        .load_intents(&execution.execution_id)
        .expect("load intents after fallback");
    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].intent_id, "output-only");

    let snapshot = store
        .load_execution(&execution.execution_id)
        .expect("load execution snapshot");
    assert!(snapshot.events.iter().any(|event| {
        event.event_type == void_control::orchestration::ControlEventType::CandidateOutputCollected
    }));
}

struct RecordingDeliveryRuntime {
    starts: Arc<Mutex<Vec<StartRequest>>>,
    output: CandidateOutput,
}

impl RecordingDeliveryRuntime {
    fn new(starts: Arc<Mutex<Vec<StartRequest>>>, output: CandidateOutput) -> Self {
        Self { starts, output }
    }
}

struct SeededDeliveryRuntime {
    starts: Arc<Mutex<Vec<StartRequest>>>,
    outputs: BTreeMap<String, CandidateOutput>,
}

impl SeededDeliveryRuntime {
    fn new(
        starts: Arc<Mutex<Vec<StartRequest>>>,
        outputs: BTreeMap<String, CandidateOutput>,
    ) -> Self {
        Self { starts, outputs }
    }
}

#[async_trait::async_trait]
impl ExecutionRuntime for RecordingDeliveryRuntime {
    async fn start_run(&mut self, request: StartRequest) -> Result<StartResult, ContractError> {
        self.starts
            .lock()
            .expect("starts poisoned")
            .push(request.clone());
        Ok(StartResult {
            handle: format!("vb:{}", request.run_id),
            attempt_id: 1,
            state: RunState::Running,
        })
    }

    async fn inspect_run(&self, handle: &str) -> Result<RuntimeInspection, ContractError> {
        Ok(RuntimeInspection {
            run_id: handle.trim_start_matches("vb:").to_string(),
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

    async fn take_structured_output(&mut self, _run_id: &str) -> StructuredOutputResult {
        StructuredOutputResult::Found(self.output.clone())
    }

    fn delivery_run_ref(&self, handle: &str) -> Option<VoidBoxRunRef> {
        Some(VoidBoxRunRef {
            daemon_base_url: "unix:///tmp/voidbox-test.sock".to_string(),
            run_id: handle.trim_start_matches("vb:").to_string(),
        })
    }
}

#[async_trait::async_trait]
impl ExecutionRuntime for SeededDeliveryRuntime {
    async fn start_run(&mut self, request: StartRequest) -> Result<StartResult, ContractError> {
        self.starts
            .lock()
            .expect("starts poisoned")
            .push(request.clone());
        Ok(StartResult {
            handle: format!("vb:{}", request.run_id),
            attempt_id: 1,
            state: RunState::Running,
        })
    }

    async fn inspect_run(&self, handle: &str) -> Result<RuntimeInspection, ContractError> {
        Ok(RuntimeInspection {
            run_id: handle.trim_start_matches("vb:").to_string(),
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

    async fn take_structured_output(&mut self, run_id: &str) -> StructuredOutputResult {
        StructuredOutputResult::Found(
            self.outputs
                .get(run_id)
                .cloned()
                .unwrap_or_else(|| CandidateOutput::new(run_id, true, BTreeMap::new())),
        )
    }

    fn delivery_run_ref(&self, handle: &str) -> Option<VoidBoxRunRef> {
        Some(VoidBoxRunRef {
            daemon_base_url: "unix:///tmp/voidbox-test.sock".to_string(),
            run_id: handle.trim_start_matches("vb:").to_string(),
        })
    }
}

struct NoRunRefRuntime;

#[async_trait::async_trait]
impl ExecutionRuntime for NoRunRefRuntime {
    async fn start_run(&mut self, request: StartRequest) -> Result<StartResult, ContractError> {
        Ok(StartResult {
            handle: format!("vb:{}", request.run_id),
            attempt_id: 1,
            state: RunState::Running,
        })
    }

    async fn inspect_run(&self, handle: &str) -> Result<RuntimeInspection, ContractError> {
        Ok(RuntimeInspection {
            run_id: handle.trim_start_matches("vb:").to_string(),
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

    async fn take_structured_output(&mut self, _run_id: &str) -> StructuredOutputResult {
        StructuredOutputResult::Missing
    }
}

struct RecordingDeliveryAdapter {
    injected: Arc<Mutex<Vec<(VoidBoxRunRef, InboxSnapshot)>>>,
    drained: Arc<Mutex<Vec<Vec<CommunicationIntent>>>>,
}

impl RecordingDeliveryAdapter {
    fn new(
        injected: Arc<Mutex<Vec<(VoidBoxRunRef, InboxSnapshot)>>>,
        drained: Arc<Mutex<Vec<Vec<CommunicationIntent>>>>,
    ) -> Self {
        Self { injected, drained }
    }
}

#[async_trait::async_trait]
impl MessageDeliveryAdapter for RecordingDeliveryAdapter {
    fn capabilities(&self) -> Vec<DeliveryCapability> {
        vec![DeliveryCapability::LaunchInjection]
    }

    async fn inject_at_launch(
        &self,
        run: &VoidBoxRunRef,
        _candidate: &void_control::orchestration::CandidateSpec,
        inbox: &InboxSnapshot,
    ) -> std::io::Result<()> {
        self.injected
            .lock()
            .expect("lock injected")
            .push((run.clone(), inbox.clone()));
        Ok(())
    }

    async fn drain_intents(
        &self,
        _run: &VoidBoxRunRef,
    ) -> std::io::Result<Vec<CommunicationIntent>> {
        Ok(self
            .drained
            .lock()
            .expect("lock drained")
            .pop()
            .unwrap_or_default())
    }

    fn messaging_skill(&self, _run: &VoidBoxRunRef) -> String {
        "skill".to_string()
    }
}

struct MappingDeliveryAdapter {
    injected: Arc<Mutex<Vec<(VoidBoxRunRef, InboxSnapshot)>>>,
    drained: Arc<Mutex<BTreeMap<String, Vec<CommunicationIntent>>>>,
}

impl MappingDeliveryAdapter {
    fn new(
        injected: Arc<Mutex<Vec<(VoidBoxRunRef, InboxSnapshot)>>>,
        drained: Arc<Mutex<BTreeMap<String, Vec<CommunicationIntent>>>>,
    ) -> Self {
        Self { injected, drained }
    }
}

#[async_trait::async_trait]
impl MessageDeliveryAdapter for MappingDeliveryAdapter {
    fn capabilities(&self) -> Vec<DeliveryCapability> {
        vec![DeliveryCapability::LaunchInjection]
    }

    async fn inject_at_launch(
        &self,
        run: &VoidBoxRunRef,
        _candidate: &void_control::orchestration::CandidateSpec,
        inbox: &InboxSnapshot,
    ) -> std::io::Result<()> {
        self.injected
            .lock()
            .expect("lock injected")
            .push((run.clone(), inbox.clone()));
        Ok(())
    }

    async fn drain_intents(
        &self,
        run: &VoidBoxRunRef,
    ) -> std::io::Result<Vec<CommunicationIntent>> {
        Ok(self
            .drained
            .lock()
            .expect("lock drained map")
            .remove(&run.run_id)
            .unwrap_or_default())
    }

    fn messaging_skill(&self, _run: &VoidBoxRunRef) -> String {
        "skill".to_string()
    }
}

struct FailingDrainDeliveryAdapter;

#[async_trait::async_trait]
impl MessageDeliveryAdapter for FailingDrainDeliveryAdapter {
    fn capabilities(&self) -> Vec<DeliveryCapability> {
        vec![DeliveryCapability::LaunchInjection]
    }

    async fn inject_at_launch(
        &self,
        _run: &VoidBoxRunRef,
        _candidate: &void_control::orchestration::CandidateSpec,
        _inbox: &InboxSnapshot,
    ) -> std::io::Result<()> {
        Ok(())
    }

    async fn drain_intents(
        &self,
        _run: &VoidBoxRunRef,
    ) -> std::io::Result<Vec<CommunicationIntent>> {
        Err(std::io::Error::new(
            ErrorKind::NotFound,
            "sidecar intents unavailable after terminal output",
        ))
    }

    fn messaging_skill(&self, _run: &VoidBoxRunRef) -> String {
        "skill".to_string()
    }
}

fn launch_spec() -> ExecutionSpec {
    ExecutionSpec {
        mode: "swarm".to_string(),
        goal: "delivery adapter test".to_string(),
        workflow: WorkflowTemplateRef {
            template: temp_workflow_template("message-delivery-launch").to_string(),
        },
        policy: OrchestrationPolicy::default(),
        evaluation: EvaluationConfig {
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
        supervision: None,
    }
}

fn two_iteration_swarm_spec() -> ExecutionSpec {
    ExecutionSpec {
        mode: "swarm".to_string(),
        goal: "delivery routing".to_string(),
        workflow: WorkflowTemplateRef {
            template: temp_workflow_template("message-delivery-routing").to_string(),
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
        evaluation: EvaluationConfig {
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
                    overrides: BTreeMap::from([(
                        "agent.prompt".to_string(),
                        "baseline".to_string(),
                    )]),
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
        supervision: None,
    }
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

fn temp_workflow_template(label: &str) -> String {
    let path = temp_store_root(label).join("workflow-template.yaml");
    fs::write(
        &path,
        r#"api_version: v1
kind: agent
name: message-delivery-test

sandbox:
  mode: auto

llm:
  provider: claude

agent:
  prompt: baseline
"#,
    )
    .expect("write workflow template");
    path.to_string_lossy().into_owned()
}
