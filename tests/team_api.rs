#![cfg(feature = "serde")]

use std::path::{Path, PathBuf};

async fn submit_team_dry_run(spec: &str) -> void_control::bridge::TestBridgeResponse {
    void_control::bridge::handle_bridge_request_for_test("POST", "/v1/teams/dry-run", Some(spec))
        .await
        .expect("team dry-run response")
}

async fn submit_team_run(spec: &str) -> void_control::bridge::TestBridgeResponse {
    void_control::bridge::handle_bridge_request_for_test("POST", "/v1/teams/run", Some(spec))
        .await
        .expect("team run response")
}

fn temp_bridge_root(name: &str) -> PathBuf {
    let mut root = std::env::temp_dir();
    root.push(format!(
        "void-control-team-test-{}-{name}",
        std::process::id()
    ));
    root
}

async fn submit_team_run_with_root(
    spec: &str,
    root: &Path,
) -> void_control::bridge::TestBridgeResponse {
    void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/teams/run",
        Some(spec),
        &root.join("specs"),
        &root.join("executions"),
    )
    .await
    .expect("team run response")
}

async fn fetch_team_run_with_root(
    execution_id: &str,
    root: &Path,
) -> void_control::bridge::TestBridgeResponse {
    let path = format!("/v1/team-runs/{execution_id}");
    void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "GET",
        &path,
        None,
        &root.join("specs"),
        &root.join("executions"),
    )
    .await
    .expect("team get response")
}

#[tokio::test]
async fn team_dry_run_rejects_missing_agents() {
    let spec = r#"
api_version: v1
kind: team
tasks:
  - name: write
    description: Write the article
process:
  type: sequential
"#;

    let response = submit_team_dry_run(spec).await;

    assert_eq!(response.status, 400);
    assert!(
        response
            .json
            .get("message")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .contains("team spec must include at least one agent"),
        "unexpected response: {response:?}"
    );
}

#[tokio::test]
async fn team_dry_run_compiles_parallel_process_to_swarm() {
    let spec = r#"
api_version: v1
kind: team
agents:
  - name: researcher
    role: Researcher
    goal: Find information
tasks:
  - name: research
    description: Gather evidence
    agent: researcher
process:
  type: parallel
"#;

    let response = submit_team_dry_run(spec).await;

    assert_eq!(response.status, 200);
    assert_eq!(response.json["kind"], "team");
    assert_eq!(response.json["compiled_primitive"], "swarm");
}

#[tokio::test]
async fn team_dry_run_compiles_sequential_process_one_task_per_iteration() {
    let spec = r#"
api_version: v1
kind: team
metadata:
  name: rust-article-team
agents:
  - name: researcher
    role: Researcher
    goal: Find information
tasks:
  - name: research
    description: Gather evidence
    agent: researcher
  - name: write
    description: Draft the article
    agent: researcher
process:
  type: sequential
"#;

    let response = submit_team_dry_run(spec).await;

    assert_eq!(response.status, 200);
    assert_eq!(response.json["compiled"]["candidates_per_iteration"], 1);
    assert_eq!(
        response.json["compiled"]["candidate_overrides"]
            .as_array()
            .expect("candidate_overrides")
            .len(),
        2
    );
}

#[tokio::test]
async fn team_dry_run_rejects_conflicting_agent_templates() {
    let spec = r#"
api_version: v1
kind: team
agents:
  - name: researcher
    role: Researcher
    goal: Find information
    template: examples/runtime-templates/warm_agent_basic.yaml
  - name: writer
    role: Writer
    goal: Draft the article
    template: examples/runtime-templates/transform_optimizer_agent.yaml
tasks:
  - name: research
    description: Gather evidence
    agent: researcher
  - name: write
    description: Draft the article
    agent: writer
process:
  type: parallel
"#;

    let response = submit_team_dry_run(spec).await;

    assert_eq!(response.status, 400);
    assert!(
        response
            .json
            .get("message")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .contains("team agents must share the same template"),
        "unexpected response: {response:?}"
    );
}

#[tokio::test]
async fn team_dry_run_requires_explicit_lead_for_lead_worker() {
    let spec = r#"
api_version: v1
kind: team
agents:
  - name: reviewer
    role: Reviewer
    goal: Review work
  - name: worker
    role: Worker
    goal: Produce draft
tasks:
  - name: review
    description: Review the draft
process:
  type: lead_worker
"#;

    let response = submit_team_dry_run(spec).await;

    assert_eq!(response.status, 400);
    assert!(
        response
            .json
            .get("message")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .contains("process.lead is required for lead_worker teams"),
        "unexpected response: {response:?}"
    );
}

#[tokio::test]
async fn team_dry_run_parallel_without_task_agent_fans_out_across_agents() {
    let spec = r#"
api_version: v1
kind: team
agents:
  - name: researcher
    role: Researcher
    goal: Find information
  - name: writer
    role: Writer
    goal: Draft the article
tasks:
  - name: article
    description: Produce the article
process:
  type: parallel
"#;

    let response = submit_team_dry_run(spec).await;

    assert_eq!(response.status, 200);
    assert_eq!(response.json["compiled"]["candidates_per_iteration"], 2);
}

#[tokio::test]
async fn team_dry_run_rejects_depends_on_in_phase_one() {
    let spec = r#"
api_version: v1
kind: team
agents:
  - name: researcher
    role: Researcher
    goal: Find information
tasks:
  - name: research
    description: Gather evidence
    agent: researcher
  - name: write
    description: Draft the article
    agent: researcher
    depends_on:
      - research
process:
  type: sequential
"#;

    let response = submit_team_dry_run(spec).await;

    assert_eq!(response.status, 400);
    assert!(
        response
            .json
            .get("message")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .contains("phase1 team spec does not support depends_on"),
        "unexpected response: {response:?}"
    );
}

#[tokio::test]
async fn team_run_returns_execution_summary() {
    let spec = r#"
api_version: v1
kind: team
metadata:
  name: rust-article-team
agents:
  - name: researcher
    role: Researcher
    goal: Find information
tasks:
  - name: research
    description: Gather evidence
    agent: researcher
process:
  type: parallel
"#;

    let response = submit_team_run(spec).await;

    assert_eq!(response.status, 200);
    assert_eq!(response.json["kind"], "team");
    assert!(response.json.get("execution_id").is_some());
}

#[tokio::test]
async fn team_run_get_wraps_execution_detail() {
    let spec = r#"
api_version: v1
kind: team
metadata:
  name: rust-article-team
agents:
  - name: researcher
    role: Researcher
    goal: Find information
tasks:
  - name: research
    description: Gather evidence
    agent: researcher
process:
  type: parallel
"#;

    let root = temp_bridge_root("roundtrip");
    let started = submit_team_run_with_root(spec, &root).await;
    let execution_id = started
        .json
        .get("execution_id")
        .and_then(|value| value.as_str())
        .expect("execution_id");

    let response = fetch_team_run_with_root(execution_id, &root).await;

    assert_eq!(response.status, 200);
    assert_eq!(response.json["kind"], "team");
    assert_eq!(response.json["run_id"], execution_id);
    assert_eq!(response.json["execution"]["execution_id"], execution_id);
}
