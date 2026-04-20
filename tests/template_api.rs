#[cfg(feature = "serde")]
use void_control::templates;

#[cfg(feature = "serde")]
fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[cfg(feature = "serde")]
#[test]
fn template_api_module_is_exposed() {
    let _ = std::any::type_name::<templates::TemplateModuleMarker>();
}

#[cfg(feature = "serde")]
#[test]
fn template_schema_parses_phase_one_shape() {
    let yaml = r#"
api_version: v1
kind: control_template

template:
  id: single-agent-basic
  name: Single Agent
  execution_kind: single_agent
  description: Run one agent once and return the result.

inputs:
  goal:
    type: string
    required: true
    description: Goal shown in the execution record.
  provider:
    type: enum
    required: false
    default: claude
    values: [claude, codex]
    description: Provider override.

defaults:
  workflow_template: examples/runtime-templates/claude_mcp_diagnostic_agent.yaml
  execution_spec:
    mode: swarm
    goal: Single agent task
    workflow:
      template: ""
    policy:
      budget:
        max_iterations: 1
        max_child_runs: 1
        max_wall_clock_secs: 900
        max_cost_usd_millis: null
      concurrency:
        max_concurrent_candidates: 1
      convergence:
        strategy: threshold
        min_score: 1.0
        max_iterations_without_improvement: 0
      max_candidate_failures_per_iteration: 1
      missing_output_policy: mark_failed
      iteration_failure_policy: fail_execution
    evaluation:
      scoring_type: weighted_metrics
      weights:
        success: 1.0
      pass_threshold: 1.0
      ranking: highest_score
      tie_breaking: success
    variation:
      source: explicit
      candidates_per_iteration: 1
      selection: null
      parameter_space: {}
      explicit:
        - overrides: {}
    swarm: true
    supervision: null

compile:
  bindings:
    - input: goal
      target: execution_spec.goal
    - input: provider
      target: variation.explicit[0].overrides.llm.provider
"#;

    let template = templates::parse_template_yaml(yaml).expect("parse template");
    assert_eq!(template.template.id, "single-agent-basic");
    assert_eq!(template.template.execution_kind.as_str(), "single_agent");
    assert_eq!(
        template.defaults.workflow_template,
        "examples/runtime-templates/claude_mcp_diagnostic_agent.yaml"
    );
    assert_eq!(template.compile.bindings.len(), 2);
}

#[cfg(feature = "serde")]
#[test]
fn template_loader_lists_checked_in_templates() {
    let template_dir = repo_root().join("templates");
    assert!(template_dir.exists(), "template dir should exist");

    let templates = templates::list_templates().expect("list templates");
    assert!(
        templates
            .iter()
            .any(|template| template.id == "single-agent-basic"),
        "single-agent-basic should be listed"
    );
    assert!(
        templates
            .iter()
            .any(|template| template.id == "warm-agent-basic"),
        "warm-agent-basic should be listed"
    );
}

#[cfg(feature = "serde")]
#[test]
fn template_loader_loads_single_agent_template() {
    let template = templates::load_template("single-agent-basic").expect("load single agent");
    assert_eq!(template.template.id, "single-agent-basic");
    assert_eq!(template.template.execution_kind.as_str(), "single_agent");
}

#[cfg(feature = "serde")]
#[test]
fn template_loader_loads_warm_agent_template() {
    let template = templates::load_template("warm-agent-basic").expect("load warm agent");
    assert_eq!(template.template.id, "warm-agent-basic");
    assert_eq!(template.template.execution_kind.as_str(), "warm_agent");
    assert_eq!(
        template.defaults.workflow_template,
        "examples/runtime-templates/warm_agent_basic.yaml"
    );
}

#[cfg(feature = "serde")]
#[test]
fn template_compile_builds_single_agent_execution_spec() {
    let template = templates::load_template("single-agent-basic").expect("load single agent");
    let inputs = serde_json::json!({
        "goal": "Review the repo",
        "prompt": "Summarize the highest-risk areas.",
        "provider": "claude"
    });

    let compiled = templates::compile_template(&template, &inputs).expect("compile template");
    assert_eq!(compiled.execution_spec.goal, "Review the repo");
    assert_eq!(
        compiled.execution_spec.workflow.template,
        "examples/runtime-templates/claude_mcp_diagnostic_agent.yaml"
    );
    assert_eq!(
        compiled.execution_spec.variation.explicit[0]
            .overrides
            .get("agent.prompt")
            .map(String::as_str),
        Some("Summarize the highest-risk areas.")
    );
    assert_eq!(
        compiled.execution_spec.variation.explicit[0]
            .overrides
            .get("llm.provider")
            .map(String::as_str),
        Some("claude")
    );
}

#[cfg(feature = "serde")]
#[test]
fn template_compile_builds_warm_agent_execution_spec() {
    let template = templates::load_template("warm-agent-basic").expect("load warm agent");
    let inputs = serde_json::json!({
        "goal": "Keep a warm agent ready",
        "prompt": "Stay alive for follow-up repo work."
    });

    let compiled = templates::compile_template(&template, &inputs).expect("compile warm template");
    assert_eq!(compiled.execution_spec.goal, "Keep a warm agent ready");
    assert_eq!(
        compiled.execution_spec.workflow.template,
        "examples/runtime-templates/warm_agent_basic.yaml"
    );
    assert_eq!(
        compiled.execution_spec.variation.explicit[0]
            .overrides
            .get("agent.prompt")
            .map(String::as_str),
        Some("Stay alive for follow-up repo work.")
    );
    assert_eq!(
        compiled.execution_spec.variation.explicit[0]
            .overrides
            .get("llm.provider")
            .map(String::as_str),
        Some("claude")
    );
}

#[cfg(feature = "serde")]
#[test]
fn template_compile_rejects_missing_required_input() {
    let template = templates::load_template("single-agent-basic").expect("load single agent");
    let inputs = serde_json::json!({
        "goal": "Missing prompt"
    });

    let err = templates::compile_template(&template, &inputs).expect_err("compile should fail");
    assert!(
        err.to_string().contains("missing required input 'prompt'"),
        "unexpected error: {err}"
    );
}

#[cfg(feature = "serde")]
#[test]
fn template_compile_rejects_invalid_enum_input() {
    let template = templates::load_template("single-agent-basic").expect("load single agent");
    let inputs = serde_json::json!({
        "goal": "Bad provider",
        "prompt": "Run once.",
        "provider": "openai"
    });

    let err = templates::compile_template(&template, &inputs).expect_err("compile should fail");
    assert!(
        err.to_string().contains("input 'provider' must be one of"),
        "unexpected error: {err}"
    );
}
