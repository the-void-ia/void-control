#![cfg(feature = "serde")]

use std::collections::BTreeMap;

use void_control::orchestration::{
    CandidateOutput, ControlEventType, ExecutionService, ExecutionSpec, ExecutionStatus,
    FsExecutionStore, GlobalConfig, OrchestrationPolicy, VariationConfig, VariationProposal,
    VariationSelection,
};
#[cfg(feature = "serde")]
use void_control::orchestration::{
    CommunicationIntent, CommunicationIntentAudience, CommunicationIntentKind,
    CommunicationIntentPriority, RoutedMessageStatus,
};
use void_control::runtime::MockRuntime;

#[test]
fn swarm_incident_mitigation_explores_distinct_hypotheses_and_finds_best_family() {
    let store_dir = temp_store_dir("swarm-incident");
    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        metrics_output_with_intents(
            "candidate-1",
            115.0,
            0.08,
            0.91,
            vec![scenario_intent(
                "intent-incident-signal",
                CommunicationIntentAudience::Leader,
                "retry raised errors under peak load",
                None,
            )],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        metrics_output_with_intents(
            "candidate-2",
            72.0,
            0.04,
            0.99,
            vec![scenario_intent(
                "intent-incident-broadcast",
                CommunicationIntentAudience::Broadcast,
                "rate limit plus cache fallback stabilized latency",
                None,
            )],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-3",
        metrics_output("candidate-3", 88.0, 0.05, 0.97),
    );
    runtime.seed_success(
        "exec-run-candidate-4",
        metrics_output("candidate-4", 96.0, 0.06, 0.94),
    );
    runtime.seed_success(
        "exec-run-candidate-5",
        metrics_output("candidate-5", 101.0, 0.05, 0.95),
    );
    for idx in 6..=10 {
        runtime.seed_success(
            &format!("exec-run-candidate-{idx}"),
            metrics_output(
                &format!("candidate-{idx}"),
                90.0 + idx as f64,
                0.05,
                0.96,
            ),
        );
    }

    let store = FsExecutionStore::new(store_dir.clone());
    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 8,
        },
        runtime,
        store,
    );
    let execution = service
        .run_to_completion(swarm_incident_message_box_spec())
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
    let best = snapshot
        .candidates
        .iter()
        .filter(|candidate| Some(&candidate.candidate_id) == execution.result_best_candidate_id.as_ref())
        .max_by_key(|candidate| candidate.created_seq)
        .expect("best candidate");

    let explored: Vec<_> = snapshot
        .candidates
        .iter()
        .map(|candidate| candidate.overrides["mitigation.strategy"].clone())
        .collect();

    assert_eq!(execution.status, ExecutionStatus::Completed);
    assert_eq!(execution.completed_iterations, 2);
    assert_eq!(
        best.overrides.get("mitigation.strategy").map(String::as_str),
        Some("rate_limit_cache")
    );
    assert!(explored.starts_with(&[
        "retry".to_string(),
        "rate_limit_cache".to_string(),
        "circuit_breaker".to_string(),
        "queue_buffering".to_string(),
        "reduce_concurrency".to_string(),
    ]));
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
        6
    );
    assert_eq!(inbox_one.entries.len(), 2);
    assert_eq!(inbox_two.entries.len(), 1);
    assert_event_counts(
        &snapshot.events,
        &[
            (ControlEventType::CandidateQueued, 10),
            (ControlEventType::CandidateDispatched, 10),
            (ControlEventType::CandidateOutputCollected, 10),
            (ControlEventType::CandidateScored, 2),
            (ControlEventType::CommunicationIntentEmitted, 2),
            (ControlEventType::MessageRouted, 2),
            (ControlEventType::MessageDelivered, 6),
            (ControlEventType::ExecutionCompleted, 1),
        ],
    );
}

#[test]
fn swarm_prompt_optimization_finds_best_style_cluster() {
    let store_dir = temp_store_dir("swarm-prompt");
    let mut runtime = MockRuntime::new();
    runtime.seed_success("exec-run-candidate-1", prompt_output("candidate-1", 0.74, 0.70));
    runtime.seed_success("exec-run-candidate-2", prompt_output("candidate-2", 0.89, 0.92));
    runtime.seed_success("exec-run-candidate-3", prompt_output("candidate-3", 0.78, 0.76));
    runtime.seed_success("exec-run-candidate-4", prompt_output("candidate-4", 0.69, 0.65));
    runtime.seed_success("exec-run-candidate-5", prompt_output("candidate-5", 0.81, 0.83));
    runtime.seed_success("exec-run-candidate-6", prompt_output("candidate-6", 0.76, 0.72));
    runtime.seed_success("exec-run-candidate-7", prompt_output("candidate-7", 0.72, 0.90));
    runtime.seed_success("exec-run-candidate-8", prompt_output("candidate-8", 0.96, 0.97));

    let store = FsExecutionStore::new(store_dir.clone());
    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 8,
        },
        runtime,
        store,
    );
    let execution = service
        .run_to_completion(swarm_prompt_spec())
        .expect("run execution");

    let snapshot = FsExecutionStore::new(store_dir)
        .load_execution(&execution.execution_id)
        .expect("load execution");
    let best = snapshot
        .candidates
        .iter()
        .find(|candidate| Some(&candidate.candidate_id) == execution.result_best_candidate_id.as_ref())
        .expect("best candidate");

    assert_eq!(execution.status, ExecutionStatus::Completed);
    assert_eq!(
        best.overrides.get("agent.prompt").map(String::as_str),
        Some("hybrid_friendly_concise_structured")
    );
    assert_eq!(snapshot.candidates.len(), 8);
    assert_event_counts(
        &snapshot.events,
        &[
            (ControlEventType::CandidateQueued, 8),
            (ControlEventType::CandidateDispatched, 8),
            (ControlEventType::CandidateOutputCollected, 8),
            (ControlEventType::ExecutionCompleted, 1),
        ],
    );
}

#[test]
fn search_rate_limit_tuning_refines_known_good_direction() {
    let store_dir = temp_store_dir("search-rate-limit");
    let mut runtime = MockRuntime::new();
    runtime.seed_success(
        "exec-run-candidate-1",
        metrics_output_with_intents(
            "candidate-1",
            95.0,
            0.06,
            0.96,
            vec![scenario_intent(
                "intent-search-parent",
                CommunicationIntentAudience::Leader,
                "baseline rate limit looks promising",
                None,
            )],
        ),
    );
    runtime.seed_success(
        "exec-run-candidate-2",
        metrics_output("candidate-2", 82.0, 0.05, 0.98),
    );
    runtime.seed_success(
        "exec-run-candidate-3",
        metrics_output_with_intents(
            "candidate-3",
            70.0,
            0.05,
            0.99,
            vec![scenario_intent(
                "intent-search-child",
                CommunicationIntentAudience::Leader,
                "refine with adaptive jitter",
                Some("intent-search-parent"),
            )],
        ),
    );

    let store = FsExecutionStore::new(store_dir.clone());
    ExecutionService::<MockRuntime>::submit_execution(
        &store,
        "exec-search-rate-limit",
        &search_rate_limit_spec(),
    )
    .expect("submit");

    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store,
    );
    service
        .plan_execution("exec-search-rate-limit")
        .expect("plan execution");

    for _ in 0..6 {
        let execution = service
            .dispatch_execution_once("exec-search-rate-limit")
            .expect("dispatch");
        if execution.status == ExecutionStatus::Completed {
            break;
        }
    }

    let store = FsExecutionStore::new(store_dir);
    let snapshot = store
        .load_execution("exec-search-rate-limit")
        .expect("load execution");
    let intents = store
        .load_intents("exec-search-rate-limit")
        .expect("load intents");
    let messages = store
        .load_routed_messages("exec-search-rate-limit")
        .expect("load routed messages");
    let inbox = store
        .load_inbox_snapshot("exec-search-rate-limit", 1, "candidate-1")
        .expect("load refinement inbox");
    let iter0_thresholds: Vec<_> = snapshot
        .candidates
        .iter()
        .filter(|candidate| candidate.iteration == 0)
        .map(|candidate| candidate.overrides["rate_limit.threshold"].clone())
        .collect();
    let iter1_thresholds: Vec<_> = snapshot
        .candidates
        .iter()
        .filter(|candidate| candidate.iteration == 1)
        .map(|candidate| candidate.overrides["rate_limit.threshold"].clone())
        .collect();

    assert_eq!(snapshot.execution.status, ExecutionStatus::Completed);
    assert_eq!(iter0_thresholds, vec!["80".to_string(), "100".to_string()]);
    assert_eq!(iter1_thresholds, vec!["120".to_string()]);
    assert_eq!(snapshot.accumulator.search_phase.as_deref(), Some("refine"));
    assert_eq!(intents.len(), 2);
    assert_eq!(
        intents
            .iter()
            .find(|intent| intent.intent_id == "intent-search-child")
            .and_then(|intent| intent.caused_by.as_deref()),
        Some("intent-search-parent")
    );
    assert!(messages.iter().any(|message| {
        message.intent_id == "intent-search-parent"
            && message.status == RoutedMessageStatus::Delivered
            && message.delivery_iteration == 1
    }));
    assert!(inbox
        .entries
        .iter()
        .any(|entry| entry.intent_id == "intent-search-parent"));
    assert_event_counts(
        &snapshot.events,
        &[
            (ControlEventType::CandidateQueued, 3),
            (ControlEventType::CandidateDispatched, 3),
            (ControlEventType::CandidateScored, 2),
            (ControlEventType::CommunicationIntentEmitted, 2),
            (ControlEventType::MessageRouted, 2),
            (ControlEventType::MessageDelivered, 1),
            (ControlEventType::ExecutionCompleted, 1),
        ],
    );
}

#[test]
fn search_pipeline_optimization_refines_known_bottleneck_config() {
    let store_dir = temp_store_dir("search-pipeline");
    let mut runtime = MockRuntime::new();
    runtime.seed_success("exec-run-candidate-1", pipeline_output("candidate-1", 0.72, 0.78));
    runtime.seed_success("exec-run-candidate-2", pipeline_output("candidate-2", 0.84, 0.86));
    runtime.seed_success("exec-run-candidate-3", pipeline_output("candidate-3", 0.93, 0.95));
    runtime.seed_success("exec-run-candidate-4", pipeline_output("candidate-4", 0.80, 0.82));

    let store = FsExecutionStore::new(store_dir.clone());
    ExecutionService::<MockRuntime>::submit_execution(
        &store,
        "exec-search-pipeline",
        &search_pipeline_spec(),
    )
    .expect("submit");

    let mut service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 2,
        },
        runtime,
        store,
    );
    service
        .plan_execution("exec-search-pipeline")
        .expect("plan execution");

    for _ in 0..6 {
        let execution = service
            .dispatch_execution_once("exec-search-pipeline")
            .expect("dispatch");
        if execution.status == ExecutionStatus::Completed {
            break;
        }
    }

    let snapshot = FsExecutionStore::new(store_dir)
        .load_execution("exec-search-pipeline")
        .expect("load execution");
    let mut iter1_prompts: Vec<_> = snapshot
        .candidates
        .iter()
        .filter(|candidate| candidate.iteration == 1)
        .map(|candidate| candidate.overrides["transform.config"].clone())
        .collect();
    iter1_prompts.sort();
    let best = snapshot
        .candidates
        .iter()
        .filter(|candidate| Some(&candidate.candidate_id) == snapshot.execution.result_best_candidate_id.as_ref())
        .max_by_key(|candidate| candidate.created_seq)
        .expect("best candidate");

    assert_eq!(snapshot.execution.status, ExecutionStatus::Completed);
    assert_eq!(
        iter1_prompts,
        vec![
            "batch1024_parallel2".to_string(),
            "batch512_parallel4_streaming".to_string(),
        ]
    );
    assert_eq!(
        best.overrides.get("transform.config").map(String::as_str),
        Some("batch512_parallel4_streaming")
    );
    assert_eq!(snapshot.accumulator.search_phase.as_deref(), Some("refine"));
}

fn swarm_incident_spec() -> ExecutionSpec {
    ExecutionSpec {
        mode: "swarm".to_string(),
        goal: "mitigate latency and errors".to_string(),
        workflow: workflow(),
        policy: swarm_policy(1),
        evaluation: infra_evaluation(),
        variation: VariationConfig::explicit(
            5,
            vec![
                proposal(&[("mitigation.strategy", "retry")]),
                proposal(&[("mitigation.strategy", "rate_limit_cache")]),
                proposal(&[("mitigation.strategy", "circuit_breaker")]),
                proposal(&[("mitigation.strategy", "queue_buffering")]),
                proposal(&[("mitigation.strategy", "reduce_concurrency")]),
            ],
        ),
        swarm: true,
    }
}

fn swarm_incident_message_box_spec() -> ExecutionSpec {
    let mut spec = swarm_incident_spec();
    spec.policy = swarm_policy(2);
    spec
}

fn swarm_prompt_spec() -> ExecutionSpec {
    ExecutionSpec {
        mode: "swarm".to_string(),
        goal: "improve support agent prompt quality".to_string(),
        workflow: workflow(),
        policy: swarm_policy(1),
        evaluation: prompt_evaluation(),
        variation: VariationConfig::explicit(
            8,
            vec![
                proposal(&[("agent.prompt", "formal")]),
                proposal(&[("agent.prompt", "friendly_concise_structured")]),
                proposal(&[("agent.prompt", "concise")]),
                proposal(&[("agent.prompt", "verbose")]),
                proposal(&[("agent.prompt", "step_by_step")]),
                proposal(&[("agent.prompt", "empathetic")]),
                proposal(&[("agent.prompt", "strict_policy")]),
                proposal(&[("agent.prompt", "hybrid_friendly_concise_structured")]),
            ],
        ),
        swarm: true,
    }
}

fn search_rate_limit_spec() -> ExecutionSpec {
    ExecutionSpec {
        mode: "search".to_string(),
        goal: "tune rate limiting".to_string(),
        workflow: workflow(),
        policy: search_policy(2),
        evaluation: infra_evaluation(),
        variation: VariationConfig::parameter_space(
            4,
            VariationSelection::Sequential,
            BTreeMap::from([(
                "rate_limit.threshold".to_string(),
                vec![
                    "80".to_string(),
                    "100".to_string(),
                    "120".to_string(),
                    "140".to_string(),
                ],
            )]),
        ),
        swarm: true,
    }
}

fn search_pipeline_spec() -> ExecutionSpec {
    ExecutionSpec {
        mode: "search".to_string(),
        goal: "tune transform bottleneck".to_string(),
        workflow: workflow(),
        policy: search_policy(2),
        evaluation: pipeline_evaluation(),
        variation: VariationConfig::explicit(
            3,
            vec![
                proposal(&[("transform.config", "current_transform")]),
                proposal(&[("transform.config", "batch256_parallel4")]),
                proposal(&[("transform.config", "batch512_parallel4_streaming")]),
                proposal(&[("transform.config", "batch1024_parallel2")]),
            ],
        ),
        swarm: true,
    }
}

fn workflow() -> void_control::orchestration::WorkflowTemplateRef {
    void_control::orchestration::WorkflowTemplateRef {
        template: "fixtures/sample.vbrun".to_string(),
    }
}

fn swarm_policy(max_iterations: u32) -> OrchestrationPolicy {
    base_policy(max_iterations, 10)
}

fn search_policy(max_iterations: u32) -> OrchestrationPolicy {
    base_policy(max_iterations, 10)
}

fn base_policy(max_iterations: u32, max_failures: u32) -> OrchestrationPolicy {
    OrchestrationPolicy {
        budget: void_control::orchestration::BudgetPolicy {
            max_iterations: Some(max_iterations),
            max_child_runs: None,
            max_wall_clock_secs: Some(60),
            max_cost_usd_millis: None,
        },
        concurrency: void_control::orchestration::ConcurrencyPolicy {
            max_concurrent_candidates: 8,
        },
        convergence: void_control::orchestration::ConvergencePolicy {
            strategy: "exhaustive".to_string(),
            min_score: None,
            max_iterations_without_improvement: None,
        },
        max_candidate_failures_per_iteration: max_failures,
        missing_output_policy: "mark_failed".to_string(),
        iteration_failure_policy: "fail_execution".to_string(),
    }
}

fn infra_evaluation() -> void_control::orchestration::EvaluationConfig {
    void_control::orchestration::EvaluationConfig {
        scoring_type: "weighted_metrics".to_string(),
        weights: BTreeMap::from([
            ("latency_p99_ms".to_string(), -0.5),
            ("cost_usd".to_string(), -0.1),
            ("success_rate".to_string(), 0.4),
        ]),
        pass_threshold: Some(0.7),
        ranking: "highest_score".to_string(),
        tie_breaking: "cost_usd".to_string(),
    }
}

fn prompt_evaluation() -> void_control::orchestration::EvaluationConfig {
    void_control::orchestration::EvaluationConfig {
        scoring_type: "weighted_metrics".to_string(),
        weights: BTreeMap::from([
            ("quality_score".to_string(), 0.6),
            ("policy_score".to_string(), 0.4),
        ]),
        pass_threshold: Some(0.7),
        ranking: "highest_score".to_string(),
        tie_breaking: "quality_score".to_string(),
    }
}

fn pipeline_evaluation() -> void_control::orchestration::EvaluationConfig {
    void_control::orchestration::EvaluationConfig {
        scoring_type: "weighted_metrics".to_string(),
        weights: BTreeMap::from([
            ("throughput".to_string(), 0.6),
            ("stability".to_string(), 0.4),
        ]),
        pass_threshold: Some(0.7),
        ranking: "highest_score".to_string(),
        tie_breaking: "throughput".to_string(),
    }
}

fn proposal(items: &[(&str, &str)]) -> VariationProposal {
    VariationProposal {
        overrides: items
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect(),
    }
}

fn metrics_output(candidate_id: &str, latency_p99_ms: f64, cost_usd: f64, success_rate: f64) -> CandidateOutput {
    CandidateOutput::new(
        candidate_id.to_string(),
        true,
        BTreeMap::from([
            ("latency_p99_ms".to_string(), latency_p99_ms),
            ("cost_usd".to_string(), cost_usd),
            ("success_rate".to_string(), success_rate),
        ]),
    )
}

#[cfg(feature = "serde")]
fn metrics_output_with_intents(
    candidate_id: &str,
    latency_p99_ms: f64,
    cost_usd: f64,
    success_rate: f64,
    intents: Vec<CommunicationIntent>,
) -> CandidateOutput {
    metrics_output(candidate_id, latency_p99_ms, cost_usd, success_rate).with_intents(intents)
}

fn prompt_output(candidate_id: &str, quality_score: f64, policy_score: f64) -> CandidateOutput {
    CandidateOutput::new(
        candidate_id.to_string(),
        true,
        BTreeMap::from([
            ("quality_score".to_string(), quality_score),
            ("policy_score".to_string(), policy_score),
        ]),
    )
}

fn pipeline_output(candidate_id: &str, throughput: f64, stability: f64) -> CandidateOutput {
    CandidateOutput::new(
        candidate_id.to_string(),
        true,
        BTreeMap::from([
            ("throughput".to_string(), throughput),
            ("stability".to_string(), stability),
        ]),
    )
}

#[cfg(feature = "serde")]
fn scenario_intent(
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
            "strategy_hint": "scenario",
        }),
        priority: CommunicationIntentPriority::Normal,
        ttl_iterations: 1,
        caused_by: caused_by.map(str::to_string),
        context: None,
    }
}

fn assert_event_counts(
    events: &[void_control::orchestration::ControlEventEnvelope],
    expected: &[(ControlEventType, usize)],
) {
    for (event_type, count) in expected {
        let actual = events
            .iter()
            .filter(|event| event.event_type == *event_type)
            .count();
        assert_eq!(actual, *count, "{event_type:?}");
    }
}

fn temp_store_dir(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("void-control-scenarios-{label}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
