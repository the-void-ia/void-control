#[cfg(feature = "serde")]
pub fn run_bridge() -> Result<(), String> {
    use std::env;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde::{Deserialize, Serialize};
    use tiny_http::{Header, Method, Response, Server, StatusCode};

    use crate::contract::{ExecutionPolicy, RunState, StartRequest};
    use crate::runtime::VoidBoxRuntimeClient;

    #[derive(Debug, Deserialize)]
    struct LaunchRequest {
        run_id: Option<String>,
        file: Option<String>,
        spec_text: Option<String>,
        spec_format: Option<String>,
        policy: Option<PolicyJson>,
    }

    #[derive(Debug, Deserialize)]
    struct PolicyJson {
        max_parallel_microvms_per_run: Option<u32>,
        max_stage_retries: Option<u32>,
        stage_timeout_secs: Option<u32>,
        cancel_grace_period_secs: Option<u32>,
    }

    #[derive(Debug, Serialize)]
    struct LaunchResponse {
        run_id: String,
        attempt_id: u32,
        state: String,
        file: String,
    }

    #[derive(Debug, Serialize)]
    struct ApiError {
        code: &'static str,
        message: String,
        retryable: bool,
    }

    fn default_policy() -> ExecutionPolicy {
        ExecutionPolicy {
            max_parallel_microvms_per_run: 2,
            max_stage_retries: 1,
            stage_timeout_secs: 300,
            cancel_grace_period_secs: 10,
        }
    }

    fn policy_from_json(raw: Option<PolicyJson>) -> ExecutionPolicy {
        let defaults = default_policy();
        let Some(raw) = raw else {
            return defaults;
        };
        ExecutionPolicy {
            max_parallel_microvms_per_run: raw
                .max_parallel_microvms_per_run
                .unwrap_or(defaults.max_parallel_microvms_per_run),
            max_stage_retries: raw
                .max_stage_retries
                .unwrap_or(defaults.max_stage_retries),
            stage_timeout_secs: raw.stage_timeout_secs.unwrap_or(defaults.stage_timeout_secs),
            cancel_grace_period_secs: raw
                .cancel_grace_period_secs
                .unwrap_or(defaults.cancel_grace_period_secs),
        }
    }

    fn now_ms() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    }

    fn next_run_id() -> String {
        format!("ui-{}", now_ms())
    }

    fn run_id_from_handle(handle: &str) -> String {
        handle
            .strip_prefix("void-box:")
            .or_else(|| handle.strip_prefix("vb:"))
            .unwrap_or(handle)
            .to_string()
    }

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

    fn write_spec_file(spec_dir: &Path, spec_text: &str, spec_format: Option<&str>) -> Result<String, String> {
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

    fn make_header(name: &str, value: &str) -> Header {
        Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("valid header")
    }

    fn json_response<T: Serialize>(status: u16, body: &T) -> Response<std::io::Cursor<Vec<u8>>> {
        let payload = serde_json::to_vec(body).unwrap_or_else(|_| b"{\"code\":\"INTERNAL_ERROR\",\"message\":\"serialization failed\",\"retryable\":true}".to_vec());
        Response::from_data(payload)
            .with_status_code(StatusCode(status))
            .with_header(make_header("Content-Type", "application/json"))
            .with_header(make_header("Access-Control-Allow-Origin", "*"))
            .with_header(make_header("Access-Control-Allow-Methods", "GET,POST,OPTIONS"))
            .with_header(make_header("Access-Control-Allow-Headers", "Content-Type"))
    }

    fn json_error(status: u16, code: &'static str, message: String, retryable: bool) -> Response<std::io::Cursor<Vec<u8>>> {
        json_response(
            status,
            &ApiError {
                code,
                message,
                retryable,
            },
        )
    }

    let listen = env::var("VOID_CONTROL_BRIDGE_LISTEN").unwrap_or_else(|_| "127.0.0.1:43210".to_string());
    let base_url = env::var("VOID_BOX_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:43100".to_string());
    let spec_dir = env::var("VOID_CONTROL_SPEC_DIR").unwrap_or_else(|_| "/tmp/void-control/specs".to_string());
    let spec_dir_path = PathBuf::from(spec_dir);

    let server = Server::http(&listen).map_err(|e| format!("listen {listen} failed: {e}"))?;
    let client = VoidBoxRuntimeClient::new(base_url.clone(), 250);
    println!("voidctl bridge listening on http://{listen} -> {base_url}");

    for mut req in server.incoming_requests() {
        let method = req.method().clone();
        let path = req.url().to_string();

        if method == Method::Options {
            let _ = req.respond(
                Response::empty(204)
                    .with_header(make_header("Access-Control-Allow-Origin", "*"))
                    .with_header(make_header("Access-Control-Allow-Methods", "GET,POST,OPTIONS"))
                    .with_header(make_header("Access-Control-Allow-Headers", "Content-Type")),
            );
            continue;
        }

        if method == Method::Get && path == "/v1/health" {
            let _ = req.respond(json_response(200, &serde_json::json!({"status":"ok","service":"voidctl-bridge"})));
            continue;
        }

        if method == Method::Post && path == "/v1/launch" {
            let mut body = String::new();
            if let Err(e) = req.as_reader().read_to_string(&mut body) {
                let _ = req.respond(json_error(400, "INVALID_SPEC", format!("failed to read request body: {e}"), false));
                continue;
            }
            let launch: LaunchRequest = match serde_json::from_str(&body) {
                Ok(v) => v,
                Err(e) => {
                    let _ = req.respond(json_error(400, "INVALID_SPEC", format!("invalid JSON body: {e}"), false));
                    continue;
                }
            };

            let file = if let Some(spec_text) = launch.spec_text.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                match write_spec_file(&spec_dir_path, spec_text, launch.spec_format.as_deref()) {
                    Ok(path) => path,
                    Err(e) => {
                        let _ = req.respond(json_error(500, "INTERNAL_ERROR", e, true));
                        continue;
                    }
                }
            } else if let Some(file) = launch.file.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
                file
            } else {
                let _ = req.respond(json_error(
                    400,
                    "INVALID_SPEC",
                    "provide either `spec_text` or `file`".to_string(),
                    false,
                ));
                continue;
            };

            let run_id = launch.run_id.filter(|s| !s.trim().is_empty()).unwrap_or_else(next_run_id);
            let policy = policy_from_json(launch.policy);
            if let Err(msg) = policy.validate() {
                let _ = req.respond(json_error(400, "INVALID_POLICY", msg.to_string(), false));
                continue;
            }

            match client.start(StartRequest {
                run_id: run_id.clone(),
                workflow_spec: file.clone(),
                policy,
            }) {
                Ok(started) => {
                    let response = LaunchResponse {
                        run_id: run_id_from_handle(&started.handle),
                        attempt_id: started.attempt_id,
                        state: state_to_str(started.state).to_string(),
                        file,
                    };
                    let _ = req.respond(json_response(200, &response));
                }
                Err(e) => {
                    let _ = req.respond(json_error(
                        500,
                        "INTERNAL_ERROR",
                        e.message,
                        e.retryable,
                    ));
                }
            }
            continue;
        }

        let _ = req.respond(json_error(
            404,
            "NOT_FOUND",
            format!("no route for {} {}", method.as_str(), path),
            false,
        ));
    }

    Ok(())
}
