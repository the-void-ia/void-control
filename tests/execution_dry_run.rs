use std::collections::BTreeMap;

use void_control::orchestration::{
    ExecutionService, ExecutionSpec, FsExecutionStore, GlobalConfig, OrchestrationPolicy,
    VariationConfig, VariationProposal,
};
use void_control::runtime::MockRuntime;

#[test]
fn dry_run_validates_without_creating_execution() {
    let store_dir = temp_store_dir("dry-run-valid");
    let store = FsExecutionStore::new(store_dir.clone());
    let service = ExecutionService::new(GlobalConfig { max_concurrent_child_runs: 4 }, MockRuntime::new(), store);

    let result = service.dry_run(&spec(3)).expect("dry run");

    assert!(result.valid);
    assert!(std::fs::read_dir(store_dir).expect("read dir").next().is_none());
}

#[test]
fn dry_run_returns_plan_warnings_and_errors() {
    let store = FsExecutionStore::new(temp_store_dir("dry-run-errors"));
    let service = ExecutionService::new(GlobalConfig { max_concurrent_child_runs: 4 }, MockRuntime::new(), store);
    let mut spec = spec(3);
    spec.policy.budget.max_wall_clock_secs = None;
    spec.policy.budget.max_iterations = None;
    spec.policy.budget.max_cost_usd_millis = None;

    let result = service.dry_run(&spec).expect("dry run");

    assert!(!result.valid);
    assert!(!result.errors.is_empty());
}

#[test]
fn dry_run_reports_parameter_space_cardinality() {
    let store = FsExecutionStore::new(temp_store_dir("dry-run-cardinality"));
    let service = ExecutionService::new(GlobalConfig { max_concurrent_child_runs: 4 }, MockRuntime::new(), store);
    let spec = ExecutionSpec {
        variation: VariationConfig::parameter_space(
            2,
            void_control::orchestration::VariationSelection::Sequential,
            BTreeMap::from([
                ("sandbox.env.CONCURRENCY".to_string(), vec!["2".to_string(), "4".to_string()]),
                ("sandbox.memory_mb".to_string(), vec!["512".to_string(), "1024".to_string()]),
            ]),
        ),
        ..spec(3)
    };

    let result = service.dry_run(&spec).expect("dry run");

    assert_eq!(result.plan.parameter_space_size, Some(4));
    assert_eq!(result.plan.max_child_runs, Some(6));
}

fn spec(max_iterations: u32) -> ExecutionSpec {
    ExecutionSpec {
        mode: "swarm".to_string(),
        goal: "optimize latency".to_string(),
        workflow: void_control::orchestration::WorkflowTemplateRef {
            template: "fixtures/sample.vbrun".to_string(),
        },
        policy: OrchestrationPolicy {
            budget: void_control::orchestration::BudgetPolicy {
                max_iterations: Some(max_iterations),
                max_child_runs: None,
                max_wall_clock_secs: Some(60),
                max_cost_usd_millis: None,
            },
            concurrency: void_control::orchestration::ConcurrencyPolicy {
                max_concurrent_candidates: 2,
            },
            convergence: void_control::orchestration::ConvergencePolicy {
                strategy: "exhaustive".to_string(),
                min_score: None,
                max_iterations_without_improvement: None,
            },
            max_candidate_failures_per_iteration: 10,
            missing_output_policy: "mark_failed".to_string(),
            iteration_failure_policy: "fail_execution".to_string(),
        },
        evaluation: void_control::orchestration::EvaluationConfig {
            scoring_type: "weighted_metrics".to_string(),
            weights: BTreeMap::from([
                ("latency_p99_ms".to_string(), -0.6),
                ("cost_usd".to_string(), -0.4),
            ]),
            pass_threshold: Some(0.7),
            ranking: "highest_score".to_string(),
            tie_breaking: "cost_usd".to_string(),
        },
        variation: VariationConfig::explicit(
            2,
            vec![
                VariationProposal {
                    overrides: BTreeMap::from([("agent.prompt".to_string(), "a".to_string())]),
                },
                VariationProposal {
                    overrides: BTreeMap::from([("agent.prompt".to_string(), "b".to_string())]),
                },
            ],
        ),
        swarm: true,
    }
}

fn temp_store_dir(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("void-control-dry-run-{label}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
