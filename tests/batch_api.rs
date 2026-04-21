#![cfg(feature = "serde")]

use void_control::batch;

#[test]
fn batch_api_module_is_exposed() {
    let _ = std::any::type_name::<batch::BatchModuleMarker>();
}

#[test]
fn batch_schema_parses_batch_shape() {
    let yaml = r#"
api_version: v1
kind: batch

metadata:
  name: repo-background-work

worker:
  template: coder-agent
  provider: claude

mode:
  parallelism: 3
  background: true
  interaction: none

jobs:
  - name: auth
    prompt: Fix failing auth tests
  - name: logging
    prompt: Improve retry logging
"#;

    let batch = batch::parse_batch_yaml(yaml).expect("parse batch");
    assert_eq!(batch.api_version, "v1");
    assert_eq!(batch.kind, "batch");
    assert_eq!(
        batch.metadata.as_ref().and_then(|m| m.name.as_deref()),
        Some("repo-background-work")
    );
    assert_eq!(batch.worker.template, "coder-agent");
    assert_eq!(batch.worker.provider.as_deref(), Some("claude"));
    assert_eq!(batch.mode.as_ref().and_then(|m| m.parallelism), Some(3));
    assert_eq!(
        batch.mode.as_ref().and_then(|m| m.interaction.as_deref()),
        Some("none")
    );
    assert_eq!(batch.jobs.len(), 2);
    assert_eq!(batch.jobs[0].prompt, "Fix failing auth tests");
}

#[test]
fn batch_schema_normalizes_yolo_alias_to_batch() {
    let json = r#"
{
  "api_version": "v1",
  "kind": "yolo",
  "worker": {
    "template": "coder-agent"
  },
  "jobs": [
    {
      "prompt": "Review migration safety"
    }
  ]
}
"#;

    let batch = batch::parse_batch_json(json).expect("parse yolo alias");
    assert_eq!(batch.kind, "batch");
    assert_eq!(batch.worker.template, "coder-agent");
    assert_eq!(batch.jobs.len(), 1);
    assert_eq!(batch.jobs[0].prompt, "Review migration safety");
}

#[test]
fn batch_schema_rejects_missing_jobs() {
    let yaml = r#"
api_version: v1
kind: batch

worker:
  template: coder-agent

jobs: []
"#;

    let err = batch::parse_batch_yaml(yaml).expect_err("batch should fail");
    assert!(
        err.to_string().contains("jobs must not be empty"),
        "unexpected error: {err}"
    );
}

#[test]
fn batch_compile_builds_swarm_execution_spec() {
    let yaml = r#"
api_version: v1
kind: batch

worker:
  template: examples/runtime-templates/warm_agent_basic.yaml
  provider: claude

mode:
  parallelism: 2

jobs:
  - name: auth
    prompt: Fix failing auth tests
  - name: logging
    prompt: Improve retry logging
  - name: migrations
    prompt: Review DB migration safety
"#;

    let batch = batch::parse_batch_yaml(yaml).expect("parse batch");
    let execution = batch::compile_batch_spec(&batch).expect("compile batch");

    assert_eq!(execution.mode, "swarm");
    assert!(execution.swarm);
    assert_eq!(
        execution.workflow.template,
        "examples/runtime-templates/warm_agent_basic.yaml"
    );
    assert_eq!(execution.variation.source, "explicit");
    assert_eq!(execution.variation.explicit.len(), 3);
    assert_eq!(execution.variation.candidates_per_iteration, 2);
    assert_eq!(execution.policy.concurrency.max_concurrent_candidates, 2);
    assert_eq!(
        execution.variation.explicit[0]
            .overrides
            .get("agent.prompt")
            .map(String::as_str),
        Some("Fix failing auth tests")
    );
    assert_eq!(
        execution.variation.explicit[1]
            .overrides
            .get("agent.prompt")
            .map(String::as_str),
        Some("Improve retry logging")
    );
    assert_eq!(
        execution.variation.explicit[2]
            .overrides
            .get("agent.prompt")
            .map(String::as_str),
        Some("Review DB migration safety")
    );
    for proposal in &execution.variation.explicit {
        assert_eq!(
            proposal.overrides.get("llm.provider").map(String::as_str),
            Some("claude")
        );
    }
}

#[test]
fn batch_compile_uses_job_count_when_parallelism_is_omitted() {
    let yaml = r#"
api_version: v1
kind: yolo

worker:
  template: examples/runtime-templates/warm_agent_basic.yaml

jobs:
  - prompt: Fix failing auth tests
  - prompt: Improve retry logging
"#;

    let batch = batch::parse_batch_yaml(yaml).expect("parse batch");
    let execution = batch::compile_batch_spec(&batch).expect("compile batch");

    assert_eq!(batch.kind, "batch");
    assert_eq!(execution.variation.explicit.len(), 2);
    assert_eq!(execution.variation.candidates_per_iteration, 2);
    assert_eq!(execution.policy.concurrency.max_concurrent_candidates, 2);
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("void-control-batch-{label}-{nanos}"))
}

#[test]
fn batch_bridge_dry_run_returns_compiled_preview() {
    let body = serde_json::json!({
        "api_version": "v1",
        "kind": "batch",
        "worker": {
            "template": "examples/runtime-templates/warm_agent_basic.yaml",
            "provider": "claude"
        },
        "mode": {
            "parallelism": 2
        },
        "jobs": [
            { "name": "auth", "prompt": "Fix failing auth tests" },
            { "name": "logging", "prompt": "Improve retry logging" },
            { "name": "migrations", "prompt": "Review DB migration safety" }
        ]
    })
    .to_string();

    let response = void_control::bridge::handle_bridge_request_for_test(
        "POST",
        "/v1/batch/dry-run",
        Some(&body),
    )
    .expect("response");

    assert_eq!(response.status, 200);
    assert_eq!(response.json["kind"], "batch");
    assert_eq!(response.json["compiled_primitive"], "swarm");
    assert_eq!(response.json["compiled"]["variation_source"], "explicit");
    assert_eq!(response.json["compiled"]["candidates_per_iteration"], 2);
    assert_eq!(
        response.json["compiled"]["candidate_overrides"]
            .as_array()
            .map(|items| items.len()),
        Some(3)
    );
}

#[test]
fn batch_bridge_run_creates_normal_execution() {
    let root = temp_root("run");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    let body = serde_json::json!({
        "api_version": "v1",
        "kind": "batch",
        "worker": {
            "template": "examples/runtime-templates/warm_agent_basic.yaml"
        },
        "jobs": [
            { "name": "auth", "prompt": "Fix failing auth tests" },
            { "name": "logging", "prompt": "Improve retry logging" }
        ]
    })
    .to_string();

    let response = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/batch/run",
        Some(&body),
        &spec_dir,
        &execution_dir,
    )
    .expect("response");

    assert_eq!(response.status, 200);
    assert_eq!(response.json["kind"], "batch");
    assert_eq!(response.json["compiled_primitive"], "swarm");
    assert_eq!(response.json["status"], "Pending");

    let execution_id = response.json["execution_id"]
        .as_str()
        .expect("execution_id");
    let store = void_control::orchestration::FsExecutionStore::new(execution_dir.clone());
    let spec = store.load_spec(execution_id).expect("load compiled spec");
    assert_eq!(spec.mode, "swarm");
    assert_eq!(spec.variation.explicit.len(), 2);

    let inspect = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "GET",
        &format!("/v1/batch-runs/{execution_id}"),
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("inspect");

    assert_eq!(inspect.status, 200);
    assert_eq!(inspect.json["kind"], "batch");
    assert_eq!(inspect.json["run_id"], execution_id);
    assert_eq!(inspect.json["execution"]["execution_id"], execution_id);
}

#[test]
fn yolo_bridge_alias_runs_as_batch() {
    let root = temp_root("yolo");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    let body = serde_json::json!({
        "api_version": "v1",
        "kind": "yolo",
        "worker": {
            "template": "examples/runtime-templates/warm_agent_basic.yaml"
        },
        "jobs": [
            { "prompt": "Review migration safety" }
        ]
    })
    .to_string();

    let response = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/yolo/run",
        Some(&body),
        &spec_dir,
        &execution_dir,
    )
    .expect("response");

    assert_eq!(response.status, 200);
    assert_eq!(response.json["kind"], "batch");
    assert_eq!(response.json["compiled_primitive"], "swarm");
}

#[test]
fn batch_example_file_parses_and_compiles() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples/batch/background_repo_work.yaml");
    let yaml = std::fs::read_to_string(&path).expect("read example batch spec");
    let batch = batch::parse_batch_yaml(&yaml).expect("parse example");
    let execution = batch::compile_batch_spec(&batch).expect("compile example");

    assert_eq!(batch.kind, "batch");
    assert_eq!(execution.mode, "swarm");
    assert!(!execution.variation.explicit.is_empty());
}
