#![cfg(feature = "serde")]

use void_control::runtime::{
    MockRuntime, SandboxCreateRequest, SandboxExecKind, SandboxExecRequest, SandboxRuntime,
    SandboxState,
};
use void_control::sandbox;

fn temp_root(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("void-control-sandbox-{label}-{nanos}"))
}

#[test]
fn sandbox_api_module_is_exposed() {
    let _ = std::any::type_name::<sandbox::SandboxModuleMarker>();
}

#[test]
fn sandbox_schema_parses_sandbox_shape() {
    let yaml = r#"
api_version: v1
kind: sandbox

metadata:
  name: python-benchmark-box
  labels:
    workload: benchmark
    language: python

runtime:
  image: python:3.12-slim
  cpus: 2
  memory_mb: 2048
  network: true
  env:
    FOO: bar
  mounts:
    - host: /data/fixtures
      guest: /workspace/fixtures
      mode: ro
  ports:
    - 3000

snapshot:
  restore_from: snapshot-transform-v1

lifecycle:
  auto_remove: false
  detach: true
  idle_timeout_secs: 900
  prewarm: true

identity:
  reusable: true
  pool: benchmark-python
"#;

    let sandbox = sandbox::parse_sandbox_yaml(yaml).expect("parse sandbox");

    assert_eq!(sandbox.api_version, "v1");
    assert_eq!(sandbox.kind, "sandbox");
    assert_eq!(
        sandbox.metadata.as_ref().and_then(|m| m.name.as_deref()),
        Some("python-benchmark-box")
    );
    assert_eq!(sandbox.runtime.image, "python:3.12-slim");
    assert_eq!(sandbox.runtime.cpus, 2);
    assert_eq!(sandbox.runtime.memory_mb, 2048);
    assert_eq!(sandbox.runtime.ports, vec![3000]);
    assert_eq!(
        sandbox
            .snapshot
            .as_ref()
            .and_then(|s| s.restore_from.as_deref()),
        Some("snapshot-transform-v1")
    );
    assert_eq!(
        sandbox.identity.as_ref().and_then(|i| i.pool.as_deref()),
        Some("benchmark-python")
    );
}

#[test]
fn sandbox_schema_rejects_missing_runtime() {
    let yaml = r#"
api_version: v1
kind: sandbox
"#;

    let err = sandbox::parse_sandbox_yaml(yaml).expect_err("sandbox should fail");
    assert!(
        err.to_string().contains("missing field `runtime`")
            || err.to_string().contains("runtime is required"),
        "unexpected error: {err}"
    );
}

#[test]
fn sandbox_schema_rejects_invalid_lifecycle_values() {
    let yaml = r#"
api_version: v1
kind: sandbox

runtime:
  image: python:3.12-slim
  cpus: 2
  memory_mb: 2048

lifecycle:
  idle_timeout_secs: 0
"#;

    let err = sandbox::parse_sandbox_yaml(yaml).expect_err("sandbox should fail");
    assert!(
        err.to_string()
            .contains("lifecycle.idle_timeout_secs must be positive"),
        "unexpected error: {err}"
    );
}

#[test]
fn checked_in_compute_examples_parse() {
    let sandbox_spec = std::fs::read_to_string("examples/compute/sandbox-python.yaml")
        .expect("read sandbox example");
    let snapshot_spec = std::fs::read_to_string("examples/compute/snapshot-from-sandbox.yaml")
        .expect("read snapshot example");
    let pool_spec =
        std::fs::read_to_string("examples/compute/pool-python.yaml").expect("read pool example");

    let sandbox = sandbox::parse_sandbox_yaml(&sandbox_spec).expect("parse sandbox example");
    let snapshot = sandbox::parse_snapshot_yaml(&snapshot_spec).expect("parse snapshot example");
    let pool = sandbox::parse_pool_yaml(&pool_spec).expect("parse pool example");

    assert_eq!(sandbox.kind, "sandbox");
    assert_eq!(sandbox.runtime.image, "python:3.12-slim");
    assert_eq!(snapshot.kind, "snapshot");
    assert_eq!(snapshot.source.sandbox_id, "sbx-example");
    assert_eq!(pool.kind, "sandbox_pool");
    assert_eq!(pool.capacity.warm, 5);
}

#[test]
fn snapshot_schema_rejects_invalid_distribution_mode() {
    let json = r#"
{
  "api_version": "v1",
  "kind": "snapshot",
  "metadata": {
    "name": "snapshot-transform-v1"
  },
  "source": {
    "sandbox_id": "sbx-123"
  },
  "distribution": {
    "mode": "broadcast",
    "targets": ["node-a", "node-b"]
  }
}
"#;

    let err = sandbox::parse_snapshot_json(json).expect_err("snapshot should fail");
    assert!(
        err.to_string()
            .contains("distribution.mode must be one of cached, copy"),
        "unexpected error: {err}"
    );
}

#[test]
fn pool_schema_parses_pool_shape() {
    let json = r#"
{
  "api_version": "v1",
  "kind": "sandbox_pool",
  "metadata": {
    "name": "benchmark-python-pool"
  },
  "sandbox_spec": {
    "runtime": {
      "image": "python:3.12-slim",
      "cpus": 2,
      "memory_mb": 2048
    },
    "snapshot": {
      "restore_from": "snapshot-transform-v1"
    },
    "lifecycle": {
      "prewarm": true,
      "idle_timeout_secs": 900
    },
    "identity": {
      "reusable": true,
      "pool": "benchmark-python"
    }
  },
  "capacity": {
    "warm": 5,
    "max": 20
  }
}
"#;

    let pool = sandbox::parse_pool_json(json).expect("parse pool");

    assert_eq!(pool.kind, "sandbox_pool");
    assert_eq!(pool.capacity.warm, 5);
    assert_eq!(pool.capacity.max, 20);
    assert_eq!(pool.sandbox_spec.runtime.image, "python:3.12-slim");
    assert_eq!(
        pool.sandbox_spec
            .snapshot
            .as_ref()
            .and_then(|s| s.restore_from.as_deref()),
        Some("snapshot-transform-v1")
    );
}

#[test]
fn mock_runtime_manages_sandbox_lifecycle() {
    let yaml = r#"
api_version: v1
kind: sandbox

runtime:
  image: python:3.12-slim
  cpus: 2
  memory_mb: 2048

snapshot:
  restore_from: snapshot-transform-v1
"#;
    let sandbox_spec = sandbox::parse_sandbox_yaml(yaml).expect("parse sandbox");
    let mut runtime = MockRuntime::new();

    let created = runtime
        .create_sandbox(SandboxCreateRequest {
            sandbox_id: "sbx-lifecycle".to_string(),
            spec: sandbox_spec,
        })
        .expect("create sandbox");

    assert_eq!(created.sandbox_id, "sbx-lifecycle");
    assert_eq!(created.state, SandboxState::Running);
    assert_eq!(
        created.restore_from_snapshot.as_deref(),
        Some("snapshot-transform-v1")
    );

    let listed = runtime.list_sandboxes().expect("list sandboxes");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].sandbox_id, "sbx-lifecycle");

    let inspected = runtime
        .inspect_sandbox("sbx-lifecycle")
        .expect("inspect sandbox");
    assert_eq!(inspected.state, SandboxState::Running);

    let stopped = runtime.stop_sandbox("sbx-lifecycle").expect("stop sandbox");
    assert_eq!(stopped.state, SandboxState::Stopped);

    runtime
        .delete_sandbox("sbx-lifecycle")
        .expect("delete sandbox");
    let err = runtime
        .inspect_sandbox("sbx-lifecycle")
        .expect_err("sandbox should be deleted");
    assert_eq!(
        err.code,
        void_control::contract::ContractErrorCode::NotFound
    );
}

#[test]
fn mock_runtime_executes_sandbox_requests() {
    let yaml = r#"
api_version: v1
kind: sandbox

runtime:
  image: python:3.12-slim
  cpus: 2
  memory_mb: 2048
"#;
    let sandbox_spec = sandbox::parse_sandbox_yaml(yaml).expect("parse sandbox");
    let mut runtime = MockRuntime::new();
    runtime
        .create_sandbox(SandboxCreateRequest {
            sandbox_id: "sbx-exec".to_string(),
            spec: sandbox_spec,
        })
        .expect("create sandbox");

    let result = runtime
        .exec_sandbox(SandboxExecRequest {
            sandbox_id: "sbx-exec".to_string(),
            kind: SandboxExecKind::Command,
            command: Some(vec!["python3".to_string(), "-V".to_string()]),
            runtime: None,
            code: None,
        })
        .expect("exec sandbox");

    assert_eq!(result.exit_code, 0);
    assert!(
        result.stdout.contains("python3 -V"),
        "unexpected stdout: {result:?}"
    );
}

#[test]
fn sandbox_bridge_create_list_get_stop_exec_and_delete_round_trip() {
    let root = temp_root("bridge");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    let body = serde_json::json!({
        "api_version": "v1",
        "kind": "sandbox",
        "metadata": {
            "name": "python-benchmark-box"
        },
        "runtime": {
            "image": "python:3.12-slim",
            "cpus": 2,
            "memory_mb": 2048
        },
        "snapshot": {
            "restore_from": "snapshot-transform-v1"
        }
    })
    .to_string();

    let created = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/sandboxes",
        Some(&body),
        &spec_dir,
        &execution_dir,
    )
    .expect("create response");
    assert_eq!(created.status, 200);
    assert_eq!(created.json["kind"], "sandbox");
    let sandbox_id = created.json["sandbox"]["sandbox_id"]
        .as_str()
        .expect("sandbox id")
        .to_string();
    assert_eq!(
        created.json["sandbox"]["restore_from_snapshot"],
        "snapshot-transform-v1"
    );

    let listed = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "GET",
        "/v1/sandboxes",
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("list response");
    assert_eq!(listed.status, 200);
    assert_eq!(listed.json["kind"], "sandbox_list");
    assert_eq!(
        listed.json["sandboxes"].as_array().map(|items| items.len()),
        Some(1)
    );

    let fetched = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "GET",
        &format!("/v1/sandboxes/{sandbox_id}"),
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("get response");
    assert_eq!(fetched.status, 200);
    assert_eq!(fetched.json["kind"], "sandbox");
    assert_eq!(fetched.json["sandbox"]["sandbox_id"], sandbox_id);
    assert_eq!(fetched.json["sandbox"]["state"], "running");

    let exec_body = serde_json::json!({
        "kind": "command",
        "command": ["python3", "-V"]
    })
    .to_string();
    let exec = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        &format!("/v1/sandboxes/{sandbox_id}/exec"),
        Some(&exec_body),
        &spec_dir,
        &execution_dir,
    )
    .expect("exec response");
    assert_eq!(exec.status, 200);
    assert_eq!(exec.json["kind"], "sandbox_exec");
    assert_eq!(exec.json["result"]["exit_code"], 0);
    assert_eq!(exec.json["result"]["stdout"], "python3 -V");

    let stopped = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        &format!("/v1/sandboxes/{sandbox_id}/stop"),
        Some("{}"),
        &spec_dir,
        &execution_dir,
    )
    .expect("stop response");
    assert_eq!(stopped.status, 200);
    assert_eq!(stopped.json["sandbox"]["state"], "stopped");

    let deleted = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "DELETE",
        &format!("/v1/sandboxes/{sandbox_id}"),
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("delete response");
    assert_eq!(deleted.status, 200);
    assert_eq!(deleted.json["kind"], "sandbox_deleted");
    assert_eq!(deleted.json["sandbox_id"], sandbox_id);

    let missing = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "GET",
        &format!("/v1/sandboxes/{sandbox_id}"),
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("missing response");
    assert_eq!(missing.status, 404);
}

#[test]
fn snapshot_bridge_create_list_get_replicate_and_delete_round_trip() {
    let root = temp_root("snapshot-bridge");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    let body = serde_json::json!({
        "api_version": "v1",
        "kind": "snapshot",
        "metadata": {
            "name": "snapshot-transform-v1"
        },
        "source": {
            "sandbox_id": "sbx-123"
        },
        "distribution": {
            "mode": "cached",
            "targets": ["node-a"]
        }
    })
    .to_string();

    let created = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/snapshots",
        Some(&body),
        &spec_dir,
        &execution_dir,
    )
    .expect("create response");
    assert_eq!(created.status, 200);
    assert_eq!(created.json["kind"], "snapshot");
    let snapshot_id = created.json["snapshot"]["snapshot_id"]
        .as_str()
        .expect("snapshot id")
        .to_string();
    assert_eq!(created.json["snapshot"]["source_sandbox_id"], "sbx-123");

    let listed = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "GET",
        "/v1/snapshots",
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("list response");
    assert_eq!(listed.status, 200);
    assert_eq!(listed.json["kind"], "snapshot_list");
    assert_eq!(
        listed.json["snapshots"].as_array().map(|items| items.len()),
        Some(1)
    );

    let fetched = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "GET",
        &format!("/v1/snapshots/{snapshot_id}"),
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("get response");
    assert_eq!(fetched.status, 200);
    assert_eq!(fetched.json["snapshot"]["snapshot_id"], snapshot_id);

    let replicate_body = serde_json::json!({
        "mode": "copy",
        "targets": ["node-a", "node-b", "node-c"]
    })
    .to_string();
    let replicated = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        &format!("/v1/snapshots/{snapshot_id}/replicate"),
        Some(&replicate_body),
        &spec_dir,
        &execution_dir,
    )
    .expect("replicate response");
    assert_eq!(replicated.status, 200);
    assert_eq!(replicated.json["kind"], "snapshot");
    assert_eq!(replicated.json["snapshot"]["distribution"]["mode"], "copy");
    assert_eq!(
        replicated.json["snapshot"]["distribution"]["targets"]
            .as_array()
            .map(|items| items.len()),
        Some(3)
    );

    let deleted = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "DELETE",
        &format!("/v1/snapshots/{snapshot_id}"),
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("delete response");
    assert_eq!(deleted.status, 200);
    assert_eq!(deleted.json["kind"], "snapshot_deleted");
    assert_eq!(deleted.json["snapshot_id"], snapshot_id);
}

#[test]
fn pool_bridge_create_get_and_scale_round_trip() {
    let root = temp_root("pool-bridge");
    let spec_dir = root.join("specs");
    let execution_dir = root.join("executions");
    let body = serde_json::json!({
        "api_version": "v1",
        "kind": "sandbox_pool",
        "metadata": {
            "name": "benchmark-python-pool"
        },
        "sandbox_spec": {
            "runtime": {
                "image": "python:3.12-slim",
                "cpus": 2,
                "memory_mb": 2048
            },
            "snapshot": {
                "restore_from": "snapshot-transform-v1"
            },
            "lifecycle": {
                "prewarm": true,
                "idle_timeout_secs": 900
            },
            "identity": {
                "reusable": true,
                "pool": "benchmark-python"
            }
        },
        "capacity": {
            "warm": 5,
            "max": 20
        }
    })
    .to_string();

    let created = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/pools",
        Some(&body),
        &spec_dir,
        &execution_dir,
    )
    .expect("create response");
    assert_eq!(created.status, 200);
    assert_eq!(created.json["kind"], "pool");
    let pool_id = created.json["pool"]["pool_id"]
        .as_str()
        .expect("pool id")
        .to_string();
    assert_eq!(created.json["pool"]["capacity"]["warm"], 5);
    assert_eq!(created.json["pool"]["capacity"]["max"], 20);

    let fetched = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "GET",
        &format!("/v1/pools/{pool_id}"),
        None,
        &spec_dir,
        &execution_dir,
    )
    .expect("get response");
    assert_eq!(fetched.status, 200);
    assert_eq!(fetched.json["kind"], "pool");
    assert_eq!(fetched.json["pool"]["pool_id"], pool_id);
    assert_eq!(
        fetched.json["pool"]["sandbox_spec"]["snapshot"]["restore_from"],
        "snapshot-transform-v1"
    );

    let scale_body = serde_json::json!({
        "warm": 8,
        "max": 24
    })
    .to_string();
    let scaled = void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        &format!("/v1/pools/{pool_id}/scale"),
        Some(&scale_body),
        &spec_dir,
        &execution_dir,
    )
    .expect("scale response");
    assert_eq!(scaled.status, 200);
    assert_eq!(scaled.json["kind"], "pool");
    assert_eq!(scaled.json["pool"]["capacity"]["warm"], 8);
    assert_eq!(scaled.json["pool"]["capacity"]["max"], 24);
}
