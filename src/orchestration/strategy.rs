use super::policy::ConvergencePolicy;
use super::scoring::{score_iteration, RankedCandidate, ScoringConfig};
use super::types::{
    CandidateInbox, CandidateOutput, CandidateSpec, ExecutionAccumulator, MessageStats,
};
use super::variation::VariationConfig;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct IterationEvaluation {
    pub ranked_candidates: Vec<RankedCandidate>,
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
pub struct SearchStrategy {
    variation: VariationConfig,
    scoring: ScoringConfig,
    convergence: ConvergencePolicy,
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

impl SearchStrategy {
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
        let proposals = if accumulator.best_candidate_overrides.is_empty() {
            self.bootstrap_proposals(accumulator)
        } else {
            self.refinement_proposals(accumulator, message_stats)
        };

        proposals
            .into_iter()
            .enumerate()
            .map(|(idx, proposal)| CandidateSpec {
                candidate_id: inboxes
                    .get(idx)
                    .map(|inbox| inbox.candidate_id.clone())
                    .unwrap_or_else(|| format!("candidate-{}", idx + 1)),
                overrides: proposal.overrides,
            })
            .collect()
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
                    return Some(StopReason::ConvergenceThreshold);
                }
            }
            "plateau" => {
                if accumulator.iterations_without_improvement
                    >= self
                        .convergence
                        .max_iterations_without_improvement
                        .unwrap_or(u32::MAX)
                {
                    return Some(StopReason::ConvergencePlateau);
                }
            }
            _ => {}
        }

        if !accumulator.best_candidate_overrides.is_empty()
            && self.refinement_proposals(accumulator, None).is_empty()
        {
            return Some(StopReason::ConvergencePlateau);
        }
        None
    }

    pub fn reduce(
        &self,
        mut accumulator: ExecutionAccumulator,
        planned_candidates: &[CandidateSpec],
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
            if let Some(spec) = planned_candidates
                .iter()
                .find(|candidate| candidate.candidate_id == best.candidate_id)
            {
                accumulator.best_candidate_overrides = spec.overrides.clone();
                let signature = candidate_signature(&spec.overrides);
                if !signature.is_empty() && !accumulator.explored_signatures.contains(&signature) {
                    accumulator.explored_signatures.push(signature);
                }
            }
        }
        for candidate in planned_candidates {
            let signature = candidate_signature(&candidate.overrides);
            if !signature.is_empty() && !accumulator.explored_signatures.contains(&signature) {
                accumulator.explored_signatures.push(signature);
            }
        }
        accumulator.search_phase = Some(if accumulator.best_candidate_overrides.is_empty() {
            "bootstrap".to_string()
        } else {
            "refine".to_string()
        });
        accumulator
    }

    fn bootstrap_proposals(
        &self,
        accumulator: &ExecutionAccumulator,
    ) -> Vec<super::variation::VariationProposal> {
        let mut generated = self.variation.generate(accumulator);
        let bootstrap_size = self.variation.candidates_per_iteration.clamp(1, 2) as usize;
        generated.truncate(bootstrap_size);
        generated
    }

    fn refinement_proposals(
        &self,
        accumulator: &ExecutionAccumulator,
        message_stats: Option<&MessageStats>,
    ) -> Vec<super::variation::VariationProposal> {
        let mut proposals: Vec<_> = match refinement_source(&self.variation) {
            RefinementSource::Explicit => self.refine_explicit(accumulator),
            RefinementSource::ParameterSpace => self.refine_parameter_space(accumulator),
            RefinementSource::None => Vec::new(),
        }
        .into_iter()
        .filter(|proposal| {
            let signature = candidate_signature(&proposal.overrides);
            !accumulator.explored_signatures.contains(&signature)
        })
        .collect();

        if let Some(stats) = advisory_message_stats(&self.variation, message_stats) {
            let exploration_pressure =
                stats.signal_count + stats.dropped_count + stats.expired_count;
            let refinement_pressure = stats.evaluation_count + stats.leader_messages;
            if exploration_pressure > refinement_pressure && proposals.len() > 2 {
                let first = proposals.remove(0);
                if let Some(last) = proposals.pop() {
                    proposals.insert(0, last);
                    proposals.insert(0, first);
                } else {
                    proposals.insert(0, first);
                }
            }
        }

        proposals
            .into_iter()
            .take(self.variation.candidates_per_iteration as usize)
            .collect()
    }

    fn refine_explicit(
        &self,
        accumulator: &ExecutionAccumulator,
    ) -> Vec<super::variation::VariationProposal> {
        if self.variation.explicit.is_empty() {
            return Vec::new();
        }
        let incumbent_index = self
            .variation
            .explicit
            .iter()
            .position(|proposal| proposal.overrides == accumulator.best_candidate_overrides)
            .unwrap_or(0);

        let mut indices = Vec::new();
        if incumbent_index > 0 {
            indices.push(incumbent_index - 1);
        }
        if incumbent_index + 1 < self.variation.explicit.len() {
            indices.push(incumbent_index + 1);
        }
        for idx in 0..self.variation.explicit.len() {
            if idx != incumbent_index && !indices.contains(&idx) {
                indices.push(idx);
            }
        }
        indices
            .into_iter()
            .map(|idx| self.variation.explicit[idx].clone())
            .collect()
    }

    fn refine_parameter_space(
        &self,
        accumulator: &ExecutionAccumulator,
    ) -> Vec<super::variation::VariationProposal> {
        let mut proposals = Vec::new();
        let incumbent = &accumulator.best_candidate_overrides;
        for (path, values) in &self.variation.parameter_space {
            let current = incumbent.get(path);
            let Some(current_idx) =
                current.and_then(|value| values.iter().position(|candidate| candidate == value))
            else {
                continue;
            };
            for neighbor_idx in [current_idx.checked_sub(1), Some(current_idx + 1)]
                .into_iter()
                .flatten()
            {
                if let Some(value) = values.get(neighbor_idx) {
                    let mut overrides = incumbent.clone();
                    overrides.insert(path.clone(), value.clone());
                    proposals.push(super::variation::VariationProposal { overrides });
                }
            }
        }
        proposals
    }
}

fn candidate_signature(overrides: &BTreeMap<String, String>) -> String {
    overrides
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("|")
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefinementSource {
    Explicit,
    ParameterSpace,
    None,
}

fn refinement_source(variation: &VariationConfig) -> RefinementSource {
    match variation.source.as_str() {
        "explicit" => RefinementSource::Explicit,
        "parameter_space" => RefinementSource::ParameterSpace,
        "signal_reactive" if !variation.explicit.is_empty() => RefinementSource::Explicit,
        "signal_reactive" if !variation.parameter_space.is_empty() => {
            RefinementSource::ParameterSpace
        }
        _ => RefinementSource::None,
    }
}
