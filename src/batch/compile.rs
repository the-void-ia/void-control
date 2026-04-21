use std::collections::BTreeMap;

use crate::orchestration::{
    BudgetPolicy, ConcurrencyPolicy, ConvergencePolicy, EvaluationConfig, ExecutionSpec,
    GlobalConfig, OrchestrationPolicy, VariationConfig, VariationProposal, WorkflowTemplateRef,
};

use super::{BatchSpec, BatchValidationError};

/// Compiles a [`BatchSpec`] into a normal [`ExecutionSpec`].
///
/// # Examples
///
/// ```
/// let spec = void_control::batch::parse_batch_yaml(
///     r#"
/// api_version: v1
/// kind: batch
/// worker:
///   template: examples/runtime-templates/warm_agent_basic.yaml
/// jobs:
///   - prompt: Fix failing auth tests
/// "#,
/// )
/// .expect("parse batch");
/// let execution = void_control::batch::compile_batch_spec(&spec).expect("compile batch");
/// assert_eq!(execution.mode, "swarm");
/// ```
///
/// # Errors
///
/// Returns [`BatchValidationError`] if the compiled execution spec is invalid.
pub fn compile_batch_spec(spec: &BatchSpec) -> Result<ExecutionSpec, BatchValidationError> {
    let parallelism = match spec.mode.as_ref().and_then(|mode| mode.parallelism) {
        Some(parallelism) => parallelism,
        None => spec.jobs.len() as u32,
    };

    let mut explicit = Vec::new();
    for job in &spec.jobs {
        let mut overrides = BTreeMap::new();
        overrides.insert("agent.prompt".to_string(), job.prompt.clone());
        if let Some(provider) = &spec.worker.provider {
            overrides.insert("llm.provider".to_string(), provider.clone());
        }
        explicit.push(VariationProposal { overrides });
    }

    let max_child_runs = spec.jobs.len() as u32;
    let execution = ExecutionSpec {
        mode: "swarm".to_string(),
        goal: batch_goal(spec),
        workflow: WorkflowTemplateRef {
            template: spec.worker.template.clone(),
        },
        policy: OrchestrationPolicy {
            budget: BudgetPolicy {
                max_iterations: Some(1),
                max_child_runs: Some(max_child_runs),
                max_wall_clock_secs: Some(1800),
                max_cost_usd_millis: None,
            },
            concurrency: ConcurrencyPolicy {
                max_concurrent_candidates: parallelism,
            },
            convergence: ConvergencePolicy {
                strategy: "exhaustive".to_string(),
                min_score: None,
                max_iterations_without_improvement: None,
            },
            max_candidate_failures_per_iteration: max_child_runs,
            missing_output_policy: "mark_failed".to_string(),
            iteration_failure_policy: "continue".to_string(),
        },
        evaluation: EvaluationConfig {
            scoring_type: "weighted_metrics".to_string(),
            weights: BTreeMap::from([("success".to_string(), 1.0)]),
            pass_threshold: Some(1.0),
            ranking: "highest_score".to_string(),
            tie_breaking: "success".to_string(),
        },
        variation: VariationConfig::explicit(parallelism, explicit),
        swarm: true,
        supervision: None,
    };
    execution
        .validate(&GlobalConfig {
            max_concurrent_child_runs: 20,
        })
        .map_err(|err| {
            BatchValidationError::new(format!("compiled execution spec is invalid: {err}"))
        })?;
    Ok(execution)
}

fn batch_goal(spec: &BatchSpec) -> String {
    let Some(metadata) = &spec.metadata else {
        return format!("run {} background jobs", spec.jobs.len());
    };
    let Some(name) = &metadata.name else {
        return format!("run {} background jobs", spec.jobs.len());
    };
    name.clone()
}
