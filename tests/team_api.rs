#![cfg(feature = "serde")]

use std::path::PathBuf;

fn submit_team_dry_run(spec: &str) -> void_control::bridge::TestBridgeResponse {
    void_control::bridge::handle_bridge_request_for_test("POST", "/v1/teams/dry-run", Some(spec))
        .expect("team dry-run response")
}

fn submit_team_run(spec: &str) -> void_control::bridge::TestBridgeResponse {
    void_control::bridge::handle_bridge_request_for_test("POST", "/v1/teams/run", Some(spec))
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

fn submit_team_run_with_root(
    spec: &str,
    root: &PathBuf,
) -> void_control::bridge::TestBridgeResponse {
    void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "POST",
        "/v1/teams/run",
        Some(spec),
        &root.join("specs"),
        &root.join("executions"),
    )
    .expect("team run response")
}

fn fetch_team_run_with_root(
    execution_id: &str,
    root: &PathBuf,
) -> void_control::bridge::TestBridgeResponse {
    let path = format!("/v1/team-runs/{execution_id}");
    void_control::bridge::handle_bridge_request_with_dirs_for_test(
        "GET",
        &path,
        None,
        &root.join("specs"),
        &root.join("executions"),
    )
    .expect("team get response")
}

#[test]
fn team_dry_run_rejects_missing_agents() {
    let spec = r#"
api_version: v1
kind: team
tasks:
  - name: write
    description: Write the article
process:
  type: sequential
"#;

    let response = submit_team_dry_run(spec);

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

#[test]
fn team_dry_run_compiles_parallel_process_to_swarm() {
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

    let response = submit_team_dry_run(spec);

    assert_eq!(response.status, 200);
    assert_eq!(response.json["kind"], "team");
    assert_eq!(response.json["compiled_primitive"], "swarm");
}

#[test]
fn team_run_returns_execution_summary() {
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

    let response = submit_team_run(spec);

    assert_eq!(response.status, 200);
    assert_eq!(response.json["kind"], "team");
    assert!(response.json.get("execution_id").is_some());
}

#[test]
fn team_run_get_wraps_execution_detail() {
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
    let started = submit_team_run_with_root(spec, &root);
    let execution_id = started
        .json
        .get("execution_id")
        .and_then(|value| value.as_str())
        .expect("execution_id");

    let response = fetch_team_run_with_root(execution_id, &root);

    assert_eq!(response.status, 200);
    assert_eq!(response.json["kind"], "team");
    assert_eq!(response.json["run_id"], execution_id);
    assert_eq!(response.json["execution"]["execution_id"], execution_id);
}
