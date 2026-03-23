use std::collections::BTreeMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::types::ExecutionAccumulator;

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariationSelection {
    Random,
    Sequential,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariationProposal {
    pub overrides: BTreeMap<String, String>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariationConfig {
    pub source: String,
    pub candidates_per_iteration: u32,
    pub selection: Option<VariationSelection>,
    pub parameter_space: BTreeMap<String, Vec<String>>,
    pub explicit: Vec<VariationProposal>,
}

impl VariationConfig {
    pub fn parameter_space(
        candidates_per_iteration: u32,
        selection: VariationSelection,
        parameter_space: BTreeMap<String, Vec<String>>,
    ) -> Self {
        Self {
            source: "parameter_space".to_string(),
            candidates_per_iteration,
            selection: Some(selection),
            parameter_space,
            explicit: Vec::new(),
        }
    }

    pub fn explicit(candidates_per_iteration: u32, explicit: Vec<VariationProposal>) -> Self {
        Self {
            source: "explicit".to_string(),
            candidates_per_iteration,
            selection: None,
            parameter_space: BTreeMap::new(),
            explicit,
        }
    }

    pub fn leader_directed(candidates_per_iteration: u32) -> Self {
        Self {
            source: "leader_directed".to_string(),
            candidates_per_iteration,
            selection: None,
            parameter_space: BTreeMap::new(),
            explicit: Vec::new(),
        }
    }

    pub fn signal_reactive(candidates_per_iteration: u32) -> Self {
        Self {
            source: "signal_reactive".to_string(),
            candidates_per_iteration,
            selection: None,
            parameter_space: BTreeMap::new(),
            explicit: Vec::new(),
        }
    }

    pub fn generate(&self, accumulator: &ExecutionAccumulator) -> Vec<VariationProposal> {
        match self.source.as_str() {
            "parameter_space" => self.generate_parameter_space(),
            "explicit" => self.generate_explicit(accumulator),
            "leader_directed" => accumulator
                .leader_proposals
                .iter()
                .filter(|proposal| !proposal.overrides.is_empty())
                .take(self.candidates_per_iteration as usize)
                .cloned()
                .collect(),
            "signal_reactive" => {
                if !self.explicit.is_empty() {
                    self.generate_explicit(accumulator)
                } else if !self.parameter_space.is_empty() {
                    self.generate_parameter_space()
                } else {
                    accumulator
                        .leader_proposals
                        .iter()
                        .filter(|proposal| !proposal.overrides.is_empty())
                        .take(self.candidates_per_iteration as usize)
                        .cloned()
                        .collect()
                }
            }
            _ => Vec::new(),
        }
    }

    fn generate_parameter_space(&self) -> Vec<VariationProposal> {
        let Some((path, values)) = self.parameter_space.iter().next() else {
            return Vec::new();
        };

        let iter: Box<dyn Iterator<Item = String>> = match self.selection.unwrap_or(VariationSelection::Sequential) {
            VariationSelection::Sequential => Box::new(values.iter().cloned()),
            VariationSelection::Random => Box::new(values.iter().rev().cloned()),
        };

        iter.take(self.candidates_per_iteration as usize)
            .map(|value| VariationProposal {
                overrides: BTreeMap::from([(path.clone(), value)]),
            })
            .collect()
    }

    fn generate_explicit(&self, accumulator: &ExecutionAccumulator) -> Vec<VariationProposal> {
        if self.explicit.is_empty() {
            return Vec::new();
        }

        let start = accumulator.scoring_history_len as usize % self.explicit.len();
        (0..self.candidates_per_iteration as usize)
            .map(|offset| self.explicit[(start + offset) % self.explicit.len()].clone())
            .collect()
    }
}
