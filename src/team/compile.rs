use std::collections::BTreeMap;

use crate::orchestration::{
    BudgetPolicy, ConcurrencyPolicy, ConvergencePolicy, EvaluationConfig, ExecutionSpec,
    GlobalConfig, OrchestrationPolicy, SupervisionConfig, SupervisionReviewPolicy, VariationConfig,
    VariationProposal, WorkflowTemplateRef,
};

use super::{AgentSpec, TaskSpec, TeamSpec, TeamValidationError};

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
        let agents = task_agents(spec, task)?;
        for agent in agents {
            let mut overrides = BTreeMap::new();
            overrides.insert("agent.prompt".to_string(), task.description.clone());
            overrides.insert("agent.role".to_string(), agent.role.clone());
            overrides.insert("agent.goal".to_string(), agent.goal.clone());
            explicit.push(VariationProposal { overrides });
        }
    }

    let mode = match spec.process.kind.as_str() {
        "lead_worker" => "supervision",
        "sequential" | "parallel" => "swarm",
        _ => "swarm",
    };
    let candidate_count = explicit.len() as u32;
    let candidates_per_iteration = candidates_per_iteration(spec);
    let max_iterations = if spec.process.kind == "sequential" {
        Some(spec.tasks.len() as u32)
    } else {
        Some(1)
    };
    let execution = ExecutionSpec {
        mode: mode.to_string(),
        goal: team_goal(spec),
        workflow: WorkflowTemplateRef {
            template: workflow_template(spec)?.to_string(),
        },
        policy: OrchestrationPolicy {
            budget: BudgetPolicy {
                max_iterations,
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
        variation: VariationConfig::explicit(candidates_per_iteration, explicit),
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

fn workflow_template(spec: &TeamSpec) -> Result<&str, TeamValidationError> {
    let mut chosen = None;
    for agent in &spec.agents {
        let Some(template) = &agent.template else {
            continue;
        };
        if template.trim().is_empty() {
            continue;
        }
        let Some(current) = chosen else {
            chosen = Some(template.as_str());
            continue;
        };
        if current != template {
            return Err(TeamValidationError::new(
                "team agents must share the same template in phase1",
            ));
        }
    }
    Ok(chosen.unwrap_or(DEFAULT_TEAM_TEMPLATE))
}

fn concurrency_limit(spec: &TeamSpec) -> u32 {
    match spec.process.kind.as_str() {
        "sequential" => 1,
        "parallel" => spec.agents.len().max(1) as u32,
        "lead_worker" => spec.tasks.len().max(1) as u32,
        _ => spec.tasks.len().max(1) as u32,
    }
}

fn supervision_config(spec: &TeamSpec) -> Option<SupervisionConfig> {
    if spec.process.kind != "lead_worker" {
        return None;
    }
    let lead = spec.process.lead.as_deref()?;
    let supervisor_role = match spec.agents.iter().find(|agent| agent.name == lead) {
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

fn candidates_per_iteration(spec: &TeamSpec) -> u32 {
    if spec.process.kind == "sequential" {
        return 1;
    }

    let mut count = 0u32;
    for task in &spec.tasks {
        if task.agent.is_some() {
            count += 1;
            continue;
        }
        count += spec.agents.len() as u32;
    }
    count.max(1)
}

fn task_agents<'a>(
    spec: &'a TeamSpec,
    task: &TaskSpec,
) -> Result<Vec<&'a AgentSpec>, TeamValidationError> {
    let Some(agent_name) = task.agent.as_deref() else {
        let mut agents = Vec::new();
        for agent in &spec.agents {
            agents.push(agent);
        }
        return Ok(agents);
    };
    let Some(agent) = spec.agents.iter().find(|agent| agent.name == agent_name) else {
        return Err(TeamValidationError::new(format!(
            "tasks['{}'].agent references unknown agent '{}'",
            task.name, agent_name
        )));
    };
    Ok(vec![agent])
}
