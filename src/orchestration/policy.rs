use std::error::Error;
use std::fmt::{Display, Formatter};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalConfig {
    pub max_concurrent_child_runs: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetPolicy {
    pub max_iterations: Option<u32>,
    pub max_child_runs: Option<u32>,
    pub max_wall_clock_secs: Option<u32>,
    pub max_cost_usd_millis: Option<u64>,
}

impl Default for BudgetPolicy {
    fn default() -> Self {
        Self {
            max_iterations: Some(10),
            max_child_runs: None,
            max_wall_clock_secs: Some(600),
            max_cost_usd_millis: None,
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcurrencyPolicy {
    pub max_concurrent_candidates: u32,
}

impl Default for ConcurrencyPolicy {
    fn default() -> Self {
        Self {
            max_concurrent_candidates: 1,
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ConvergencePolicy {
    pub strategy: String,
    pub min_score: Option<f64>,
    pub max_iterations_without_improvement: Option<u32>,
}

impl Default for ConvergencePolicy {
    fn default() -> Self {
        Self {
            strategy: "plateau".to_string(),
            min_score: None,
            max_iterations_without_improvement: Some(2),
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct OrchestrationPolicy {
    pub budget: BudgetPolicy,
    pub concurrency: ConcurrencyPolicy,
    pub convergence: ConvergencePolicy,
    pub max_candidate_failures_per_iteration: u32,
    pub missing_output_policy: String,
    pub iteration_failure_policy: String,
}

impl Default for OrchestrationPolicy {
    fn default() -> Self {
        Self {
            budget: BudgetPolicy::default(),
            concurrency: ConcurrencyPolicy::default(),
            convergence: ConvergencePolicy::default(),
            max_candidate_failures_per_iteration: u32::MAX,
            missing_output_policy: "mark_failed".to_string(),
            iteration_failure_policy: "fail_execution".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyValidationError(String);

impl PolicyValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl Display for PolicyValidationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for PolicyValidationError {}

impl OrchestrationPolicy {
    pub fn validate(&self, global: &GlobalConfig) -> Result<(), PolicyValidationError> {
        if self.budget.max_iterations.is_none() && self.budget.max_wall_clock_secs.is_none() {
            return Err(PolicyValidationError::new(
                "at least one of policy.budget.max_iterations or policy.budget.max_wall_clock_secs must be set",
            ));
        }

        if self.concurrency.max_concurrent_candidates == 0 {
            return Err(PolicyValidationError::new(
                "policy.concurrency.max_concurrent_candidates must be positive",
            ));
        }

        if self.concurrency.max_concurrent_candidates > global.max_concurrent_child_runs {
            return Err(PolicyValidationError::new(
                "policy.concurrency.max_concurrent_candidates cannot exceed global.max_concurrent_child_runs",
            ));
        }

        match self.convergence.strategy.as_str() {
            "threshold" => {
                if self.convergence.min_score.is_none() {
                    return Err(PolicyValidationError::new(
                        "policy.convergence.min_score is required for threshold strategy",
                    ));
                }
            }
            "plateau" => {
                if self
                    .convergence
                    .max_iterations_without_improvement
                    .is_none()
                {
                    return Err(PolicyValidationError::new(
                        "policy.convergence.max_iterations_without_improvement is required for plateau strategy",
                    ));
                }
            }
            "exhaustive" => {
                if self.budget.max_iterations.is_none() {
                    return Err(PolicyValidationError::new(
                        "policy.budget.max_iterations is required for exhaustive strategy",
                    ));
                }
            }
            other => {
                return Err(PolicyValidationError::new(format!(
                    "unknown convergence strategy '{}'",
                    other
                )));
            }
        }

        Ok(())
    }
}
