use std::collections::BTreeMap;

use void_control::orchestration::{
    CandidateInbox, CandidateOutput, CandidateSpec, ConvergencePolicy, ExecutionAccumulator,
    IterationEvaluation, MessageStats, MetricDirection, ScoringConfig, SearchStrategy, StopReason,
    VariationConfig, VariationProposal, VariationSelection, WeightedMetric,
};

#[test]
fn search_bootstraps_when_no_seed_exists() {
    let strategy = SearchStrategy::new(
        VariationConfig::parameter_space(
            4,
            VariationSelection::Sequential,
            BTreeMap::from([(
                "sandbox.env.CONCURRENCY".to_string(),
                vec![
                    "2".to_string(),
                    "4".to_string(),
                    "8".to_string(),
                    "16".to_string(),
                ],
            )]),
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
        None,
    );

    assert!(!candidates.is_empty());
    assert!(candidates.len() < 4);
}

#[test]
fn search_refines_around_explicit_incumbent() {
    let strategy = SearchStrategy::new(
        VariationConfig::explicit(
            2,
            vec![
                proposal(&[("agent.prompt", "baseline")]),
                proposal(&[("agent.prompt", "v1")]),
                proposal(&[("agent.prompt", "v2")]),
            ],
        ),
        scoring_config(),
        ConvergencePolicy::default(),
    );
    let accumulator = ExecutionAccumulator {
        best_candidate_overrides: BTreeMap::from([("agent.prompt".to_string(), "v1".to_string())]),
        ..ExecutionAccumulator::default()
    };

    let candidates = strategy.plan_candidates(
        &accumulator,
        &[
            CandidateInbox::new("candidate-1"),
            CandidateInbox::new("candidate-2"),
        ],
        None,
    );

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].overrides["agent.prompt"], "baseline");
    assert_eq!(candidates[1].overrides["agent.prompt"], "v2");
}

#[test]
fn search_avoids_explored_signatures() {
    let strategy = SearchStrategy::new(
        VariationConfig::explicit(
            2,
            vec![
                proposal(&[("agent.prompt", "baseline")]),
                proposal(&[("agent.prompt", "v1")]),
                proposal(&[("agent.prompt", "v2")]),
            ],
        ),
        scoring_config(),
        ConvergencePolicy::default(),
    );
    let accumulator = ExecutionAccumulator {
        best_candidate_overrides: BTreeMap::from([("agent.prompt".to_string(), "v1".to_string())]),
        explored_signatures: vec!["agent.prompt=baseline".to_string()],
        ..ExecutionAccumulator::default()
    };

    let candidates = strategy.plan_candidates(
        &accumulator,
        &[
            CandidateInbox::new("candidate-1"),
            CandidateInbox::new("candidate-2"),
        ],
        None,
    );

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].overrides["agent.prompt"], "v2");
}

#[test]
fn search_reduce_updates_incumbent_phase_and_signatures() {
    let strategy = SearchStrategy::new(
        VariationConfig::explicit(
            2,
            vec![
                proposal(&[("agent.prompt", "baseline")]),
                proposal(&[("agent.prompt", "v1")]),
            ],
        ),
        scoring_config(),
        ConvergencePolicy::default(),
    );

    let planned_candidates = vec![
        CandidateSpec {
            candidate_id: "candidate-1".to_string(),
            overrides: BTreeMap::from([("agent.prompt".to_string(), "baseline".to_string())]),
        },
        CandidateSpec {
            candidate_id: "candidate-2".to_string(),
            overrides: BTreeMap::from([("agent.prompt".to_string(), "v1".to_string())]),
        },
    ];

    let next = strategy.reduce(
        ExecutionAccumulator::default(),
        &planned_candidates,
        IterationEvaluation {
            ranked_candidates: void_control::orchestration::score_iteration(
                &scoring_config(),
                &[
                    candidate_output("candidate-1", true, &[("latency_p99_ms", 100.0)]),
                    candidate_output("candidate-2", true, &[("latency_p99_ms", 80.0)]),
                ],
            ),
        },
    );

    assert_eq!(next.best_candidate_id.as_deref(), Some("candidate-2"));
    assert_eq!(
        next.best_candidate_overrides
            .get("agent.prompt")
            .map(String::as_str),
        Some("v1")
    );
    assert_eq!(next.search_phase.as_deref(), Some("refine"));
    assert!(next
        .explored_signatures
        .contains(&"agent.prompt=baseline".to_string()));
    assert!(next
        .explored_signatures
        .contains(&"agent.prompt=v1".to_string()));
}

#[test]
fn search_stops_when_no_new_neighbors_remain() {
    let strategy = SearchStrategy::new(
        VariationConfig::explicit(
            2,
            vec![
                proposal(&[("agent.prompt", "baseline")]),
                proposal(&[("agent.prompt", "v1")]),
            ],
        ),
        scoring_config(),
        ConvergencePolicy::default(),
    );
    let accumulator = ExecutionAccumulator {
        best_candidate_overrides: BTreeMap::from([(
            "agent.prompt".to_string(),
            "baseline".to_string(),
        )]),
        explored_signatures: vec![
            "agent.prompt=v1".to_string(),
            "agent.prompt=baseline".to_string(),
        ],
        ..ExecutionAccumulator::default()
    };

    let stop = strategy.should_stop(
        &accumulator,
        &IterationEvaluation {
            ranked_candidates: vec![],
        },
    );

    assert_eq!(stop, Some(StopReason::ConvergencePlateau));
}

#[test]
fn search_falls_back_to_incumbent_centered_planning_without_meaningful_stats() {
    let strategy = SearchStrategy::new(
        VariationConfig::explicit(
            2,
            vec![
                proposal(&[("agent.prompt", "baseline")]),
                proposal(&[("agent.prompt", "v1")]),
                proposal(&[("agent.prompt", "v2")]),
                proposal(&[("agent.prompt", "v3")]),
            ],
        ),
        scoring_config(),
        ConvergencePolicy::default(),
    );
    let accumulator = ExecutionAccumulator {
        best_candidate_overrides: BTreeMap::from([("agent.prompt".to_string(), "v1".to_string())]),
        ..ExecutionAccumulator::default()
    };

    let candidates = strategy.plan_candidates(
        &accumulator,
        &[
            CandidateInbox::new("candidate-1"),
            CandidateInbox::new("candidate-2"),
        ],
        Some(&MessageStats::default()),
    );

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].overrides["agent.prompt"], "baseline");
    assert_eq!(candidates[1].overrides["agent.prompt"], "v2");
}

#[test]
fn search_keeps_a_small_exploration_quota_when_signal_pressure_is_high() {
    let strategy = SearchStrategy::new(
        VariationConfig::explicit(
            2,
            vec![
                proposal(&[("agent.prompt", "baseline")]),
                proposal(&[("agent.prompt", "v1")]),
                proposal(&[("agent.prompt", "v2")]),
                proposal(&[("agent.prompt", "v3")]),
            ],
        ),
        scoring_config(),
        ConvergencePolicy::default(),
    );
    let accumulator = ExecutionAccumulator {
        best_candidate_overrides: BTreeMap::from([("agent.prompt".to_string(), "v1".to_string())]),
        ..ExecutionAccumulator::default()
    };

    let candidates = strategy.plan_candidates(
        &accumulator,
        &[
            CandidateInbox::new("candidate-1"),
            CandidateInbox::new("candidate-2"),
        ],
        Some(&MessageStats {
            iteration: 1,
            total_messages: 4,
            leader_messages: 0,
            broadcast_messages: 2,
            proposal_count: 0,
            signal_count: 3,
            evaluation_count: 1,
            high_priority_count: 1,
            normal_priority_count: 3,
            low_priority_count: 0,
            delivered_count: 3,
            dropped_count: 1,
            expired_count: 0,
            unique_sources: 2,
            unique_intent_count: 4,
        }),
    );

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].overrides["agent.prompt"], "baseline");
    assert_eq!(candidates[1].overrides["agent.prompt"], "v3");
}

#[test]
fn search_reduce_uses_the_actual_planned_candidates() {
    let strategy = SearchStrategy::new(
        VariationConfig::explicit(
            2,
            vec![
                proposal(&[("agent.prompt", "baseline")]),
                proposal(&[("agent.prompt", "v1")]),
                proposal(&[("agent.prompt", "v2")]),
            ],
        ),
        scoring_config(),
        ConvergencePolicy::default(),
    );
    let accumulator = ExecutionAccumulator::default();
    let planned_candidates = vec![
        CandidateSpec {
            candidate_id: "candidate-1".to_string(),
            overrides: BTreeMap::from([("agent.prompt".to_string(), "v2".to_string())]),
        },
        CandidateSpec {
            candidate_id: "candidate-2".to_string(),
            overrides: BTreeMap::from([("agent.prompt".to_string(), "baseline".to_string())]),
        },
    ];

    let next = strategy.reduce(
        accumulator,
        &planned_candidates,
        IterationEvaluation {
            ranked_candidates: void_control::orchestration::score_iteration(
                &scoring_config(),
                &[
                    candidate_output("candidate-1", true, &[("latency_p99_ms", 60.0)]),
                    candidate_output("candidate-2", true, &[("latency_p99_ms", 80.0)]),
                ],
            ),
        },
    );

    assert_eq!(
        next.best_candidate_overrides
            .get("agent.prompt")
            .map(String::as_str),
        Some("v2")
    );
}

fn scoring_config() -> ScoringConfig {
    ScoringConfig {
        metrics: vec![WeightedMetric {
            name: "latency_p99_ms".to_string(),
            weight: 1.0,
            direction: MetricDirection::Minimize,
        }],
        pass_threshold: 0.7,
        tie_break_metric: "latency_p99_ms".to_string(),
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

fn proposal(items: &[(&str, &str)]) -> VariationProposal {
    VariationProposal {
        overrides: items
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect(),
    }
}
