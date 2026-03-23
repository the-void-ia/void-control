use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::policy::{GlobalConfig, OrchestrationPolicy};
use super::variation::VariationConfig;

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowTemplateRef {
    pub template: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluationConfig {
    pub scoring_type: String,
    pub weights: BTreeMap<String, f64>,
    pub pass_threshold: Option<f64>,
    pub ranking: String,
    pub tie_breaking: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionSpec {
    pub mode: String,
    pub goal: String,
    pub workflow: WorkflowTemplateRef,
    pub policy: OrchestrationPolicy,
    pub evaluation: EvaluationConfig,
    pub variation: VariationConfig,
    pub swarm: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecValidationError(String);

impl SpecValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl Display for SpecValidationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SpecValidationError {}

impl ExecutionSpec {
    pub fn validate(&self, global: &GlobalConfig) -> Result<(), SpecValidationError> {
        if !matches!(self.mode.as_str(), "swarm" | "search") {
            return Err(SpecValidationError::new(format!(
                "unknown mode '{}'",
                self.mode
            )));
        }

        if self.mode == "swarm" && !self.swarm {
            return Err(SpecValidationError::new(
                "swarm mode requires the swarm section",
            ));
        }

        self.policy
            .validate(global)
            .map_err(|err| SpecValidationError::new(err.to_string()))?;

        if self.variation.candidates_per_iteration == 0 {
            return Err(SpecValidationError::new(
                "variation.candidates_per_iteration must be positive",
            ));
        }

        if self.workflow.template.trim().is_empty() {
            return Err(SpecValidationError::new("workflow.template is required"));
        }

        Ok(())
    }
}
