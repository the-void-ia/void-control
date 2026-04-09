use super::policy::ConvergencePolicy;
use super::spec::SupervisionReviewPolicy;
use super::scoring::{score_iteration, RankedCandidate, ScoringConfig};
use super::types::{
    CandidateInbox, CandidateOutput, CandidateSpec, ExecutionAccumulator, MessageStats,
    WorkerReviewStatus,
};
use super::variation::VariationConfig;

#[derive(Debug, Clone, PartialEq)]
pub struct IterationEvaluation {
    pub ranked_candidates: Vec<RankedCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerReviewDecision {
    pub candidate_id: String,
    pub status: WorkerReviewStatus,
    pub revision_round: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisionEvaluation {
    pub decisions: Vec<WorkerReviewDecision>,
    pub final_approval_ready: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    ConvergenceThreshold,
    ConvergencePlateau,
}

#[derive(Debug, Clone)]
pub struct SwarmStrategy {
    variation: VariationConfig,
    scoring: ScoringConfig,
    convergence: ConvergencePolicy,
}

#[derive(Debug, Clone)]
pub struct SupervisionStrategy {
    variation: VariationConfig,
    review_policy: SupervisionReviewPolicy,
}

impl Default for SwarmStrategy {
    fn default() -> Self {
        Self {
            variation: VariationConfig::explicit(1, Vec::new()),
            scoring: ScoringConfig {
                metrics: Vec::new(),
                pass_threshold: 0.0,
                tie_break_metric: String::new(),
            },
            convergence: ConvergencePolicy::default(),
        }
    }
}

impl SwarmStrategy {
    pub fn new(
        variation: VariationConfig,
        scoring: ScoringConfig,
        convergence: ConvergencePolicy,
    ) -> Self {
        Self {
            variation,
            scoring,
            convergence,
        }
    }

    pub fn plan_candidates(
        &self,
        accumulator: &ExecutionAccumulator,
        inboxes: &[CandidateInbox],
        message_stats: Option<&MessageStats>,
    ) -> Vec<CandidateSpec> {
        let mut candidates: Vec<_> = self
            .variation
            .generate(accumulator)
            .into_iter()
            .enumerate()
            .map(|(idx, proposal)| CandidateSpec {
                candidate_id: inboxes
                    .get(idx)
                    .map(|inbox| inbox.candidate_id.clone())
                    .unwrap_or_else(|| format!("candidate-{}", idx + 1)),
                overrides: proposal.overrides,
            })
            .collect();

        if let Some(stats) = advisory_message_stats(&self.variation, message_stats) {
            let exploration_pressure = stats.proposal_count
                + stats.signal_count
                + stats.unique_sources
                + stats.leader_messages;
            let convergence_pressure =
                stats.broadcast_messages + stats.dropped_count + stats.expired_count;
            if convergence_pressure > exploration_pressure && candidates.len() > 1 {
                candidates.truncate(candidates.len().div_ceil(2));
            }
        }

        candidates
    }

    pub fn evaluate(
        &self,
        _accumulator: &ExecutionAccumulator,
        outputs: &[CandidateOutput],
    ) -> IterationEvaluation {
        IterationEvaluation {
            ranked_candidates: score_iteration(&self.scoring, outputs),
        }
    }

    pub fn should_stop(
        &self,
        accumulator: &ExecutionAccumulator,
        evaluation: &IterationEvaluation,
    ) -> Option<StopReason> {
        match self.convergence.strategy.as_str() {
            "threshold" => {
                let best = evaluation.ranked_candidates.first()?;
                if best.score >= self.convergence.min_score.unwrap_or(f64::INFINITY) {
                    Some(StopReason::ConvergenceThreshold)
                } else {
                    None
                }
            }
            "plateau" => {
                if accumulator.iterations_without_improvement
                    >= self
                        .convergence
                        .max_iterations_without_improvement
                        .unwrap_or(u32::MAX)
                {
                    Some(StopReason::ConvergencePlateau)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn reduce(
        &self,
        mut accumulator: ExecutionAccumulator,
        evaluation: IterationEvaluation,
    ) -> ExecutionAccumulator {
        accumulator.scoring_history_len += 1;
        accumulator.completed_iterations += 1;
        accumulator.failure_counts.total_candidate_failures += evaluation
            .ranked_candidates
            .iter()
            .filter(|candidate| !candidate.pass)
            .count() as u32;
        if let Some(best) = evaluation.ranked_candidates.first() {
            accumulator.best_candidate_id = Some(best.candidate_id.clone());
        }
        accumulator
    }
}

impl SupervisionStrategy {
    pub fn new(variation: VariationConfig, review_policy: SupervisionReviewPolicy) -> Self {
        Self {
            variation,
            review_policy,
        }
    }

    pub fn plan_candidates(
        &self,
        accumulator: &ExecutionAccumulator,
        inboxes: &[CandidateInbox],
    ) -> Vec<CandidateSpec> {
        let mut candidates = Vec::new();
        let proposals = self.variation.generate(accumulator);

        for (idx, proposal) in proposals.into_iter().enumerate() {
            let candidate_id = if let Some(inbox) = inboxes.get(idx) {
                inbox.candidate_id.clone()
            } else {
                format!("candidate-{}", idx + 1)
            };
            candidates.push(CandidateSpec {
                candidate_id,
                overrides: proposal.overrides,
            });
        }

        candidates
    }

    pub fn evaluate(
        &self,
        accumulator: &ExecutionAccumulator,
        outputs: &[CandidateOutput],
    ) -> SupervisionEvaluation {
        let mut decisions = Vec::new();
        let mut final_approval_ready = !outputs.is_empty();

        for output in outputs {
            let revision_round = accumulator
                .supervision_revision_rounds
                .get(&output.candidate_id)
                .copied()
                .unwrap_or(0);
            let status = self.review_status_for_output(output, revision_round);
            if status != WorkerReviewStatus::Approved {
                final_approval_ready = false;
            }
            decisions.push(WorkerReviewDecision {
                candidate_id: output.candidate_id.clone(),
                status,
                revision_round,
            });
        }

        SupervisionEvaluation {
            decisions,
            final_approval_ready,
        }
    }

    pub fn reduce(
        &self,
        mut accumulator: ExecutionAccumulator,
        evaluation: &SupervisionEvaluation,
    ) -> ExecutionAccumulator {
        let mut approved_count = 0;

        for decision in &evaluation.decisions {
            accumulator
                .supervision_reviews
                .insert(decision.candidate_id.clone(), decision.status);
            match decision.status {
                WorkerReviewStatus::RevisionRequested | WorkerReviewStatus::RetryRequested => {
                    accumulator.supervision_revision_rounds.insert(
                        decision.candidate_id.clone(),
                        decision.revision_round + 1,
                    );
                }
                WorkerReviewStatus::Approved => {
                    approved_count += 1;
                }
                WorkerReviewStatus::PendingReview | WorkerReviewStatus::Rejected => {}
            }
        }

        if approved_count > 0 && evaluation.final_approval_ready {
            accumulator.supervision_final_approval = Some(true);
        } else if !evaluation.decisions.is_empty() {
            accumulator.supervision_final_approval = Some(false);
        }

        accumulator
    }

    fn review_status_for_output(
        &self,
        output: &CandidateOutput,
        revision_round: u32,
    ) -> WorkerReviewStatus {
        if !output.succeeded {
            if self.review_policy.retry_on_runtime_failure
                && revision_round < self.review_policy.max_revision_rounds
            {
                return WorkerReviewStatus::RetryRequested;
            }
            return WorkerReviewStatus::Rejected;
        }

        let approved = output.metrics.get("approved").copied().unwrap_or(0.0);
        if approved >= 1.0 {
            return WorkerReviewStatus::Approved;
        }

        if revision_round < self.review_policy.max_revision_rounds {
            return WorkerReviewStatus::RevisionRequested;
        }

        WorkerReviewStatus::Rejected
    }
}

fn advisory_message_stats<'a>(
    variation: &VariationConfig,
    message_stats: Option<&'a MessageStats>,
) -> Option<&'a MessageStats> {
    let stats = message_stats?;
    if variation.source == "leader_directed" || stats.total_messages == 0 {
        return None;
    }
    Some(stats)
}
