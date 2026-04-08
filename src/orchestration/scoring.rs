use std::cmp::Ordering;
use std::collections::BTreeMap;

use super::types::CandidateOutput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricDirection {
    Minimize,
    Maximize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeightedMetric {
    pub name: String,
    pub weight: f64,
    pub direction: MetricDirection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoringConfig {
    pub metrics: Vec<WeightedMetric>,
    pub pass_threshold: f64,
    pub tie_break_metric: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RankedCandidate {
    pub candidate_id: String,
    pub score: f64,
    pub pass: bool,
    pub metrics: BTreeMap<String, f64>,
}

pub fn score_iteration(
    config: &ScoringConfig,
    outputs: &[CandidateOutput],
) -> Vec<RankedCandidate> {
    let mut ranked: Vec<RankedCandidate> = outputs
        .iter()
        .map(|output| {
            if !output.succeeded {
                return RankedCandidate {
                    candidate_id: output.candidate_id.clone(),
                    score: 0.0,
                    pass: false,
                    metrics: output.metrics.clone(),
                };
            }

            let score = config
                .metrics
                .iter()
                .map(|metric| metric.weight * normalized_value(metric, outputs, output))
                .sum::<f64>();

            RankedCandidate {
                candidate_id: output.candidate_id.clone(),
                score,
                pass: score >= config.pass_threshold,
                metrics: output.metrics.clone(),
            }
        })
        .collect();

    ranked.sort_by(|left, right| compare_ranked(config, left, right));
    ranked
}

fn normalized_value(
    metric: &WeightedMetric,
    outputs: &[CandidateOutput],
    output: &CandidateOutput,
) -> f64 {
    let values: Vec<f64> = outputs
        .iter()
        .filter(|candidate| candidate.succeeded)
        .filter_map(|candidate| candidate.metrics.get(&metric.name).copied())
        .collect();
    let Some(current) = output.metrics.get(&metric.name).copied() else {
        return 0.0;
    };
    if values.len() <= 1 {
        return 1.0;
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if (max - min).abs() < f64::EPSILON {
        return 1.0;
    }
    match metric.direction {
        MetricDirection::Minimize => (max - current) / (max - min),
        MetricDirection::Maximize => (current - min) / (max - min),
    }
}

fn compare_ranked(
    config: &ScoringConfig,
    left: &RankedCandidate,
    right: &RankedCandidate,
) -> Ordering {
    right
        .score
        .partial_cmp(&left.score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| {
            let left_metric = left
                .metrics
                .get(&config.tie_break_metric)
                .copied()
                .unwrap_or(f64::INFINITY);
            let right_metric = right
                .metrics
                .get(&config.tie_break_metric)
                .copied()
                .unwrap_or(f64::INFINITY);
            left_metric
                .partial_cmp(&right_metric)
                .unwrap_or(Ordering::Equal)
        })
}
