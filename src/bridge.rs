#[cfg(feature = "serde")]
use std::fs::{self, OpenOptions};
#[cfg(feature = "serde")]
use std::io::Write;
#[cfg(feature = "serde")]
use std::path::{Path, PathBuf};
#[cfg(feature = "serde")]
use std::thread;
#[cfg(feature = "serde")]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "serde")]
use serde_json::{json, Value};

#[cfg(feature = "serde")]
use crate::batch;
#[cfg(feature = "serde")]
use crate::contract::{ExecutionPolicy, RunState, StartRequest};
#[cfg(feature = "serde")]
use crate::orchestration::{
    BudgetPolicy, ConcurrencyPolicy, ConvergencePolicy, EvaluationConfig, ExecutionAction,
    ExecutionRuntime, ExecutionService, ExecutionSpec, FsExecutionStore, GlobalConfig,
    GlobalScheduler, OrchestrationPolicy, PolicyPatch, QueuedCandidate, SupervisionConfig,
    SupervisionReviewPolicy, VariationConfig, VariationProposal, VariationSelection,
    WorkflowTemplateRef,
};
#[cfg(feature = "serde")]
use crate::runtime::{MockRuntime, VoidBoxRuntimeClient};
#[cfg(feature = "serde")]
use crate::team;
#[cfg(feature = "serde")]
use crate::templates;

#[cfg(feature = "serde")]
#[derive(Debug, Serialize)]
struct ExecutionProgressResponse {
    completed_iterations: u32,
    scoring_history_len: u32,
    event_count: usize,
    last_event: Option<String>,
    candidate_queue_count: u32,
    candidate_dispatch_count: u32,
    candidate_output_count: u32,
    queued_candidate_count: u32,
    running_candidate_count: u32,
    completed_candidate_count: u32,
    failed_candidate_count: u32,
    canceled_candidate_count: u32,
    event_type_counts: std::collections::BTreeMap<String, u32>,
}

#[cfg(feature = "serde")]
#[derive(Debug, Serialize)]
struct ExecutionDetailResponse {
    execution: crate::orchestration::Execution,
    progress: ExecutionProgressResponse,
    result: ExecutionResultResponse,
    candidates: Vec<crate::orchestration::ExecutionCandidate>,
}

#[cfg(feature = "serde")]
#[derive(Debug, Serialize)]
struct ExecutionResultResponse {
    best_candidate_id: Option<String>,
    completed_iterations: u32,
    total_candidate_failures: u32,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct LaunchRequest {
    run_id: Option<String>,
    file: Option<String>,
    spec_text: Option<String>,
    spec_format: Option<String>,
    policy: Option<RunPolicyJson>,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct RunPolicyJson {
    max_parallel_microvms_per_run: Option<u32>,
    max_stage_retries: Option<u32>,
    stage_timeout_secs: Option<u32>,
    cancel_grace_period_secs: Option<u32>,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct ExecutionSpecRequest {
    mode: String,
    goal: String,
    workflow: WorkflowTemplateRequest,
    policy: ExecutionPolicyRequest,
    evaluation: EvaluationRequest,
    variation: VariationRequest,
    swarm: bool,
    supervision: Option<SupervisionRequest>,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct WorkflowTemplateRequest {
    template: String,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct ExecutionPolicyRequest {
    budget: BudgetPolicyRequest,
    concurrency: ConcurrencyPolicyRequest,
    convergence: ConvergencePolicyRequest,
    max_candidate_failures_per_iteration: u32,
    missing_output_policy: String,
    iteration_failure_policy: String,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct BudgetPolicyRequest {
    max_iterations: Option<u32>,
    max_child_runs: Option<u32>,
    max_wall_clock_secs: Option<u32>,
    max_cost_usd_millis: Option<u64>,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct ConcurrencyPolicyRequest {
    max_concurrent_candidates: u32,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct ConvergencePolicyRequest {
    strategy: String,
    min_score: Option<f64>,
    max_iterations_without_improvement: Option<u32>,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct EvaluationRequest {
    scoring_type: String,
    weights: std::collections::BTreeMap<String, f64>,
    pass_threshold: Option<f64>,
    ranking: String,
    tie_breaking: String,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct VariationRequest {
    source: String,
    candidates_per_iteration: u32,
    selection: Option<String>,
    parameter_space: Option<std::collections::BTreeMap<String, Vec<String>>>,
    explicit: Option<Vec<VariationProposalRequest>>,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct VariationProposalRequest {
    overrides: std::collections::BTreeMap<String, String>,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct SupervisionRequest {
    supervisor_role: String,
    review_policy: SupervisionReviewPolicyRequest,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct SupervisionReviewPolicyRequest {
    max_revision_rounds: u32,
    retry_on_runtime_failure: bool,
    require_final_approval: bool,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct PolicyPatchRequest {
    budget: Option<PolicyPatchBudgetRequest>,
    concurrency: Option<PolicyPatchConcurrencyRequest>,
    convergence: Option<serde_json::Value>,
    evaluation: Option<serde_json::Value>,
    variation: Option<serde_json::Value>,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct PolicyPatchBudgetRequest {
    max_iterations: Option<u32>,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct PolicyPatchConcurrencyRequest {
    max_concurrent_candidates: Option<u32>,
}

#[cfg(feature = "serde")]
#[derive(Debug, Serialize)]
struct LaunchResponse {
    run_id: String,
    attempt_id: u32,
    state: String,
    file: String,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize)]
struct TemplateRequestBody {
    inputs: Value,
}

#[cfg(feature = "serde")]
#[derive(Debug, Serialize)]
struct ApiError {
    code: &'static str,
    message: String,
    retryable: bool,
}

#[cfg(feature = "serde")]
#[derive(Debug)]
pub struct TestBridgeResponse {
    pub status: u16,
    pub json: Value,
}

#[cfg(feature = "serde")]
pub fn handle_bridge_request_for_test(
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<TestBridgeResponse, String> {
    let root = std::env::temp_dir().join(format!("void-control-bridge-test-{}", now_ms()));
    handle_bridge_request_with_dirs_for_test(
        method,
        path,
        body,
        &root.join("specs"),
        &root.join("executions"),
    )
}

#[cfg(feature = "serde")]
pub fn handle_bridge_request_with_dirs_for_test(
    method: &str,
    path: &str,
    body: Option<&str>,
    spec_dir: &Path,
    execution_dir: &Path,
) -> Result<TestBridgeResponse, String> {
    let response = handle_bridge_request(
        method,
        path,
        body.unwrap_or(""),
        &BridgeConfig {
            listen: "127.0.0.1:0".to_string(),
            base_url: "http://127.0.0.1:43100".to_string(),
            spec_dir: spec_dir.to_path_buf(),
            execution_dir: execution_dir.to_path_buf(),
        },
        None,
    );
    let json = serde_json::from_slice::<Value>(&response.body)
        .unwrap_or_else(|_| json!({"invalid_json": true}));
    Ok(TestBridgeResponse {
        status: response.status,
        json,
    })
}

#[cfg(feature = "serde")]
pub fn run_bridge() -> Result<(), String> {
    use tiny_http::{Method, Response, Server, StatusCode};

    let config = BridgeConfig::from_env();

    for dir in [&config.spec_dir, &config.execution_dir] {
        if let Err(err) = std::fs::create_dir_all(dir) {
            return Err(format!(
                "failed to create bridge storage dir {}: {err}",
                dir.display()
            ));
        }
    }

    let worker_config = config.clone();
    thread::spawn(move || loop {
        let runtime = VoidBoxRuntimeClient::new(worker_config.base_url.clone(), 250);
        if let Err(err) = process_pending_executions_once(
            GlobalConfig {
                max_concurrent_child_runs: 20,
            },
            runtime,
            worker_config.execution_dir.clone(),
        ) {
            eprintln!("bridge worker tick failed: {err}");
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    });
    let server = Server::http(&config.listen)
        .map_err(|e| format!("listen {} failed: {e}", config.listen))?;
    let client = VoidBoxRuntimeClient::new(config.base_url.clone(), 250);
    println!(
        "voidctl bridge listening on http://{} -> {}",
        config.listen, config.base_url
    );

    for mut req in server.incoming_requests() {
        let method = req.method().as_str().to_string();
        let path = req.url().to_string();

        if req.method() == &Method::Options {
            let _ = req.respond(
                Response::empty(204)
                    .with_header(make_header("Access-Control-Allow-Origin", "*"))
                    .with_header(make_header(
                        "Access-Control-Allow-Methods",
                        "GET,POST,OPTIONS",
                    ))
                    .with_header(make_header("Access-Control-Allow-Headers", "Content-Type")),
            );
            continue;
        }

        let mut body = String::new();
        if let Err(e) = req.as_reader().read_to_string(&mut body) {
            let _ = req.respond(to_tiny_response(json_response(
                400,
                &ApiError {
                    code: "INVALID_SPEC",
                    message: format!("failed to read request body: {e}"),
                    retryable: false,
                },
            )));
            continue;
        }

        let response = handle_bridge_request(&method, &path, &body, &config, Some(&client));
        let _ = req.respond(
            Response::from_data(response.body)
                .with_status_code(StatusCode(response.status))
                .with_header(make_header("Content-Type", "application/json"))
                .with_header(make_header("Access-Control-Allow-Origin", "*"))
                .with_header(make_header(
                    "Access-Control-Allow-Methods",
                    "GET,POST,OPTIONS",
                ))
                .with_header(make_header("Access-Control-Allow-Headers", "Content-Type")),
        );
    }

    Ok(())
}

#[cfg(feature = "serde")]
struct BridgeConfig {
    listen: String,
    base_url: String,
    spec_dir: PathBuf,
    execution_dir: PathBuf,
}

#[cfg(feature = "serde")]
impl Clone for BridgeConfig {
    fn clone(&self) -> Self {
        Self {
            listen: self.listen.clone(),
            base_url: self.base_url.clone(),
            spec_dir: self.spec_dir.clone(),
            execution_dir: self.execution_dir.clone(),
        }
    }
}

#[cfg(feature = "serde")]
impl BridgeConfig {
    fn from_env() -> Self {
        let listen = std::env::var("VOID_CONTROL_BRIDGE_LISTEN")
            .unwrap_or_else(|_| "127.0.0.1:43210".to_string());
        let base_url = std::env::var("VOID_BOX_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:43100".to_string());
        let spec_dir = std::env::var("VOID_CONTROL_SPEC_DIR")
            .unwrap_or_else(|_| "/tmp/void-control/specs".to_string());
        let execution_dir = std::env::var("VOID_CONTROL_EXECUTION_DIR")
            .unwrap_or_else(|_| "/tmp/void-control/executions".to_string());
        Self {
            listen,
            base_url,
            spec_dir: PathBuf::from(spec_dir),
            execution_dir: PathBuf::from(execution_dir),
        }
    }
}

#[cfg(feature = "serde")]
struct JsonHttpResponse {
    status: u16,
    body: Vec<u8>,
}

#[cfg(feature = "serde")]
fn handle_bridge_request(
    method: &str,
    path: &str,
    body: &str,
    config: &BridgeConfig,
    client: Option<&VoidBoxRuntimeClient>,
) -> JsonHttpResponse {
    if method == "GET" && path == "/v1/health" {
        return json_response(200, &json!({"status":"ok","service":"voidctl-bridge"}));
    }

    if method == "POST" && path == "/v1/executions/dry-run" {
        return handle_execution_dry_run(body);
    }

    if method == "POST" && (path == "/v1/batch/dry-run" || path == "/v1/yolo/dry-run") {
        return handle_batch_dry_run(body);
    }

    if method == "POST" && path == "/v1/teams/dry-run" {
        return handle_team_dry_run(body);
    }

    if method == "POST" && path == "/v1/teams/run" {
        return handle_team_run(body, config);
    }

    if method == "GET" && path.starts_with("/v1/team-runs/") {
        return handle_team_get(path, config);
    }

    if method == "POST" && (path == "/v1/batch/run" || path == "/v1/yolo/run") {
        return handle_batch_run(body, config);
    }

    if method == "GET" && path.starts_with("/v1/batch-runs/") {
        return handle_batch_get(path, config);
    }

    if method == "GET" && path.starts_with("/v1/yolo-runs/") {
        return handle_batch_get(path, config);
    }

    if method == "GET" && path == "/v1/templates" {
        return handle_template_list();
    }

    if method == "GET"
        && path.starts_with("/v1/templates/")
        && !path.ends_with("/dry-run")
        && !path.ends_with("/execute")
    {
        return handle_template_get(path);
    }

    if method == "POST" && path.starts_with("/v1/templates/") && path.ends_with("/dry-run") {
        return handle_template_dry_run(path, body);
    }

    if method == "POST" && path.starts_with("/v1/templates/") && path.ends_with("/execute") {
        return handle_template_execute(path, body, config);
    }

    if method == "POST" && path == "/v1/executions" {
        return handle_execution_create(body, config, client.is_some());
    }

    if method == "GET" && path == "/v1/executions" {
        return handle_execution_list(config);
    }

    if method == "GET" && path.starts_with("/v1/executions/") && path.ends_with("/events") {
        return handle_execution_events(path, config);
    }

    if method == "GET" && path.starts_with("/v1/executions/") {
        return handle_execution_get(path, config);
    }

    if method == "PATCH" && path.starts_with("/v1/executions/") && path.ends_with("/policy") {
        return handle_execution_policy_patch(path, body, config);
    }

    if method == "POST" && path.starts_with("/v1/executions/") && path.ends_with("/pause") {
        return handle_execution_action(path, config, ExecutionAction::Pause);
    }

    if method == "POST" && path.starts_with("/v1/executions/") && path.ends_with("/resume") {
        return handle_execution_action(path, config, ExecutionAction::Resume);
    }

    if method == "POST" && path.starts_with("/v1/executions/") && path.ends_with("/cancel") {
        return handle_execution_action(path, config, ExecutionAction::Cancel);
    }

    if method == "POST" && path == "/v1/launch" {
        return handle_launch(body, config, client);
    }

    json_response(
        404,
        &ApiError {
            code: "NOT_FOUND",
            message: format!("no route for {} {}", method, path),
            retryable: false,
        },
    )
}

#[cfg(feature = "serde")]
fn handle_execution_dry_run(body: &str) -> JsonHttpResponse {
    let temp_root = std::env::temp_dir().join(format!("void-control-dry-run-{}", now_ms()));
    let spec = match parse_submitted_execution_spec(body, &temp_root.join("specs")) {
        Ok(spec) => spec,
        Err(err) => {
            return json_response(
                400,
                &json!({
                    "valid": false,
                    "plan": {
                        "candidates_per_iteration": 0,
                        "max_iterations": Value::Null,
                        "max_child_runs": Value::Null,
                        "estimated_concurrent_peak": 0,
                        "variation_source": "invalid",
                        "parameter_space_size": Value::Null
                    },
                    "warnings": [],
                    "errors": [err]
                }),
            )
        }
    };

    respond_with_execution_dry_run(spec, temp_root)
}

#[cfg(feature = "serde")]
fn handle_execution_create(
    body: &str,
    config: &BridgeConfig,
    _use_live_runtime: bool,
) -> JsonHttpResponse {
    let spec = match parse_submitted_execution_spec(body, &config.spec_dir) {
        Ok(spec) => spec,
        Err(err) => {
            return json_response(
                400,
                &ApiError {
                    code: "INVALID_SPEC",
                    message: err,
                    retryable: false,
                },
            )
        }
    };

    let store = FsExecutionStore::new(config.execution_dir.clone());
    let execution_id = format!("exec-{}", now_ms());
    submit_execution_spec(&store, &execution_id, &spec)
}

#[cfg(feature = "serde")]
fn handle_batch_dry_run(body: &str) -> JsonHttpResponse {
    let spec = match parse_submitted_batch_spec(body) {
        Ok(spec) => spec,
        Err(response) => return response,
    };
    let execution = match batch::compile_batch_spec(&spec) {
        Ok(execution) => execution,
        Err(err) => {
            return json_response(
                400,
                &ApiError {
                    code: "INVALID_BATCH",
                    message: err.to_string(),
                    retryable: false,
                },
            )
        }
    };

    json_response(
        200,
        &json!({
            "kind": "batch",
            "compiled_primitive": "swarm",
            "compiled": compiled_execution_summary(&execution)
        }),
    )
}

#[cfg(feature = "serde")]
fn handle_team_dry_run(body: &str) -> JsonHttpResponse {
    let spec = match parse_submitted_team_spec(body) {
        Ok(spec) => spec,
        Err(response) => return response,
    };
    let execution = match team::compile_team_spec(&spec) {
        Ok(execution) => execution,
        Err(err) => {
            return json_response(
                400,
                &ApiError {
                    code: "INVALID_TEAM",
                    message: err.to_string(),
                    retryable: false,
                },
            )
        }
    };

    json_response(
        200,
        &json!({
            "kind": "team",
            "compiled_primitive": execution.mode,
            "compiled": compiled_execution_summary(&execution)
        }),
    )
}

#[cfg(feature = "serde")]
fn handle_team_run(body: &str, config: &BridgeConfig) -> JsonHttpResponse {
    let spec = match parse_submitted_team_spec(body) {
        Ok(spec) => spec,
        Err(response) => return response,
    };
    let execution_spec = match team::compile_team_spec(&spec) {
        Ok(execution) => execution,
        Err(err) => {
            return json_response(
                400,
                &ApiError {
                    code: "INVALID_TEAM",
                    message: err.to_string(),
                    retryable: false,
                },
            )
        }
    };

    let store = FsExecutionStore::new(config.execution_dir.clone());
    let execution_id = format!("exec-{}", now_ms());
    match ExecutionService::<MockRuntime>::submit_execution(&store, &execution_id, &execution_spec)
    {
        Ok(execution) => json_response(
            200,
            &json!({
                "kind": "team",
                "execution_id": execution.execution_id,
                "compiled_primitive": execution_spec.mode,
                "status": execution.status,
                "goal": execution.goal
            }),
        ),
        Err(err) => json_response(
            500,
            &ApiError {
                code: "INTERNAL_ERROR",
                message: err.to_string(),
                retryable: true,
            },
        ),
    }
}

#[cfg(feature = "serde")]
fn handle_team_get(path: &str, config: &BridgeConfig) -> JsonHttpResponse {
    let Some(execution_id) = path.strip_prefix("/v1/team-runs/") else {
        return json_response(
            404,
            &ApiError {
                code: "NOT_FOUND",
                message: format!("no route for GET {path}"),
                retryable: false,
            },
        );
    };

    let execution_path = format!("/v1/executions/{execution_id}");
    let response = handle_execution_get(&execution_path, config);
    let Ok(mut value) = serde_json::from_slice::<Value>(&response.body) else {
        return response;
    };
    if response.status == 200 {
        let Some(object) = value.as_object_mut() else {
            return response;
        };
        object.insert("kind".to_string(), Value::String("team".to_string()));
        object.insert(
            "run_id".to_string(),
            Value::String(execution_id.to_string()),
        );
    }
    json_response(response.status, &value)
}

#[cfg(feature = "serde")]
fn handle_batch_run(body: &str, config: &BridgeConfig) -> JsonHttpResponse {
    let spec = match parse_submitted_batch_spec(body) {
        Ok(spec) => spec,
        Err(response) => return response,
    };
    let execution_spec = match batch::compile_batch_spec(&spec) {
        Ok(execution) => execution,
        Err(err) => {
            return json_response(
                400,
                &ApiError {
                    code: "INVALID_BATCH",
                    message: err.to_string(),
                    retryable: false,
                },
            )
        }
    };

    let store = FsExecutionStore::new(config.execution_dir.clone());
    let execution_id = format!("exec-{}", now_ms());
    match ExecutionService::<MockRuntime>::submit_execution(&store, &execution_id, &execution_spec)
    {
        Ok(execution) => json_response(
            200,
            &json!({
                "kind": "batch",
                "run_id": execution.execution_id,
                "execution_id": execution.execution_id,
                "compiled_primitive": "swarm",
                "status": execution.status,
                "goal": execution.goal
            }),
        ),
        Err(err) => json_response(
            500,
            &ApiError {
                code: "INTERNAL_ERROR",
                message: err.to_string(),
                retryable: true,
            },
        ),
    }
}

#[cfg(feature = "serde")]
fn handle_batch_get(path: &str, config: &BridgeConfig) -> JsonHttpResponse {
    let execution_path = if let Some(execution_id) = path.strip_prefix("/v1/batch-runs/") {
        format!("/v1/executions/{execution_id}")
    } else if let Some(execution_id) = path.strip_prefix("/v1/yolo-runs/") {
        format!("/v1/executions/{execution_id}")
    } else {
        String::new()
    };
    if execution_path.is_empty() {
        return json_response(
            404,
            &ApiError {
                code: "NOT_FOUND",
                message: format!("no route for GET {path}"),
                retryable: false,
            },
        );
    }

    let response = handle_execution_get(&execution_path, config);
    let Ok(mut value) = serde_json::from_slice::<Value>(&response.body) else {
        return response;
    };
    if response.status == 200 {
        let Some(object) = value.as_object_mut() else {
            return response;
        };
        let execution_id = object
            .get("execution")
            .and_then(|execution| execution.get("execution_id"))
            .cloned()
            .unwrap_or(Value::Null);
        object.insert("kind".to_string(), Value::String("batch".to_string()));
        object.insert("run_id".to_string(), execution_id);
    }
    json_response(response.status, &value)
}

#[cfg(feature = "serde")]
fn parse_submitted_batch_spec(body: &str) -> Result<batch::BatchSpec, JsonHttpResponse> {
    let trimmed = body.trim_start();
    let parsed = if trimmed.starts_with('{') || trimmed.starts_with('[') {
        batch::parse_batch_json(body)
    } else {
        batch::parse_batch_yaml(body)
    };
    parsed.map_err(|err| {
        json_response(
            400,
            &ApiError {
                code: "INVALID_BATCH",
                message: err.to_string(),
                retryable: false,
            },
        )
    })
}

#[cfg(feature = "serde")]
fn parse_submitted_team_spec(body: &str) -> Result<team::TeamSpec, JsonHttpResponse> {
    let trimmed = body.trim_start();
    let parsed = if trimmed.starts_with('{') || trimmed.starts_with('[') {
        team::parse_team_json(body)
    } else {
        team::parse_team_yaml(body)
    };
    parsed.map_err(|err| {
        json_response(
            400,
            &ApiError {
                code: "INVALID_TEAM",
                message: err.to_string(),
                retryable: false,
            },
        )
    })
}

#[cfg(feature = "serde")]
fn handle_template_list() -> JsonHttpResponse {
    match templates::list_templates() {
        Ok(list) => json_response(200, &json!({ "templates": list })),
        Err(err) => json_response(
            500,
            &ApiError {
                code: "INTERNAL_ERROR",
                message: err.to_string(),
                retryable: true,
            },
        ),
    }
}

#[cfg(feature = "serde")]
fn handle_template_get(path: &str) -> JsonHttpResponse {
    let Some(template_id) = path.strip_prefix("/v1/templates/") else {
        return json_response(
            404,
            &ApiError {
                code: "NOT_FOUND",
                message: format!("no route for GET {path}"),
                retryable: false,
            },
        );
    };

    match templates::load_template(template_id) {
        Ok(template) => json_response(
            200,
            &json!({
                "template": template.template,
                "inputs": template.inputs,
                "defaults": {
                    "workflow_template": template.defaults.workflow_template
                },
                "compile": {
                    "bindings": template.compile.bindings
                }
            }),
        ),
        Err(err) if err.to_string().contains("No such file or directory") => json_response(
            404,
            &ApiError {
                code: "NOT_FOUND",
                message: format!("template '{}' not found", template_id),
                retryable: false,
            },
        ),
        Err(err) => json_response(
            400,
            &ApiError {
                code: "INVALID_TEMPLATE",
                message: err.to_string(),
                retryable: false,
            },
        ),
    }
}

#[cfg(feature = "serde")]
fn handle_template_dry_run(path: &str, body: &str) -> JsonHttpResponse {
    let Some(template_id) = path
        .strip_prefix("/v1/templates/")
        .and_then(|rest| rest.strip_suffix("/dry-run"))
    else {
        return json_response(
            404,
            &ApiError {
                code: "NOT_FOUND",
                message: format!("no route for POST {path}"),
                retryable: false,
            },
        );
    };

    let compiled = match compile_template_request(template_id, body) {
        Ok(compiled) => compiled,
        Err(response) => return response,
    };

    json_response(
        200,
        &json!({
            "template": {
                "id": compiled.template.id,
                "execution_kind": compiled.template.execution_kind
            },
            "inputs": compiled.normalized_inputs,
            "compiled": compiled_execution_summary(&compiled.execution_spec)
        }),
    )
}

#[cfg(feature = "serde")]
fn handle_template_execute(path: &str, body: &str, config: &BridgeConfig) -> JsonHttpResponse {
    let Some(template_id) = path
        .strip_prefix("/v1/templates/")
        .and_then(|rest| rest.strip_suffix("/execute"))
    else {
        return json_response(
            404,
            &ApiError {
                code: "NOT_FOUND",
                message: format!("no route for POST {path}"),
                retryable: false,
            },
        );
    };

    let compiled = match compile_template_request(template_id, body) {
        Ok(compiled) => compiled,
        Err(response) => return response,
    };

    let execution_id = format!("exec-{}", now_ms());
    let store = FsExecutionStore::new(config.execution_dir.clone());
    match ExecutionService::<MockRuntime>::submit_execution(
        &store,
        &execution_id,
        &compiled.execution_spec,
    ) {
        Ok(execution) => json_response(
            200,
            &json!({
                "execution_id": execution.execution_id,
                "template": {
                    "id": compiled.template.id,
                    "execution_kind": compiled.template.execution_kind
                },
                "status": execution.status,
                "goal": execution.goal
            }),
        ),
        Err(err) => json_response(
            500,
            &ApiError {
                code: "INTERNAL_ERROR",
                message: err.to_string(),
                retryable: true,
            },
        ),
    }
}

#[cfg(feature = "serde")]
fn compile_template_request(
    template_id: &str,
    body: &str,
) -> Result<templates::CompiledTemplate, JsonHttpResponse> {
    let template = templates::load_template(template_id).map_err(|err| {
        if err.to_string().contains("No such file or directory") {
            json_response(
                404,
                &ApiError {
                    code: "NOT_FOUND",
                    message: format!("template '{}' not found", template_id),
                    retryable: false,
                },
            )
        } else {
            json_response(
                400,
                &ApiError {
                    code: "INVALID_TEMPLATE",
                    message: err.to_string(),
                    retryable: false,
                },
            )
        }
    })?;
    let request: TemplateRequestBody = serde_json::from_str(body).map_err(|err| {
        json_response(
            400,
            &ApiError {
                code: "INVALID_TEMPLATE_REQUEST",
                message: format!("invalid template request body: {err}"),
                retryable: false,
            },
        )
    })?;
    templates::compile_template(&template, &request.inputs).map_err(|err| {
        json_response(
            400,
            &ApiError {
                code: "INVALID_TEMPLATE_INPUTS",
                message: err.to_string(),
                retryable: false,
            },
        )
    })
}

#[cfg(feature = "serde")]
fn compiled_execution_summary(spec: &ExecutionSpec) -> Value {
    let candidate_overrides: Vec<_> = spec
        .variation
        .explicit
        .iter()
        .map(|proposal| proposal.overrides.clone())
        .collect();
    let overrides = spec
        .variation
        .explicit
        .first()
        .map(|proposal| proposal.overrides.clone())
        .unwrap_or_default();
    json!({
        "goal": spec.goal,
        "workflow_template": spec.workflow.template,
        "mode": spec.mode,
        "variation_source": spec.variation.source,
        "candidates_per_iteration": spec.variation.candidates_per_iteration,
        "candidate_overrides": candidate_overrides,
        "overrides": overrides
    })
}

#[cfg(feature = "serde")]
fn respond_with_execution_dry_run(spec: ExecutionSpec, temp_root: PathBuf) -> JsonHttpResponse {
    let service = ExecutionService::new(
        GlobalConfig {
            max_concurrent_child_runs: 20,
        },
        MockRuntime::new(),
        FsExecutionStore::new(temp_root),
    );
    let result = match service.dry_run(&spec) {
        Ok(result) => result,
        Err(err) => {
            return json_response(
                500,
                &ApiError {
                    code: "INTERNAL_ERROR",
                    message: err.to_string(),
                    retryable: true,
                },
            )
        }
    };
    let status = if result.valid { 200 } else { 400 };
    json_response(status, &result)
}

#[cfg(feature = "serde")]
fn submit_execution_spec(
    store: &FsExecutionStore,
    execution_id: &str,
    spec: &ExecutionSpec,
) -> JsonHttpResponse {
    match ExecutionService::<MockRuntime>::submit_execution(store, execution_id, spec) {
        Ok(execution) => json_response(200, &execution),
        Err(err) => json_response(
            500,
            &ApiError {
                code: "INTERNAL_ERROR",
                message: err.to_string(),
                retryable: true,
            },
        ),
    }
}

#[cfg(feature = "serde")]
fn parse_execution_spec_request(body: &str) -> Result<ExecutionSpecRequest, String> {
    match serde_json::from_str(body) {
        Ok(value) => Ok(value),
        Err(json_err) => serde_yaml::from_str(body).map_err(|yaml_err| {
            format!(
                "invalid execution spec body: JSON parse error: {json_err}; YAML parse error: {yaml_err}"
            )
        }),
    }
}

#[cfg(feature = "serde")]
fn parse_submitted_execution_spec(body: &str, spec_dir: &Path) -> Result<ExecutionSpec, String> {
    match parse_execution_spec_request(body) {
        Ok(spec_request) => spec_request.try_into_spec(),
        Err(parse_err) => {
            let runtime_doc = parse_runtime_spec_document(body)?;
            if !looks_like_runtime_spec(&runtime_doc) {
                return Err(parse_err);
            }
            let runtime_spec_path = write_spec_file(spec_dir, body, None)?;
            Ok(wrap_runtime_spec_as_execution(
                &runtime_spec_path,
                runtime_goal(&runtime_doc),
            ))
        }
    }
}

#[cfg(feature = "serde")]
fn parse_runtime_spec_document(body: &str) -> Result<serde_yaml::Value, String> {
    serde_yaml::from_str(body).map_err(|err| format!("invalid runtime spec body: {err}"))
}

#[cfg(feature = "serde")]
fn looks_like_runtime_spec(document: &serde_yaml::Value) -> bool {
    let Some(mapping) = document.as_mapping() else {
        return false;
    };
    for key in ["kind", "agent", "stages", "sandbox", "llm"] {
        let value = mapping.get(serde_yaml::Value::String(key.to_string()));
        if value.is_some() {
            return true;
        }
    }
    false
}

#[cfg(feature = "serde")]
fn runtime_goal(document: &serde_yaml::Value) -> String {
    let Some(mapping) = document.as_mapping() else {
        return "runtime execution".to_string();
    };
    let name = mapping
        .get(serde_yaml::Value::String("name".to_string()))
        .and_then(|value| value.as_str());
    let kind = mapping
        .get(serde_yaml::Value::String("kind".to_string()))
        .and_then(|value| value.as_str());

    if let Some(name) = name {
        return format!("run {name}");
    }
    if let Some(kind) = kind {
        return format!("run {kind}");
    }
    "runtime execution".to_string()
}

#[cfg(feature = "serde")]
fn wrap_runtime_spec_as_execution(workflow_template: &str, goal: String) -> ExecutionSpec {
    ExecutionSpec {
        mode: "swarm".to_string(),
        goal,
        workflow: WorkflowTemplateRef {
            template: workflow_template.to_string(),
        },
        policy: OrchestrationPolicy {
            budget: BudgetPolicy {
                max_iterations: Some(1),
                max_child_runs: Some(1),
                max_wall_clock_secs: Some(600),
                max_cost_usd_millis: None,
            },
            concurrency: ConcurrencyPolicy {
                max_concurrent_candidates: 1,
            },
            convergence: ConvergencePolicy {
                strategy: "exhaustive".to_string(),
                min_score: None,
                max_iterations_without_improvement: None,
            },
            max_candidate_failures_per_iteration: 1,
            missing_output_policy: "mark_failed".to_string(),
            iteration_failure_policy: "fail_execution".to_string(),
        },
        evaluation: EvaluationConfig {
            scoring_type: "weighted_metrics".to_string(),
            weights: std::collections::BTreeMap::new(),
            pass_threshold: None,
            ranking: "highest_score".to_string(),
            tie_breaking: "lexicographic".to_string(),
        },
        variation: VariationConfig::explicit(
            1,
            vec![VariationProposal {
                overrides: std::collections::BTreeMap::new(),
            }],
        ),
        swarm: true,
        supervision: None,
    }
}

#[cfg(feature = "serde")]
fn handle_execution_list(config: &BridgeConfig) -> JsonHttpResponse {
    let store = FsExecutionStore::new(config.execution_dir.clone());
    match store.list_execution_ids() {
        Ok(ids) => {
            let executions: Vec<_> = ids
                .into_iter()
                .filter_map(|execution_id| store.load_execution(&execution_id).ok())
                .map(|snapshot| snapshot.execution)
                .collect();
            json_response(200, &json!({ "executions": executions }))
        }
        Err(err) => json_response(
            500,
            &ApiError {
                code: "INTERNAL_ERROR",
                message: err.to_string(),
                retryable: true,
            },
        ),
    }
}

#[cfg(feature = "serde")]
fn handle_execution_get(path: &str, config: &BridgeConfig) -> JsonHttpResponse {
    let Some(execution_id) = path.strip_prefix("/v1/executions/") else {
        return json_response(
            404,
            &ApiError {
                code: "NOT_FOUND",
                message: format!("no route for GET {path}"),
                retryable: false,
            },
        );
    };
    let store = FsExecutionStore::new(config.execution_dir.clone());
    match store.load_execution(execution_id) {
        Ok(snapshot) => {
            let progress = summarize_progress(&snapshot);
            let result = ExecutionResultResponse {
                best_candidate_id: snapshot.execution.result_best_candidate_id.clone(),
                completed_iterations: snapshot.execution.completed_iterations,
                total_candidate_failures: snapshot
                    .execution
                    .failure_counts
                    .total_candidate_failures,
            };
            json_response(
                200,
                &ExecutionDetailResponse {
                    execution: snapshot.execution,
                    progress,
                    result,
                    candidates: snapshot.candidates,
                },
            )
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => json_response(
            404,
            &ApiError {
                code: "NOT_FOUND",
                message: format!("execution '{execution_id}' not found"),
                retryable: false,
            },
        ),
        Err(err) => json_response(
            500,
            &ApiError {
                code: "INTERNAL_ERROR",
                message: err.to_string(),
                retryable: true,
            },
        ),
    }
}

#[cfg(feature = "serde")]
fn summarize_progress(
    snapshot: &crate::orchestration::ExecutionSnapshot,
) -> ExecutionProgressResponse {
    let mut event_type_counts = std::collections::BTreeMap::new();
    for event in &snapshot.events {
        *event_type_counts
            .entry(event.event_type.as_str().to_string())
            .or_insert(0) += 1;
    }
    let queued_candidate_count = snapshot
        .candidates
        .iter()
        .filter(|candidate| candidate.status == crate::orchestration::CandidateStatus::Queued)
        .count() as u32;
    let running_candidate_count = snapshot
        .candidates
        .iter()
        .filter(|candidate| candidate.status == crate::orchestration::CandidateStatus::Running)
        .count() as u32;
    let completed_candidate_count = snapshot
        .candidates
        .iter()
        .filter(|candidate| candidate.status == crate::orchestration::CandidateStatus::Completed)
        .count() as u32;
    let failed_candidate_count = snapshot
        .candidates
        .iter()
        .filter(|candidate| candidate.status == crate::orchestration::CandidateStatus::Failed)
        .count() as u32;
    let canceled_candidate_count = snapshot
        .candidates
        .iter()
        .filter(|candidate| candidate.status == crate::orchestration::CandidateStatus::Canceled)
        .count() as u32;

    ExecutionProgressResponse {
        completed_iterations: snapshot.accumulator.completed_iterations,
        scoring_history_len: snapshot.accumulator.scoring_history_len,
        event_count: snapshot.events.len(),
        last_event: snapshot
            .events
            .last()
            .map(|event| event.event_type.as_str().to_string()),
        candidate_queue_count: snapshot
            .events
            .iter()
            .filter(|event| {
                event.event_type == crate::orchestration::ControlEventType::CandidateQueued
            })
            .count() as u32,
        candidate_dispatch_count: snapshot
            .events
            .iter()
            .filter(|event| {
                event.event_type == crate::orchestration::ControlEventType::CandidateDispatched
            })
            .count() as u32,
        candidate_output_count: snapshot
            .events
            .iter()
            .filter(|event| {
                event.event_type == crate::orchestration::ControlEventType::CandidateOutputCollected
            })
            .count() as u32,
        queued_candidate_count,
        running_candidate_count,
        completed_candidate_count,
        failed_candidate_count,
        canceled_candidate_count,
        event_type_counts,
    }
}

#[cfg(feature = "serde")]
fn handle_execution_events(path: &str, config: &BridgeConfig) -> JsonHttpResponse {
    let Some(execution_id) = path
        .strip_prefix("/v1/executions/")
        .and_then(|rest| rest.strip_suffix("/events"))
    else {
        return json_response(
            404,
            &ApiError {
                code: "NOT_FOUND",
                message: format!("no route for GET {path}"),
                retryable: false,
            },
        );
    };
    let store = FsExecutionStore::new(config.execution_dir.clone());
    match store.load_execution(execution_id) {
        Ok(snapshot) => json_response(
            200,
            &json!({
                "execution_id": execution_id,
                "events": snapshot.events
                    .into_iter()
                    .map(|event| json!({
                        "seq": event.seq,
                        "event_type": event.event_type.as_str(),
                    }))
                    .collect::<Vec<_>>()
            }),
        ),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => json_response(
            404,
            &ApiError {
                code: "NOT_FOUND",
                message: format!("execution '{execution_id}' not found"),
                retryable: false,
            },
        ),
        Err(err) => json_response(
            500,
            &ApiError {
                code: "INTERNAL_ERROR",
                message: err.to_string(),
                retryable: true,
            },
        ),
    }
}

#[cfg(feature = "serde")]
fn handle_execution_action(
    path: &str,
    config: &BridgeConfig,
    action: ExecutionAction,
) -> JsonHttpResponse {
    let suffix = match action {
        ExecutionAction::Pause => "/pause",
        ExecutionAction::Resume => "/resume",
        ExecutionAction::Cancel => "/cancel",
    };
    let Some(execution_id) = path
        .strip_prefix("/v1/executions/")
        .and_then(|rest| rest.strip_suffix(suffix))
    else {
        return json_response(
            404,
            &ApiError {
                code: "NOT_FOUND",
                message: format!("no route for POST {path}"),
                retryable: false,
            },
        );
    };
    let store = FsExecutionStore::new(config.execution_dir.clone());
    match ExecutionService::<MockRuntime>::update_execution_status(&store, execution_id, action) {
        Ok(execution) => json_response(200, &execution),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => json_response(
            404,
            &ApiError {
                code: "NOT_FOUND",
                message: format!("execution '{execution_id}' not found"),
                retryable: false,
            },
        ),
        Err(err) if err.kind() == std::io::ErrorKind::InvalidInput => json_response(
            400,
            &ApiError {
                code: "INVALID_STATE",
                message: err.to_string(),
                retryable: false,
            },
        ),
        Err(err) => json_response(
            500,
            &ApiError {
                code: "INTERNAL_ERROR",
                message: err.to_string(),
                retryable: true,
            },
        ),
    }
}

#[cfg(feature = "serde")]
fn handle_execution_policy_patch(
    path: &str,
    body: &str,
    config: &BridgeConfig,
) -> JsonHttpResponse {
    let Some(execution_id) = path
        .strip_prefix("/v1/executions/")
        .and_then(|rest| rest.strip_suffix("/policy"))
    else {
        return json_response(
            404,
            &ApiError {
                code: "NOT_FOUND",
                message: format!("no route for PATCH {path}"),
                retryable: false,
            },
        );
    };

    let request: PolicyPatchRequest = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(err) => {
            return json_response(
                400,
                &ApiError {
                    code: "INVALID_POLICY",
                    message: format!("invalid JSON body: {err}"),
                    retryable: false,
                },
            )
        }
    };

    if request.convergence.is_some() || request.evaluation.is_some() || request.variation.is_some()
    {
        return json_response(
            400,
            &ApiError {
                code: "INVALID_POLICY",
                message: "convergence, evaluation, and variation fields are immutable".to_string(),
                retryable: false,
            },
        );
    }

    let patch = PolicyPatch {
        max_iterations: request.budget.and_then(|budget| budget.max_iterations),
        max_concurrent_candidates: request
            .concurrency
            .and_then(|concurrency| concurrency.max_concurrent_candidates),
    };
    let store = FsExecutionStore::new(config.execution_dir.clone());
    match ExecutionService::<MockRuntime>::patch_execution_policy(
        &store,
        execution_id,
        patch,
        &GlobalConfig {
            max_concurrent_child_runs: 20,
        },
    ) {
        Ok(spec) => json_response(
            200,
            &json!({
                "execution_id": execution_id,
                "max_iterations": spec.policy.budget.max_iterations,
                "max_concurrent_candidates": spec.policy.concurrency.max_concurrent_candidates
            }),
        ),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => json_response(
            404,
            &ApiError {
                code: "NOT_FOUND",
                message: format!("execution '{execution_id}' not found"),
                retryable: false,
            },
        ),
        Err(err) if err.kind() == std::io::ErrorKind::InvalidInput => json_response(
            400,
            &ApiError {
                code: "INVALID_POLICY",
                message: err.to_string(),
                retryable: false,
            },
        ),
        Err(err) => json_response(
            500,
            &ApiError {
                code: "INTERNAL_ERROR",
                message: err.to_string(),
                retryable: true,
            },
        ),
    }
}

#[cfg(feature = "serde")]
fn process_pending_executions_once<R: ExecutionRuntime>(
    global: GlobalConfig,
    runtime: R,
    execution_dir: PathBuf,
) -> std::io::Result<()> {
    let store = FsExecutionStore::new(execution_dir);
    let mut service = ExecutionService::new(global.clone(), runtime, store.clone());
    let mut running_only_execution_ids = Vec::new();

    let ids = store.list_execution_ids()?;
    for execution_id in &ids {
        let snapshot = store.load_execution(execution_id)?;
        if matches!(
            snapshot.execution.status,
            crate::orchestration::ExecutionStatus::Pending
                | crate::orchestration::ExecutionStatus::Running
        ) {
            match service.plan_execution(execution_id) {
                Ok(_) => {}
                Err(err)
                    if matches!(
                        err.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::InvalidInput
                    ) => {}
                Err(err) => return Err(err),
            }
        }
    }

    let mut scheduler = GlobalScheduler::new(global.max_concurrent_child_runs as usize);
    for execution_id in ids {
        let snapshot = store.load_execution(&execution_id)?;
        if !matches!(
            snapshot.execution.status,
            crate::orchestration::ExecutionStatus::Running
                | crate::orchestration::ExecutionStatus::Paused
        ) {
            continue;
        }
        let spec = match store.load_spec(&execution_id) {
            Ok(spec) => spec,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        let paused = snapshot.execution.status == crate::orchestration::ExecutionStatus::Paused;
        let running = snapshot
            .candidates
            .iter()
            .filter(|candidate| candidate.status == crate::orchestration::CandidateStatus::Running)
            .count();
        scheduler.register_execution(
            &execution_id,
            paused,
            running,
            spec.policy.concurrency.max_concurrent_candidates as usize,
        );
        let queued_candidates: Vec<_> = snapshot
            .candidates
            .iter()
            .filter(|candidate| candidate.status == crate::orchestration::CandidateStatus::Queued)
            .collect();
        if !queued_candidates.is_empty() {
            for candidate in queued_candidates {
                scheduler.enqueue(QueuedCandidate::new(
                    &execution_id,
                    &candidate.candidate_id,
                    candidate.created_seq,
                ));
            }
        } else if snapshot.execution.status == crate::orchestration::ExecutionStatus::Running {
            running_only_execution_ids.push(execution_id.clone());
        }
    }

    for execution_id in running_only_execution_ids {
        match service.bridge_dispatch_execution_once(&execution_id) {
            Ok(_) => {}
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::InvalidInput
                ) => {}
            Err(err) => return Err(err),
        }
    }

    while let Some(grant) = scheduler.next_dispatch() {
        scheduler.mark_running(&grant);
        match service.bridge_dispatch_execution_once(&grant.execution_id) {
            Ok(_) => {}
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::InvalidInput
                ) => {}
            Err(err) => return Err(err),
        }
        let snapshot = store.load_execution(&grant.execution_id)?;
        let spec = match store.load_spec(&grant.execution_id) {
            Ok(spec) => spec,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        let paused = snapshot.execution.status == crate::orchestration::ExecutionStatus::Paused;
        let running = snapshot
            .candidates
            .iter()
            .filter(|candidate| candidate.status == crate::orchestration::CandidateStatus::Running)
            .count();
        scheduler.register_execution(
            &grant.execution_id,
            paused,
            running,
            spec.policy.concurrency.max_concurrent_candidates as usize,
        );
    }
    Ok(())
}

#[cfg(feature = "serde")]
pub fn process_pending_executions_once_for_test<R: ExecutionRuntime>(
    global: GlobalConfig,
    runtime: R,
    execution_dir: PathBuf,
) -> std::io::Result<()> {
    process_pending_executions_once(global, runtime, execution_dir)
}

#[cfg(feature = "serde")]
fn handle_launch(
    body: &str,
    config: &BridgeConfig,
    client: Option<&VoidBoxRuntimeClient>,
) -> JsonHttpResponse {
    let launch: LaunchRequest = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            return json_response(
                400,
                &ApiError {
                    code: "INVALID_SPEC",
                    message: format!("invalid JSON body: {e}"),
                    retryable: false,
                },
            )
        }
    };

    let file = if let Some(spec_text) = launch
        .spec_text
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        match write_spec_file(&config.spec_dir, spec_text, launch.spec_format.as_deref()) {
            Ok(path) => path,
            Err(e) => {
                return json_response(
                    500,
                    &ApiError {
                        code: "INTERNAL_ERROR",
                        message: e,
                        retryable: true,
                    },
                )
            }
        }
    } else if let Some(file) = launch
        .file
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        file
    } else {
        return json_response(
            400,
            &ApiError {
                code: "INVALID_SPEC",
                message: "provide either `spec_text` or `file`".to_string(),
                retryable: false,
            },
        );
    };

    let run_id = launch
        .run_id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(next_run_id);
    let policy = policy_from_json(launch.policy);
    if let Err(msg) = policy.validate() {
        return json_response(
            400,
            &ApiError {
                code: "INVALID_POLICY",
                message: msg.to_string(),
                retryable: false,
            },
        );
    }

    let Some(client) = client else {
        return json_response(
            500,
            &ApiError {
                code: "INTERNAL_ERROR",
                message: "launch route requires runtime client".to_string(),
                retryable: false,
            },
        );
    };

    match client.start(StartRequest {
        run_id: run_id.clone(),
        workflow_spec: file.clone(),
        launch_context: None,
        policy,
    }) {
        Ok(started) => json_response(
            200,
            &LaunchResponse {
                run_id: run_id_from_handle(&started.handle),
                attempt_id: started.attempt_id,
                state: state_to_str(started.state).to_string(),
                file,
            },
        ),
        Err(e) => json_response(
            500,
            &ApiError {
                code: "INTERNAL_ERROR",
                message: e.message,
                retryable: e.retryable,
            },
        ),
    }
}

#[cfg(feature = "serde")]
impl ExecutionSpecRequest {
    fn try_into_spec(self) -> Result<ExecutionSpec, String> {
        let mode = self.mode.trim().to_string();
        if !matches!(mode.as_str(), "swarm" | "supervision") {
            return Err(format!("unsupported mode '{mode}'"));
        }
        let goal = self.goal.trim().to_string();
        let workflow_template = self.workflow.template.trim().to_string();
        let variation = match self.variation.source.as_str() {
            "parameter_space" => VariationConfig::parameter_space(
                self.variation.candidates_per_iteration,
                match self.variation.selection.as_deref().unwrap_or("sequential") {
                    "random" => VariationSelection::Random,
                    _ => VariationSelection::Sequential,
                },
                self.variation.parameter_space.unwrap_or_default(),
            ),
            "explicit" => VariationConfig::explicit(
                self.variation.candidates_per_iteration,
                self.variation
                    .explicit
                    .unwrap_or_default()
                    .into_iter()
                    .map(|proposal| VariationProposal {
                        overrides: proposal.overrides,
                    })
                    .collect(),
            ),
            "signal_reactive" => VariationConfig {
                source: "signal_reactive".to_string(),
                candidates_per_iteration: self.variation.candidates_per_iteration,
                selection: match self.variation.selection.as_deref() {
                    Some("random") => Some(VariationSelection::Random),
                    Some("sequential") | None => Some(VariationSelection::Sequential),
                    Some(_) => Some(VariationSelection::Sequential),
                },
                parameter_space: self.variation.parameter_space.unwrap_or_default(),
                explicit: self
                    .variation
                    .explicit
                    .unwrap_or_default()
                    .into_iter()
                    .map(|proposal| VariationProposal {
                        overrides: proposal.overrides,
                    })
                    .collect(),
            },
            "leader_directed" => {
                VariationConfig::leader_directed(self.variation.candidates_per_iteration)
            }
            other => return Err(format!("unsupported variation source '{other}'")),
        };

        Ok(ExecutionSpec {
            mode,
            goal,
            workflow: WorkflowTemplateRef {
                template: workflow_template,
            },
            policy: OrchestrationPolicy {
                budget: BudgetPolicy {
                    max_iterations: self.policy.budget.max_iterations,
                    max_child_runs: self.policy.budget.max_child_runs,
                    max_wall_clock_secs: self.policy.budget.max_wall_clock_secs,
                    max_cost_usd_millis: self.policy.budget.max_cost_usd_millis,
                },
                concurrency: ConcurrencyPolicy {
                    max_concurrent_candidates: self.policy.concurrency.max_concurrent_candidates,
                },
                convergence: ConvergencePolicy {
                    strategy: self.policy.convergence.strategy,
                    min_score: self.policy.convergence.min_score,
                    max_iterations_without_improvement: self
                        .policy
                        .convergence
                        .max_iterations_without_improvement,
                },
                max_candidate_failures_per_iteration: self
                    .policy
                    .max_candidate_failures_per_iteration,
                missing_output_policy: self.policy.missing_output_policy,
                iteration_failure_policy: self.policy.iteration_failure_policy,
            },
            evaluation: EvaluationConfig {
                scoring_type: self.evaluation.scoring_type,
                weights: self.evaluation.weights,
                pass_threshold: self.evaluation.pass_threshold,
                ranking: self.evaluation.ranking,
                tie_breaking: self.evaluation.tie_breaking,
            },
            variation,
            swarm: self.swarm,
            supervision: self.supervision.map(|supervision| SupervisionConfig {
                supervisor_role: supervision.supervisor_role,
                review_policy: SupervisionReviewPolicy {
                    max_revision_rounds: supervision.review_policy.max_revision_rounds,
                    retry_on_runtime_failure: supervision.review_policy.retry_on_runtime_failure,
                    require_final_approval: supervision.review_policy.require_final_approval,
                },
            }),
        })
    }
}

#[cfg(feature = "serde")]
fn default_policy() -> ExecutionPolicy {
    ExecutionPolicy {
        max_parallel_microvms_per_run: 2,
        max_stage_retries: 1,
        stage_timeout_secs: 300,
        cancel_grace_period_secs: 10,
    }
}

#[cfg(feature = "serde")]
fn policy_from_json(raw: Option<RunPolicyJson>) -> ExecutionPolicy {
    let defaults = default_policy();
    let Some(raw) = raw else {
        return defaults;
    };
    ExecutionPolicy {
        max_parallel_microvms_per_run: raw
            .max_parallel_microvms_per_run
            .unwrap_or(defaults.max_parallel_microvms_per_run),
        max_stage_retries: raw.max_stage_retries.unwrap_or(defaults.max_stage_retries),
        stage_timeout_secs: raw
            .stage_timeout_secs
            .unwrap_or(defaults.stage_timeout_secs),
        cancel_grace_period_secs: raw
            .cancel_grace_period_secs
            .unwrap_or(defaults.cancel_grace_period_secs),
    }
}

#[cfg(feature = "serde")]
fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

#[cfg(feature = "serde")]
fn next_run_id() -> String {
    format!("ui-{}", now_ms())
}

#[cfg(feature = "serde")]
fn run_id_from_handle(handle: &str) -> String {
    handle
        .strip_prefix("void-box:")
        .or_else(|| handle.strip_prefix("vb:"))
        .unwrap_or(handle)
        .to_string()
}

#[cfg(feature = "serde")]
fn state_to_str(state: RunState) -> &'static str {
    match state {
        RunState::Pending => "pending",
        RunState::Starting => "starting",
        RunState::Running => "running",
        RunState::Succeeded => "succeeded",
        RunState::Failed => "failed",
        RunState::Canceled => "cancelled",
    }
}

#[cfg(feature = "serde")]
fn infer_ext(spec_format: Option<&str>, spec_text: &str) -> &'static str {
    if let Some(fmt) = spec_format {
        let f = fmt.to_ascii_lowercase();
        if f.contains("json") {
            return "json";
        }
        if f.contains("yaml") || f.contains("yml") {
            return "yaml";
        }
    }
    if spec_text.trim_start().starts_with('{') || spec_text.trim_start().starts_with('[') {
        "json"
    } else {
        "yaml"
    }
}

#[cfg(feature = "serde")]
fn write_spec_file(
    spec_dir: &Path,
    spec_text: &str,
    spec_format: Option<&str>,
) -> Result<String, String> {
    fs::create_dir_all(spec_dir)
        .map_err(|e| format!("failed to create spec dir {}: {e}", spec_dir.display()))?;
    let ext = infer_ext(spec_format, spec_text);
    let filename = format!("spec-{}-{}.{}", now_ms(), std::process::id(), ext);
    let path = spec_dir.join(filename);
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|e| format!("failed to create spec file {}: {e}", path.display()))?;
    file.write_all(spec_text.as_bytes())
        .and_then(|_| file.flush())
        .map_err(|e| format!("failed to write spec file {}: {e}", path.display()))?;
    Ok(path.display().to_string())
}

#[cfg(feature = "serde")]
fn json_response<T: Serialize>(status: u16, body: &T) -> JsonHttpResponse {
    let payload = serde_json::to_vec(body).unwrap_or_else(|_| {
        b"{\"code\":\"INTERNAL_ERROR\",\"message\":\"serialization failed\",\"retryable\":true}"
            .to_vec()
    });
    JsonHttpResponse {
        status,
        body: payload,
    }
}

#[cfg(feature = "serde")]
fn make_header(name: &str, value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("valid header")
}

#[cfg(feature = "serde")]
fn to_tiny_response(response: JsonHttpResponse) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_data(response.body)
        .with_status_code(tiny_http::StatusCode(response.status))
        .with_header(make_header("Content-Type", "application/json"))
        .with_header(make_header("Access-Control-Allow-Origin", "*"))
        .with_header(make_header(
            "Access-Control-Allow-Methods",
            "GET,POST,OPTIONS",
        ))
        .with_header(make_header("Access-Control-Allow-Headers", "Content-Type"))
}
