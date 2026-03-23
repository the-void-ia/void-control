use std::collections::BTreeMap;

use void_control::orchestration::{
    CandidateOutput, CandidateStatus, ControlEventType, ExecutionService, ExecutionSpec,
    ExecutionStatus, FsExecutionStore, GlobalConfig, OrchestrationPolicy, VariationConfig, VariationProposal,
};
#[cfg(feature = "serde")]
use void_control::orchestration::{
    CommunicationIntent, CommunicationIntentAudience, CommunicationIntentKind,
    CommunicationIntentPriority,
};
use void_control::runtime::MockRuntime;

#[test]
fn swarm_strategy_runs_end_to_end() {
    let (execution, _, _) = run_mode_to_completion("swarm", temp_store_dir("swarm-acceptance"));

    assert_eq!(execution.status, ExecutionStatus::Completed);
    assert!(execution.result_best_candidate_id.is_some());
}

#[test]
fn search_strategy_runs_end_to_end() {
    let store_dir = temp_store_dir("search-acceptance");
    let (execution, store, _) = run_mode_to_completion("search", store_dir.clone());

    assert_eq!(execution.status, ExecutionStatus::Completed);
    assert!(execution.result_best_candidate_id.is_some());

    let candidates = store
        .load_candidates(&execution.execution_id)
        .expect("load candidates");
    let mut refinement_prompts: Vec<_> = candidates
        .iter()
        .filter(|candidate| candidate.iteration == 1)
        .map(|candidate| {
            candidate
                .overrides
                .get("agent.prompt")
                .expect("agent.prompt override")
                .clone()
        })
        .collect();
    refinement_prompts.sort();

    assert_eq!(refinement_prompts, vec!["v2".to_string(), "v3".to_string()]);
}

#[test]
fn supported_strategies_emit_expected_completion_events() {
    for mode in ["swarm", "search"] {
        let label = format!("{mode}-events");
        let (execution, _, snapshot) = run_mode_to_completion(mode, temp_store_dir(&label));

        assert_eq!(execution.status, ExecutionStatus::Completed, "{mode}");
        assert_event_counts(
            mode,
            &snapshot.events,
            &[
                (ControlEventType::ExecutionCreated, 1),
                (ControlEventType::ExecutionSubmitted, 1),
                (ControlEventType::ExecutionStarted, 1),
                (ControlEventType::IterationStarted, 2),
                (ControlEventType::CandidateQueued, 4),
                (ControlEventType::CandidateDispatched, 4),
                (ControlEventType::CandidateOutputCollected, 4),
                (ControlEventType::CandidateScored, 2),
                (ControlEventType::IterationCompleted, 2),
                (ControlEventType::ExecutionCompleted, 1),
                (ControlEventType::ExecutionFailed, 0),
            ],
        );
    }
}

#[test]
fn supported_strategies_persist_terminal_candidate_records() {
    for mode in ["swarm", "search"] {
        let label = format!("{mode}-candidates");
        let (execution, store, snapshot) = run_mode_to_completion(mode, temp_store_dir(&label));
        let candidates = store
            .load_candidates(&execution.execution_id)
            .expect("load candidates");
        let queued_count = snapshot
            .events
            .iter()
            .filter(|event| event.event_type == ControlEventType::CandidateQueued)
            .count();

        assert_eq!(candidates.len(), queued_count, "{mode}");
        assert!(!candidates.is_empty(), "{mode}");
        assert!(candidates.iter().all(|candidate| candidate.status == CandidateStatus::Completed), "{mode}");
        assert!(candidates.iter().all(|candidate| candidate.runtime_run_id.is_some()), "{mode}");
        assert!(candidates.iter().all(|candidate| candidate.succeeded == Some(true)), "{mode}");
    }
}

#[test]
fn supported_strategies_emit_failed_terminal_events_on_all_failure() {
    for mode in ["swarm", "search"] {
        let label = format!("{mode}-failed");
        let (execution, _, snapshot) = run_mode_with_all_failures(mode, temp_store_dir(&label));

        assert_eq!(execution.status, ExecutionStatus::Failed, "{mode}");
        assert_event_counts(
            mode,
            &snapshot.events,
            &[
                (ControlEventType::ExecutionCreated, 1),
                (ControlEventType::ExecutionSubmitted, 1),
                (ControlEventType::ExecutionStarted, 1),
                (ControlEventType::IterationStarted, 1),
                (ControlEventType::CandidateQueued, 2),
                (ControlEventType::CandidateDispatched, 2),
                (ControlEventType::CandidateOutputCollected, 2),
                (ControlEventType::CandidateScored, 1),
                (ControlEventType::IterationCompleted, 0),
                (ControlEventType::ExecutionCompleted, 0),
                (ControlEventType::ExecutionFailed, 1),
            ],
        );
    }
}

#[test]
fn search_strategy_refines_across_incremental_worker_ticks() {
    let store_dir = temp_store_dir("search-incremental");
    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        output("candidate-1", &[("latency_p99_ms", 95.0), ("cost_usd", 0.05)]),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        output("candidate-2", &[("latency_p99_ms", 80.0), ("cost_usd", 0.03)]),
    );
    runtime.seed_success(
        "exec-run-candidate-3",
        output("candidate-3", &[("latency_p99_ms", 70.0), ("cost_usd", 0.02)]),
    );
    runtime.seed_success(
        "exec-run-candidate-4",
        output("candidate-4", &[("latency_p99_ms", 72.0), ("cost_usd", 0.025)]),
    );

    let store = FsExecutionStore::new(store_dir.clone());
    ExecutionService::<MockRuntime>::submit_execution(&store, "exec-search", &strategy_spec("search"))
        .expect("submit execution");

    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store,
    );
    service.plan_execution("exec-search").expect("plan execution");

    for _ in 0..8 {
        let execution = service
            .dispatch_execution_once("exec-search")
            .expect("dispatch execution");
        if matches!(execution.status, ExecutionStatus::Completed | ExecutionStatus::Failed) {
            break;
        }
    }

    let store = FsExecutionStore::new(store_dir);
    let snapshot = store.load_execution("exec-search").expect("load execution");
    assert_eq!(snapshot.execution.status, ExecutionStatus::Completed);

    let mut refinement_prompts: Vec<_> = snapshot
        .candidates
        .iter()
        .filter(|candidate| candidate.iteration == 1)
        .map(|candidate| candidate.overrides["agent.prompt"].clone())
        .collect();
    refinement_prompts.sort();
    assert_eq!(refinement_prompts, vec!["v2".to_string(), "v3".to_string()]);
    assert_eq!(snapshot.accumulator.search_phase.as_deref(), Some("refine"));
}

#[cfg(feature = "serde")]
#[test]
fn swarm_strategy_routes_intents_into_next_iteration_message_box_and_events() {
    let store_dir = temp_store_dir("swarm-message-box");
    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        output_with_intents(
            "candidate-1",
            &[("latency_p99_ms", 95.0), ("cost_usd", 0.05)],
            vec![proposal_intent(
                "intent-swarm-leader",
                CommunicationIntentAudience::Leader,
                "leader: favor cache fallback",
                None,
            )],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        output_with_intents(
            "candidate-2",
            &[("latency_p99_ms", 80.0), ("cost_usd", 0.03)],
            vec![proposal_intent(
                "intent-swarm-broadcast",
                CommunicationIntentAudience::Broadcast,
                "broadcast: jitter helps",
                None,
            )],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-3",
        output("candidate-3", &[("latency_p99_ms", 70.0), ("cost_usd", 0.02)]),
    );
    runtime.seed_success(
        "exec-run-candidate-4",
        output("candidate-4", &[("latency_p99_ms", 72.0), ("cost_usd", 0.025)]),
    );

    let store = FsExecutionStore::new(store_dir.clone());
    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store,
    );
    let execution = service
        .run_to_completion(strategy_spec("swarm"))
        .expect("run execution");

    let store = FsExecutionStore::new(store_dir);
    let snapshot = store.load_execution(&execution.execution_id).expect("load execution");
    let intents = store.load_intents(&execution.execution_id).expect("load intents");
    let messages = store
        .load_routed_messages(&execution.execution_id)
        .expect("load routed messages");
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
            .filter(|message| message.status == void_control::orchestration::RoutedMessageStatus::Routed)
            .count(),
        2
    );
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.status == void_control::orchestration::RoutedMessageStatus::Delivered)
            .count(),
        3
    );
    assert_eq!(inbox_one.entries.len(), 2);
    assert_eq!(inbox_two.entries.len(), 1);
    assert_event_counts(
        "swarm-message-box",
        &snapshot.events,
        &[
            (ControlEventType::CommunicationIntentEmitted, 2),
            (ControlEventType::MessageRouted, 2),
            (ControlEventType::MessageDelivered, 3),
            (ControlEventType::ExecutionCompleted, 1),
        ],
    );
}

#[cfg(feature = "serde")]
#[test]
fn search_strategy_persists_lineage_and_delivers_parent_intent_to_refinement_iteration() {
    let store_dir = temp_store_dir("search-message-box");
    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        output_with_intents(
            "candidate-1",
            &[("latency_p99_ms", 95.0), ("cost_usd", 0.05)],
            vec![proposal_intent(
                "intent-search-parent",
                CommunicationIntentAudience::Leader,
                "start from rate limit baseline",
                None,
            )],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        output("candidate-2", &[("latency_p99_ms", 80.0), ("cost_usd", 0.03)]),
    );
    runtime.seed_success(
        "exec-run-candidate-3",
        output_with_intents(
            "candidate-3",
            &[("latency_p99_ms", 70.0), ("cost_usd", 0.02)],
            vec![proposal_intent(
                "intent-search-child",
                CommunicationIntentAudience::Leader,
                "refine with jitter",
                Some("intent-search-parent"),
            )],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-4",
        output("candidate-4", &[("latency_p99_ms", 72.0), ("cost_usd", 0.025)]),
    );

    let store = FsExecutionStore::new(store_dir.clone());
    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store,
    );
    let execution = service
        .run_to_completion(strategy_spec("search"))
        .expect("run execution");

    let store = FsExecutionStore::new(store_dir);
    let intents = store.load_intents(&execution.execution_id).expect("load intents");
    let inbox = store
        .load_inbox_snapshot(&execution.execution_id, 1, "candidate-1")
        .expect("load iteration-1 inbox");
    let child = intents
        .iter()
        .find(|intent| intent.intent_id == "intent-search-child")
        .expect("child intent");

    assert_eq!(intents.len(), 2);
    assert_eq!(child.caused_by.as_deref(), Some("intent-search-parent"));
    assert!(inbox
        .entries
        .iter()
        .any(|entry| entry.intent_id == "intent-search-parent"));
}

fn run_mode_to_completion(
    mode: &str,
    store_dir: std::path::PathBuf,
) -> (
    void_control::orchestration::Execution,
    FsExecutionStore,
    void_control::orchestration::ExecutionSnapshot,
) {
    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        output("candidate-1", &[("latency_p99_ms", 95.0), ("cost_usd", 0.05)]),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        output("candidate-2", &[("latency_p99_ms", 80.0), ("cost_usd", 0.03)]),
    );
    runtime.seed_success(
        "exec-run-candidate-3",
        output("candidate-3", &[("latency_p99_ms", 70.0), ("cost_usd", 0.02)]),
    );
    runtime.seed_success(
        "exec-run-candidate-4",
        output("candidate-4", &[("latency_p99_ms", 72.0), ("cost_usd", 0.025)]),
    );

    let store = FsExecutionStore::new(store_dir.clone());
    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store,
    );
    let execution = service
        .run_to_completion(strategy_spec(mode))
        .expect("run execution");

    let store = FsExecutionStore::new(store_dir);
    let snapshot = store
        .load_execution(&execution.execution_id)
        .expect("load execution snapshot");
    (execution, store, snapshot)
}

fn run_mode_with_all_failures(
    mode: &str,
    store_dir: std::path::PathBuf,
) -> (
    void_control::orchestration::Execution,
    FsExecutionStore,
    void_control::orchestration::ExecutionSnapshot,
) {
    let mut runtime = MockRuntime::new();
    runtime.seed_failure("exec-run-candidate-1");
    runtime.seed_failure("exec-run-candidate-2");

    let store = FsExecutionStore::new(store_dir.clone());
    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store,
    );
    let execution = service
        .run_to_completion(failing_strategy_spec(mode))
        .expect("run execution");

    let store = FsExecutionStore::new(store_dir);
    let snapshot = store
        .load_execution(&execution.execution_id)
        .expect("load execution snapshot");
    (execution, store, snapshot)
}

fn strategy_spec(mode: &str) -> ExecutionSpec {
    ExecutionSpec {
        mode: mode.to_string(),
        goal: "optimize latency".to_string(),
        workflow: void_control::orchestration::WorkflowTemplateRef {
            template: "fixtures/sample.vbrun".to_string(),
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

fn failing_strategy_spec(mode: &str) -> ExecutionSpec {
    let mut spec = strategy_spec(mode);
    spec.policy.budget.max_iterations = Some(1);
    spec
}

fn assert_event_counts(
    mode: &str,
    events: &[void_control::orchestration::ControlEventEnvelope],
    expected: &[(ControlEventType, usize)],
) {
    for (event_type, count) in expected {
        let actual = events
            .iter()
            .filter(|event| event.event_type == *event_type)
            .count();
        assert_eq!(actual, *count, "{mode} {:?}", event_type);
    }
}

fn output(candidate_id: &str, metrics: &[(&str, f64)]) -> CandidateOutput {
    CandidateOutput::new(
        candidate_id.to_string(),
        true,
        metrics.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
    )
}

#[cfg(feature = "serde")]
fn output_with_intents(
    candidate_id: &str,
    metrics: &[(&str, f64)],
    intents: Vec<CommunicationIntent>,
) -> CandidateOutput {
    output(candidate_id, metrics).with_intents(intents)
}

#[cfg(feature = "serde")]
fn proposal_intent(
    intent_id: &str,
    audience: CommunicationIntentAudience,
    summary_text: &str,
    caused_by: Option<&str>,
) -> CommunicationIntent {
    CommunicationIntent {
        intent_id: intent_id.to_string(),
        from_candidate_id: "placeholder".to_string(),
        iteration: 0,
        kind: CommunicationIntentKind::Proposal,
        audience,
        payload: serde_json::json!({
            "summary_text": summary_text,
            "strategy_hint": "message-box-test",
        }),
        priority: CommunicationIntentPriority::Normal,
        ttl_iterations: 1,
        caused_by: caused_by.map(str::to_string),
        context: None,
    }
}

fn temp_store_dir(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("void-control-strategy-{label}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
