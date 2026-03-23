use std::collections::BTreeMap;

use void_control::orchestration::{
    CandidateInbox, CandidateOutput, ConvergencePolicy, ExecutionAccumulator,
    IterationEvaluation, MessageStats, MetricDirection, ScoringConfig, StopReason,
    SwarmStrategy, VariationConfig, VariationProposal, VariationSelection, WeightedMetric,
    score_iteration,
};

#[test]
fn weighted_metrics_normalizes_within_iteration() {
    let scores = score_iteration(
        &scoring_config(),
        &[
            candidate_output("cand-a", true, &[("latency_p99_ms", 100.0), ("cost_usd", 0.02)]),
            candidate_output("cand-b", true, &[("latency_p99_ms", 200.0), ("cost_usd", 0.05)]),
        ],
    );

    assert!(scores[0].score > scores[1].score);
}

#[test]
fn failed_candidate_scores_zero() {
    let scores = score_iteration(
        &scoring_config(),
        &[candidate_output("cand-fail", false, &[("latency_p99_ms", 100.0)])],
    );

    assert_eq!(scores[0].score, 0.0);
    assert!(!scores[0].pass);
}

#[test]
fn best_result_uses_tie_breaking_after_score() {
    let scores = score_iteration(
        &scoring_config(),
        &[
            candidate_output("cand-a", true, &[("latency_p99_ms", 100.0), ("cost_usd", 0.05)]),
            candidate_output("cand-b", true, &[("latency_p99_ms", 100.0), ("cost_usd", 0.03)]),
        ],
    );

    assert_eq!(scores[0].candidate_id, "cand-b");
}

#[test]
fn parameter_space_random_respects_candidates_per_iteration() {
    let proposals = VariationConfig::parameter_space(
        2,
        VariationSelection::Random,
        BTreeMap::from([(
            "sandbox.env.CONCURRENCY".to_string(),
            vec!["2".to_string(), "4".to_string(), "8".to_string()],
        )]),
    )
    .generate(&ExecutionAccumulator::default());

    assert_eq!(proposals.len(), 2);
}

#[test]
fn parameter_space_sequential_preserves_order() {
    let proposals = VariationConfig::parameter_space(
        2,
        VariationSelection::Sequential,
        BTreeMap::from([(
            "sandbox.env.CONCURRENCY".to_string(),
            vec!["2".to_string(), "4".to_string(), "8".to_string()],
        )]),
    )
    .generate(&ExecutionAccumulator::default());

    assert_eq!(proposals[0].overrides["sandbox.env.CONCURRENCY"], "2");
    assert_eq!(proposals[1].overrides["sandbox.env.CONCURRENCY"], "4");
}

#[test]
fn explicit_variation_cycles_through_overrides() {
    let mut accumulator = ExecutionAccumulator::default();
    accumulator.scoring_history_len = 1;
    let proposals = VariationConfig::explicit(
        2,
        vec![
            proposal(&[("agent.prompt", "first")]),
            proposal(&[("agent.prompt", "second")]),
            proposal(&[("agent.prompt", "third")]),
        ],
    )
    .generate(&accumulator);

    assert_eq!(proposals[0].overrides["agent.prompt"], "second");
    assert_eq!(proposals[1].overrides["agent.prompt"], "third");
}

#[test]
fn leader_directed_proposals_are_validated_before_use() {
    let mut accumulator = ExecutionAccumulator::default();
    accumulator.leader_proposals = vec![
        proposal(&[("sandbox.env.CONCURRENCY", "2")]),
        VariationProposal {
            overrides: BTreeMap::new(),
        },
    ];

    let proposals = VariationConfig::leader_directed(2).generate(&accumulator);

    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].overrides["sandbox.env.CONCURRENCY"], "2");
}

#[test]
fn signal_reactive_proposals_are_generated_from_planner_output() {
    let mut accumulator = ExecutionAccumulator::default();
    accumulator.leader_proposals = vec![
        proposal(&[("sandbox.env.CONCURRENCY", "2")]),
        VariationProposal {
            overrides: BTreeMap::new(),
        },
    ];

    let proposals = VariationConfig::signal_reactive(2).generate(&accumulator);

    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].overrides["sandbox.env.CONCURRENCY"], "2");
}

#[test]
fn swarm_plans_candidates_from_variation_source() {
    let strategy = SwarmStrategy::new(
        VariationConfig::explicit(
            2,
            vec![
                proposal(&[("agent.prompt", "first")]),
                proposal(&[("agent.prompt", "second")]),
            ],
        ),
        scoring_config(),
        ConvergencePolicy::default(),
    );

    let candidates = strategy.plan_candidates(
        &ExecutionAccumulator::default(),
        &[CandidateInbox::new("candidate-1"), CandidateInbox::new("candidate-2")],
        None,
    );

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].overrides["agent.prompt"], "first");
}

#[test]
fn swarm_reduces_breadth_when_broadcast_and_delivery_failures_raise_convergence_pressure() {
    let strategy = SwarmStrategy::new(
        VariationConfig::explicit(
            4,
            vec![
                proposal(&[("agent.prompt", "first")]),
                proposal(&[("agent.prompt", "second")]),
                proposal(&[("agent.prompt", "third")]),
                proposal(&[("agent.prompt", "fourth")]),
            ],
        ),
        scoring_config(),
        ConvergencePolicy::default(),
    );

    let candidates = strategy.plan_candidates(
        &ExecutionAccumulator::default(),
        &[
            CandidateInbox::new("candidate-1"),
            CandidateInbox::new("candidate-2"),
            CandidateInbox::new("candidate-3"),
            CandidateInbox::new("candidate-4"),
        ],
        Some(&MessageStats {
            iteration: 1,
            total_messages: 4,
            leader_messages: 0,
            broadcast_messages: 3,
            proposal_count: 0,
            signal_count: 1,
            evaluation_count: 0,
            high_priority_count: 0,
            normal_priority_count: 4,
            low_priority_count: 0,
            delivered_count: 2,
            dropped_count: 1,
            expired_count: 1,
            unique_sources: 1,
            unique_intent_count: 4,
        }),
    );

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].overrides["agent.prompt"], "first");
    assert_eq!(candidates[1].overrides["agent.prompt"], "second");
}

#[test]
fn swarm_preserves_full_breadth_when_proposals_arrive_from_multiple_sources() {
    let strategy = SwarmStrategy::new(
        VariationConfig::explicit(
            4,
            vec![
                proposal(&[("agent.prompt", "first")]),
                proposal(&[("agent.prompt", "second")]),
                proposal(&[("agent.prompt", "third")]),
                proposal(&[("agent.prompt", "fourth")]),
            ],
        ),
        scoring_config(),
        ConvergencePolicy::default(),
    );

    let candidates = strategy.plan_candidates(
        &ExecutionAccumulator::default(),
        &[
            CandidateInbox::new("candidate-1"),
            CandidateInbox::new("candidate-2"),
            CandidateInbox::new("candidate-3"),
            CandidateInbox::new("candidate-4"),
        ],
        Some(&MessageStats {
            iteration: 1,
            total_messages: 4,
            leader_messages: 1,
            broadcast_messages: 1,
            proposal_count: 3,
            signal_count: 1,
            evaluation_count: 0,
            high_priority_count: 1,
            normal_priority_count: 3,
            low_priority_count: 0,
            delivered_count: 4,
            dropped_count: 0,
            expired_count: 0,
            unique_sources: 3,
            unique_intent_count: 4,
        }),
    );

    assert_eq!(candidates.len(), 4);
}

#[test]
fn swarm_keeps_legacy_leader_directed_planning_unbiased_by_message_stats() {
    let strategy = SwarmStrategy::new(
        VariationConfig::leader_directed(3),
        scoring_config(),
        ConvergencePolicy::default(),
    );
    let mut accumulator = ExecutionAccumulator::default();
    accumulator.leader_proposals = vec![
        proposal(&[("agent.prompt", "first")]),
        proposal(&[("agent.prompt", "second")]),
        proposal(&[("agent.prompt", "third")]),
    ];

    let candidates = strategy.plan_candidates(
        &accumulator,
        &[
            CandidateInbox::new("candidate-1"),
            CandidateInbox::new("candidate-2"),
            CandidateInbox::new("candidate-3"),
        ],
        Some(&MessageStats {
            iteration: 1,
            total_messages: 3,
            leader_messages: 0,
            broadcast_messages: 3,
            proposal_count: 0,
            signal_count: 0,
            evaluation_count: 0,
            high_priority_count: 0,
            normal_priority_count: 3,
            low_priority_count: 0,
            delivered_count: 1,
            dropped_count: 1,
            expired_count: 1,
            unique_sources: 1,
            unique_intent_count: 3,
        }),
    );

    assert_eq!(candidates.len(), 3);
    assert_eq!(candidates[0].overrides["agent.prompt"], "first");
    assert_eq!(candidates[1].overrides["agent.prompt"], "second");
    assert_eq!(candidates[2].overrides["agent.prompt"], "third");
}

#[test]
fn swarm_should_stop_on_threshold() {
    let strategy = SwarmStrategy::new(
        VariationConfig::explicit(1, vec![proposal(&[("agent.prompt", "only")])]),
        scoring_config(),
        ConvergencePolicy {
            strategy: "threshold".to_string(),
            min_score: Some(0.8),
            max_iterations_without_improvement: None,
        },
    );

    let stop = strategy.should_stop(
        &ExecutionAccumulator::default(),
        &IterationEvaluation {
            ranked_candidates: score_iteration(
                &scoring_config(),
                &[candidate_output(
                    "cand-a",
                    true,
                    &[("latency_p99_ms", 100.0), ("cost_usd", 0.02)],
                )],
            ),
        },
    );

    assert_eq!(stop, Some(StopReason::ConvergenceThreshold));
}

#[test]
fn swarm_should_stop_on_plateau() {
    let strategy = SwarmStrategy::new(
        VariationConfig::explicit(1, vec![proposal(&[("agent.prompt", "only")])]),
        scoring_config(),
        ConvergencePolicy {
            strategy: "plateau".to_string(),
            min_score: None,
            max_iterations_without_improvement: Some(2),
        },
    );
    let mut accumulator = ExecutionAccumulator::default();
    accumulator.iterations_without_improvement = 2;

    let stop = strategy.should_stop(
        &accumulator,
        &IterationEvaluation {
            ranked_candidates: vec![],
        },
    );

    assert_eq!(stop, Some(StopReason::ConvergencePlateau));
}

#[test]
fn swarm_reduce_updates_best_result_and_failure_counts() {
    let strategy = SwarmStrategy::new(
        VariationConfig::explicit(1, vec![proposal(&[("agent.prompt", "only")])]),
        scoring_config(),
        ConvergencePolicy::default(),
    );

    let next = strategy.reduce(
        ExecutionAccumulator::default(),
        IterationEvaluation {
            ranked_candidates: score_iteration(
                &scoring_config(),
                &[
                    candidate_output(
                        "cand-a",
                        true,
                        &[("latency_p99_ms", 100.0), ("cost_usd", 0.02)],
                    ),
                    candidate_output("cand-b", false, &[("latency_p99_ms", 200.0)]),
                ],
            ),
        },
    );

    assert_eq!(next.best_candidate_id.as_deref(), Some("cand-a"));
    assert_eq!(next.failure_counts.total_candidate_failures, 1);
}

fn scoring_config() -> ScoringConfig {
    ScoringConfig {
        metrics: vec![
            WeightedMetric {
                name: "latency_p99_ms".to_string(),
                weight: 0.6,
                direction: MetricDirection::Minimize,
            },
            WeightedMetric {
                name: "cost_usd".to_string(),
                weight: 0.4,
                direction: MetricDirection::Minimize,
            },
        ],
        pass_threshold: 0.7,
        tie_break_metric: "cost_usd".to_string(),
    }
}

fn candidate_output(
    candidate_id: &str,
    succeeded: bool,
    metrics: &[(&str, f64)],
) -> CandidateOutput {
    CandidateOutput::new(
        candidate_id.to_string(),
        succeeded,
        metrics
            .iter()
            .map(|(name, value)| (name.to_string(), *value))
            .collect(),
    )
}

fn proposal(values: &[(&str, &str)]) -> VariationProposal {
    VariationProposal {
        overrides: values
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
    }
}
