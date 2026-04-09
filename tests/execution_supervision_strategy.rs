#![cfg(feature = "serde")]

use std::collections::BTreeMap;

use void_control::orchestration::{
    CandidateInbox, CandidateOutput, ExecutionAccumulator, SupervisionReviewPolicy,
    SupervisionStrategy, VariationConfig, VariationProposal, WorkerReviewStatus,
};

#[test]
fn supervision_strategy_plans_workers_from_variation() {
    let strategy = supervision_strategy();

    let planned = strategy.plan_candidates(
        &ExecutionAccumulator::default(),
        &[
            CandidateInbox::new("worker-1"),
            CandidateInbox::new("worker-2"),
        ],
    );

    assert_eq!(planned.len(), 2);
    assert_eq!(planned[0].candidate_id, "worker-1");
    assert_eq!(
        planned[0].overrides.get("role").map(String::as_str),
        Some("researcher")
    );
    assert_eq!(planned[1].candidate_id, "worker-2");
    assert_eq!(
        planned[1].overrides.get("role").map(String::as_str),
        Some("implementer")
    );
}

#[test]
fn supervision_strategy_marks_approved_outputs_for_final_approval() {
    let strategy = supervision_strategy();
    let evaluation = strategy.evaluate(
        &ExecutionAccumulator::default(),
        &[approved_output("worker-1"), approved_output("worker-2")],
    );

    assert_eq!(evaluation.decisions.len(), 2);
    assert_eq!(evaluation.decisions[0].status, WorkerReviewStatus::Approved);
    assert_eq!(evaluation.decisions[1].status, WorkerReviewStatus::Approved);
    assert!(evaluation.final_approval_ready);

    let accumulator = strategy.reduce(ExecutionAccumulator::default(), &evaluation);

    assert_eq!(accumulator.supervision_final_approval, Some(true));
    assert_eq!(
        accumulator.supervision_reviews.get("worker-1"),
        Some(&WorkerReviewStatus::Approved)
    );
}

#[test]
fn supervision_strategy_requests_revision_when_output_is_not_approved() {
    let strategy = supervision_strategy();
    let evaluation = strategy.evaluate(
        &ExecutionAccumulator::default(),
        &[unapproved_output("worker-1")],
    );

    assert_eq!(evaluation.decisions.len(), 1);
    assert_eq!(
        evaluation.decisions[0].status,
        WorkerReviewStatus::RevisionRequested
    );
    assert!(!evaluation.final_approval_ready);

    let accumulator = strategy.reduce(ExecutionAccumulator::default(), &evaluation);

    assert_eq!(accumulator.supervision_final_approval, Some(false));
    assert_eq!(
        accumulator.supervision_revision_rounds.get("worker-1"),
        Some(&1)
    );
}

#[test]
fn supervision_strategy_retries_runtime_failures_when_policy_allows_it() {
    let strategy = supervision_strategy();
    let evaluation = strategy.evaluate(
        &ExecutionAccumulator::default(),
        &[failed_output("worker-1")],
    );

    assert_eq!(evaluation.decisions.len(), 1);
    assert_eq!(
        evaluation.decisions[0].status,
        WorkerReviewStatus::RetryRequested
    );

    let accumulator = strategy.reduce(ExecutionAccumulator::default(), &evaluation);

    assert_eq!(
        accumulator.supervision_revision_rounds.get("worker-1"),
        Some(&1)
    );
}

#[test]
fn supervision_strategy_rejects_after_revision_budget_is_exhausted() {
    let strategy = supervision_strategy();
    let mut accumulator = ExecutionAccumulator::default();
    accumulator
        .supervision_revision_rounds
        .insert("worker-1".to_string(), 2);

    let evaluation = strategy.evaluate(&accumulator, &[unapproved_output("worker-1")]);

    assert_eq!(evaluation.decisions.len(), 1);
    assert_eq!(evaluation.decisions[0].status, WorkerReviewStatus::Rejected);
    assert!(!evaluation.final_approval_ready);
}

fn supervision_strategy() -> SupervisionStrategy {
    SupervisionStrategy::new(
        VariationConfig::explicit(
            2,
            vec![
                VariationProposal {
                    overrides: BTreeMap::from([("role".to_string(), "researcher".to_string())]),
                },
                VariationProposal {
                    overrides: BTreeMap::from([("role".to_string(), "implementer".to_string())]),
                },
            ],
        ),
        SupervisionReviewPolicy {
            max_revision_rounds: 2,
            retry_on_runtime_failure: true,
            require_final_approval: true,
        },
    )
}

fn approved_output(candidate_id: &str) -> CandidateOutput {
    CandidateOutput::new(
        candidate_id,
        true,
        BTreeMap::from([("approved".to_string(), 1.0)]),
    )
}

fn unapproved_output(candidate_id: &str) -> CandidateOutput {
    CandidateOutput::new(
        candidate_id,
        true,
        BTreeMap::from([("approved".to_string(), 0.0)]),
    )
}

fn failed_output(candidate_id: &str) -> CandidateOutput {
    CandidateOutput::new(candidate_id, false, BTreeMap::new())
}
