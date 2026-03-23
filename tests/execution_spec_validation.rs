use std::collections::BTreeMap;

use void_control::orchestration::{
    ConvergencePolicy, EvaluationConfig, ExecutionSpec, GlobalConfig, OrchestrationPolicy,
    VariationConfig, VariationSelection, WorkflowTemplateRef,
};

#[test]
fn orchestration_module_exports_execution_spec() {
    let _ = std::any::type_name::<ExecutionSpec>();
}

#[test]
fn rejects_unbounded_execution() {
    let err = spec_with(|policy| {
        policy.budget.max_iterations = None;
        policy.budget.max_wall_clock_secs = None;
    })
    .validate(&global_config())
    .expect_err("expected unbounded execution to be rejected");

    assert!(err.to_string().contains("max_iterations"));
}

#[test]
fn rejects_concurrency_above_global_pool() {
    let err = spec_with(|policy| {
        policy.concurrency.max_concurrent_candidates = 3;
    })
    .validate(&GlobalConfig {
        max_concurrent_child_runs: 2,
    })
    .expect_err("expected concurrency validation error");

    assert!(err.to_string().contains("max_concurrent_candidates"));
}

#[test]
fn rejects_threshold_without_min_score() {
    let err = spec_with(|policy| {
        policy.convergence = ConvergencePolicy {
            strategy: "threshold".to_string(),
            min_score: None,
            max_iterations_without_improvement: None,
        };
    })
    .validate(&global_config())
    .expect_err("expected threshold validation error");

    assert!(err.to_string().contains("min_score"));
}

#[test]
fn accepts_exhaustive_with_max_iterations() {
    spec_with(|policy| {
        policy.convergence = ConvergencePolicy {
            strategy: "exhaustive".to_string(),
            min_score: None,
            max_iterations_without_improvement: None,
        };
        policy.budget.max_iterations = Some(5);
    })
    .validate(&global_config())
    .expect("expected exhaustive plan to validate");
}

#[test]
fn rejects_unknown_mode() {
    let mut spec = base_spec();
    spec.mode = "unknown".to_string();

    let err = spec
        .validate(&global_config())
        .expect_err("expected unknown mode to fail");

    assert!(err.to_string().contains("unknown mode"));
}

#[test]
fn accepts_search_mode() {
    let mut spec = base_spec();
    spec.mode = "search".to_string();

    spec.validate(&global_config())
        .expect("expected search mode to validate");
}

#[test]
fn rejects_unknown_variation_source() {
    let mut spec = base_spec();
    spec.variation.source = "unsupported_mode".to_string();

    let err = spec
        .validate(&global_config())
        .expect_err("expected invalid variation source to fail");

    assert!(err.to_string().contains("unsupported_mode"));
}

#[cfg(feature = "serde")]
#[test]
fn bridge_accepts_signal_reactive_and_legacy_leader_directed_variations() {
    use serde_json::json;

    for source in ["signal_reactive", "leader_directed"] {
        let body = json!({
            "mode": "swarm",
            "goal": "optimize latency",
            "workflow": { "template": "fixtures/sample.vbrun" },
            "policy": {
                "budget": {
                    "max_iterations": 3,
                    "max_wall_clock_secs": 60
                },
                "concurrency": {
                    "max_concurrent_candidates": 2
                },
                "convergence": {
                    "strategy": "exhaustive"
                },
                "max_candidate_failures_per_iteration": 10,
                "missing_output_policy": "mark_failed",
                "iteration_failure_policy": "fail_execution"
            },
            "evaluation": {
                "scoring_type": "weighted_metrics",
                "weights": {
                    "latency_p99_ms": -0.6,
                    "cost_usd": -0.4
                },
                "pass_threshold": 0.7,
                "ranking": "highest_score",
                "tie_breaking": "cost_usd"
            },
            "variation": {
                "source": source,
                "candidates_per_iteration": 2
            },
            "swarm": true
        })
        .to_string();

        let response = void_control::bridge::handle_bridge_request_for_test(
            "POST",
            "/v1/executions/dry-run",
            Some(&body),
        )
        .expect("response");

        assert_eq!(response.status, 200);
        assert_eq!(response.json["valid"], true);
        assert_eq!(response.json["plan"]["variation_source"], source);
    }
}

fn global_config() -> GlobalConfig {
    GlobalConfig {
        max_concurrent_child_runs: 4,
    }
}

fn spec_with(edit: impl FnOnce(&mut OrchestrationPolicy)) -> ExecutionSpec {
    let mut spec = base_spec();
    edit(&mut spec.policy);
    spec
}

fn base_spec() -> ExecutionSpec {
    ExecutionSpec {
        mode: "swarm".to_string(),
        goal: "optimize latency".to_string(),
        workflow: WorkflowTemplateRef {
            template: "fixtures/sample.vbrun".to_string(),
        },
        policy: OrchestrationPolicy::default(),
        evaluation: EvaluationConfig {
            scoring_type: "weighted_metrics".to_string(),
            weights: BTreeMap::from([
                ("latency_p99_ms".to_string(), -0.6),
                ("cost_usd".to_string(), -0.4),
            ]),
            pass_threshold: Some(0.7),
            ranking: "highest_score".to_string(),
            tie_breaking: "lowest_cost".to_string(),
        },
        variation: VariationConfig::parameter_space(
            2,
            VariationSelection::Sequential,
            BTreeMap::from([(
                "sandbox.env.CONCURRENCY".to_string(),
                vec!["2".to_string(), "4".to_string()],
            )]),
        ),
        swarm: true,
    }
}
