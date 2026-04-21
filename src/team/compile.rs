use std::collections::BTreeMap;

use crate::orchestration::{
    BudgetPolicy, ConcurrencyPolicy, ConvergencePolicy, EvaluationConfig, ExecutionSpec,
    GlobalConfig, OrchestrationPolicy, SupervisionConfig, SupervisionReviewPolicy, VariationConfig,
    VariationProposal, WorkflowTemplateRef,
};

use super::{TeamSpec, TeamValidationError};

const DEFAULT_TEAM_TEMPLATE: &str = "examples/runtime-templates/warm_agent_basic.yaml";

/// Compiles a [`TeamSpec`] into a normal [`ExecutionSpec`].
///
/// # Errors
///
/// Returns [`TeamValidationError`] if the compiled execution spec is invalid.
pub fn compile_team_spec(spec: &TeamSpec) -> Result<ExecutionSpec, TeamValidationError> {
    spec.validate()?;

    let mut explicit = Vec::new();
    for task in &spec.tasks {
        let agent_name = match task.agent.as_deref() {
            Some(agent_name) => agent_name,
            None => &spec.agents[0].name,
        };
        let Some(agent) = spec.agents.iter().find(|agent| agent.name == agent_name) else {
            return Err(TeamValidationError::new(format!(
                "tasks['{}'].agent references unknown agent '{}'",
                task.name, agent_name
            )));
        };

        let mut overrides = BTreeMap::new();
        overrides.insert("agent.prompt".to_string(), task.description.clone());
        overrides.insert("agent.role".to_string(), agent.role.clone());
        overrides.insert("agent.goal".to_string(), agent.goal.clone());
        explicit.push(VariationProposal { overrides });
    }

    let mode = match spec.process.kind.as_str() {
        "lead_worker" => "supervision",
        "sequential" | "parallel" => "swarm",
        _ => "swarm",
    };
    let candidate_count = spec.tasks.len() as u32;
    let execution = ExecutionSpec {
        mode: mode.to_string(),
        goal: team_goal(spec),
        workflow: WorkflowTemplateRef {
            template: workflow_template(spec).to_string(),
        },
        policy: OrchestrationPolicy {
            budget: BudgetPolicy {
                max_iterations: Some(1),
                max_child_runs: Some(candidate_count),
                max_wall_clock_secs: Some(1800),
                max_cost_usd_millis: None,
            },
            concurrency: ConcurrencyPolicy {
                max_concurrent_candidates: concurrency_limit(spec),
            },
            convergence: ConvergencePolicy {
                strategy: "exhaustive".to_string(),
                min_score: None,
                max_iterations_without_improvement: None,
            },
            max_candidate_failures_per_iteration: candidate_count,
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
        variation: VariationConfig::explicit(candidate_count, explicit),
        swarm: true,
        supervision: supervision_config(spec),
    };

    execution
        .validate(&GlobalConfig {
            max_concurrent_child_runs: 20,
        })
        .map_err(|err| {
            TeamValidationError::new(format!("compiled execution spec is invalid: {err}"))
        })?;

    Ok(execution)
}

fn workflow_template(spec: &TeamSpec) -> &str {
    for agent in &spec.agents {
        let Some(template) = &agent.template else {
            continue;
        };
        if !template.trim().is_empty() {
            return template;
        }
    }
    DEFAULT_TEAM_TEMPLATE
}

fn concurrency_limit(spec: &TeamSpec) -> u32 {
    match spec.process.kind.as_str() {
        "sequential" => 1,
        "parallel" => spec.tasks.len() as u32,
        "lead_worker" => spec.tasks.len() as u32,
        _ => spec.tasks.len() as u32,
    }
}

fn supervision_config(spec: &TeamSpec) -> Option<SupervisionConfig> {
    if spec.process.kind != "lead_worker" {
        return None;
    }
    let supervisor_role = match spec.agents.first() {
        Some(agent) => agent.role.clone(),
        None => "Supervisor".to_string(),
    };
    Some(SupervisionConfig {
        supervisor_role,
        review_policy: SupervisionReviewPolicy {
            max_revision_rounds: 1,
            retry_on_runtime_failure: true,
            require_final_approval: false,
        },
    })
}

fn team_goal(spec: &TeamSpec) -> String {
    let Some(metadata) = &spec.metadata else {
        return format!("run {} team tasks", spec.tasks.len());
    };
    let Some(name) = &metadata.name else {
        return format!("run {} team tasks", spec.tasks.len());
    };
    name.clone()
}
