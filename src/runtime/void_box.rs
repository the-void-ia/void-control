use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Method, Request as HyperRequest};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client as HyperClient;
use hyper_util::rt::TokioExecutor;
use hyperlocal::{UnixConnector, Uri as HyperLocalUri};

use crate::contract::{
    from_void_box_run_and_events_json, from_void_box_run_json, map_void_box_status, ContractError,
    ContractErrorCode, ConvertedRunView, EventEnvelope, EventType, RunState, RuntimeInspection,
    StartRequest, StartResult, StopRequest, StopResult, SubscribeEventsRequest,
};
use crate::orchestration::CandidateOutput;
use crate::runtime::daemon_address::{
    classify_daemon_url, default_unix_url, resolve_tcp_token, token_search_labels,
    DaemonAddressError, DaemonScheme,
};
#[cfg(feature = "serde")]
use crate::runtime::VoidBoxRunRef;

/// Per-request HTTP timeout for the daemon transport. Applied uniformly to
/// both TCP and AF_UNIX dispatch so callers see the same bound regardless of
/// the configured socket scheme. Mirrors void-box's CLI backend.
const HTTP_TIMEOUT: Duration = Duration::from_secs(60);

/// Pooled hyper-util client over a TCP connector. Wraps a connection pool
/// over `Connection: keep-alive`, matching what the daemon serves on the
/// same transport.
type TcpHyperClient = HyperClient<HttpConnector, Full<Bytes>>;

/// Pooled hyper-util client over `hyperlocal::UnixConnector`. Same client
/// shape as `TcpHyperClient` — only the connector differs, so dispatch can
/// share one code path once the URI is built.
type UnixHyperClient = HyperClient<UnixConnector, Full<Bytes>>;

/// Shared HTTP client for the void-box daemon.
///
/// Cheap to `clone` — the underlying transport is held in an `Arc`, and
/// the wrapped hyper-util `Client` already pools connections internally,
/// so all clones share one connection pool. This is the shape the bridge
/// uses to fan a single startup-built client out to the worker tick and
/// per-request handlers without re-running the construction-time token
/// resolution.
#[derive(Clone)]
pub struct VoidBoxRuntimeClient {
    base_url: String,
    poll_interval_ms: u64,
    transport: std::sync::Arc<dyn HttpTransport + Send + Sync>,
}

impl VoidBoxRuntimeClient {
    /// Construct a client.
    ///
    /// The transport is selected once from `base_url`:
    /// - `unix:///abs/path` → AF_UNIX transport, no auth header.
    /// - `http://host:port` or bare `host:port` → TCP transport. A bearer
    ///   token must be resolvable from `VOIDBOX_DAEMON_TOKEN_FILE`,
    ///   `VOIDBOX_DAEMON_TOKEN`, or `$XDG_CONFIG_HOME/voidbox/daemon-token`;
    ///   construction panics if none is configured (we fail at construction
    ///   so a misconfigured deployment doesn't dial and discover via 401).
    ///
    /// Empty `base_url` is treated as "use the default discovered AF_UNIX
    /// socket path", mirroring the daemon's own auto-discovery so a same-uid
    /// invocation needs no configuration.
    pub fn new(base_url: String, poll_interval_ms: u64) -> Self {
        let url = if base_url.trim().is_empty() {
            default_unix_url()
        } else {
            base_url
        };
        let transport = build_transport(&url).unwrap_or_else(|err| {
            panic!("void-box runtime client construction failed: {err}");
        });
        Self {
            base_url: url,
            poll_interval_ms,
            transport: transport.into(),
        }
    }

    #[cfg(test)]
    fn with_transport(
        base_url: String,
        poll_interval_ms: u64,
        transport: Box<dyn HttpTransport + Send + Sync>,
    ) -> Self {
        Self {
            base_url,
            poll_interval_ms,
            transport: transport.into(),
        }
    }

    pub fn poll_interval_ms(&self) -> u64 {
        self.poll_interval_ms
    }

    #[cfg(feature = "serde")]
    pub fn delivery_run_ref(&self, handle: &str) -> Result<VoidBoxRunRef, ContractError> {
        Ok(VoidBoxRunRef {
            daemon_base_url: self.base_url.clone(),
            run_id: run_id_from_handle(handle)?.to_string(),
        })
    }

    pub async fn start(&self, request: StartRequest) -> Result<StartResult, ContractError> {
        request
            .policy
            .validate()
            .map_err(|msg| ContractError::new(ContractErrorCode::InvalidPolicy, msg, false))?;

        let payload = serde_json::json!({
            "file": request.workflow_spec,
            "input": request
                .launch_context
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null)
        })
        .to_string();

        let response = self.http_post("/v1/runs", &payload).await?;
        if response.status == 404 {
            return Err(ContractError::new(
                ContractErrorCode::NotFound,
                "void-box endpoint not found",
                false,
            ));
        }
        if response.status >= 400 {
            return Err(ContractError::new(
                ContractErrorCode::InternalError,
                format!("void-box start failed: HTTP {}", response.status),
                response.status >= 500,
            ));
        }

        let body: serde_json::Value = serde_json::from_str(&response.body).map_err(|e| {
            ContractError::new(
                ContractErrorCode::InvalidSpec,
                format!("invalid create-run response: {e}"),
                false,
            )
        })?;
        let run_id = body
            .get("run_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ContractError::new(
                    ContractErrorCode::InvalidSpec,
                    "missing run_id in create-run response",
                    false,
                )
            })?
            .to_string();

        Ok(StartResult {
            handle: handle_from_run_id(&run_id),
            attempt_id: 1,
            state: RunState::Running,
        })
    }

    pub async fn stop(&self, request: StopRequest) -> Result<StopResult, ContractError> {
        let run_id = run_id_from_handle(&request.handle)?;
        let cancel_path = format!("/v1/runs/{run_id}/cancel");
        let cancel_resp = self.http_post(&cancel_path, "{}").await?;

        if cancel_resp.status == 404 {
            return Err(ContractError::new(
                ContractErrorCode::NotFound,
                format!("run '{run_id}' not found"),
                false,
            ));
        }
        if cancel_resp.status >= 400 {
            return Err(ContractError::new(
                ContractErrorCode::InternalError,
                format!("void-box cancel failed: HTTP {}", cancel_resp.status),
                cancel_resp.status >= 500,
            ));
        }

        let converted = self.fetch_converted_run(run_id).await?;
        let Some(terminal) = converted
            .events
            .iter()
            .rev()
            .find(|e| {
                matches!(
                    e.event_type,
                    EventType::RunCanceled | EventType::RunFailed | EventType::RunCompleted
                )
            })
            .cloned()
        else {
            return Err(ContractError::new(
                ContractErrorCode::InternalError,
                "terminal event not found after cancel",
                true,
            ));
        };

        Ok(StopResult {
            state: converted.inspection.state,
            terminal_event_id: terminal.event_id,
        })
    }

    pub async fn inspect(&self, handle: &str) -> Result<RuntimeInspection, ContractError> {
        let run_id = run_id_from_handle(handle)?;
        let run_path = format!("/v1/runs/{run_id}");
        let run_resp = self.http_get(&run_path).await?;

        if run_resp.status == 404 {
            return Err(ContractError::new(
                ContractErrorCode::NotFound,
                format!("run '{run_id}' not found"),
                false,
            ));
        }
        if run_resp.status >= 400 {
            return Err(ContractError::new(
                ContractErrorCode::InternalError,
                format!("inspect failed: HTTP {}", run_resp.status),
                run_resp.status >= 500,
            ));
        }

        let converted = from_void_box_run_json(&run_resp.body)?;
        Ok(converted.inspection)
    }

    pub async fn list_runs(
        &self,
        state: Option<&str>,
    ) -> Result<Vec<RuntimeInspection>, ContractError> {
        let path = if let Some(filter) = state.filter(|s| !s.trim().is_empty()) {
            format!("/v1/runs?state={}", filter.trim())
        } else {
            "/v1/runs".to_string()
        };
        let response = self.http_get(&path).await?;

        if response.status >= 400 {
            return Err(ContractError::new(
                ContractErrorCode::InternalError,
                format!("list runs failed: HTTP {}", response.status),
                response.status >= 500,
            ));
        }

        let body: serde_json::Value = serde_json::from_str(&response.body).map_err(|e| {
            ContractError::new(
                ContractErrorCode::InvalidSpec,
                format!("invalid runs response JSON: {e}"),
                false,
            )
        })?;
        let runs = body
            .get("runs")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ContractError::new(
                    ContractErrorCode::InvalidSpec,
                    "missing runs array in list response",
                    false,
                )
            })?;

        let inspections = runs
            .iter()
            .filter_map(|run| {
                let run_id = run
                    .get("id")
                    .or_else(|| run.get("run_id"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)?;
                let status_raw = run
                    .get("status")
                    .or_else(|| run.get("state"))
                    .and_then(serde_json::Value::as_str)?;
                let state = map_void_box_status(status_raw)?;
                let attempt_id = run
                    .get("attempt_id")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(1) as u32;
                let active_stage_count = run
                    .get("active_stage_count")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as u32;
                let active_microvm_count = run
                    .get("active_microvm_count")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as u32;
                let started_at = run
                    .get("started_at")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let updated_at = run
                    .get("updated_at")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let terminal_reason = run
                    .get("terminal_reason")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
                let exit_code = run
                    .get("exit_code")
                    .and_then(serde_json::Value::as_i64)
                    .map(|v| v as i32);
                Some(RuntimeInspection {
                    run_id,
                    attempt_id,
                    state,
                    active_stage_count,
                    active_microvm_count,
                    started_at,
                    updated_at,
                    terminal_reason,
                    exit_code,
                })
            })
            .collect();

        Ok(inspections)
    }

    pub async fn subscribe_events(
        &self,
        request: SubscribeEventsRequest,
    ) -> Result<Vec<EventEnvelope>, ContractError> {
        let run_id = run_id_from_handle(&request.handle)?;
        let converted = self.fetch_converted_run(run_id).await?;
        Ok(filter_events_from_id(
            converted.events,
            request.from_event_id.as_deref(),
        ))
    }

    pub async fn fetch_structured_output(
        &self,
        run_id: &str,
    ) -> Result<Option<CandidateOutput>, ContractError> {
        let run_path = format!("/v1/runs/{run_id}");
        let run_resp = self.http_get(&run_path).await?;
        if run_resp.status == 404 {
            return Ok(None);
        }
        if run_resp.status >= 400 {
            return Err(map_http_error(
                run_resp.status,
                &run_resp.body,
                "inspect failed while resolving structured output",
            ));
        }

        if let Some(retrieval_path) = manifest_retrieval_path(&run_resp.body, None, "result.json")?
        {
            let response = self.http_get(&retrieval_path).await?;
            return match parse_artifact_response(
                &response,
                ContractErrorCode::StructuredOutputMissing,
            )? {
                Some(body) => parse_structured_output(run_id, &body).map(Some),
                None => Ok(None),
            };
        }

        let run_value: serde_json::Value = serde_json::from_str(&run_resp.body).map_err(|e| {
            ContractError::new(
                ContractErrorCode::InvalidSpec,
                format!("invalid run JSON: {e}"),
                false,
            )
        })?;

        if let Some(output) = structured_output_from_run_report(run_id, &run_value)? {
            return Ok(Some(output));
        }

        let mut last_missing_error = None;
        let mut stages = vec!["main".to_string(), "output".to_string()];
        if let Some(report_stage) = run_value
            .get("report")
            .and_then(|report| report.get("name"))
            .and_then(serde_json::Value::as_str)
        {
            if !stages.iter().any(|stage| stage == report_stage) {
                stages.push(report_stage.to_string());
            }
        }
        for stage in stages {
            let path = format!("/v1/runs/{run_id}/stages/{stage}/output-file");
            let response = self.http_get(&path).await?;
            if response.status == 404 {
                if let Some(err) = parse_api_error(&response.body) {
                    match err.code {
                        ContractErrorCode::StructuredOutputMissing
                        | ContractErrorCode::ArtifactNotFound
                        | ContractErrorCode::NotFound => {
                            last_missing_error = Some(err);
                            continue;
                        }
                        _ => return Err(err),
                    }
                }
                continue;
            }
            if response.status >= 400 {
                return Err(map_http_error(
                    response.status,
                    &response.body,
                    "structured output fetch failed",
                ));
            }
            return parse_structured_output(run_id, &response.body).map(Some);
        }
        if let Some(err) = last_missing_error {
            return Err(err);
        }
        Ok(None)
    }

    pub async fn fetch_named_artifact(
        &self,
        run_id: &str,
        stage: &str,
        name: &str,
    ) -> Result<Option<String>, ContractError> {
        let path = self
            .find_manifest_artifact_path(run_id, Some(stage), name)
            .await?
            .unwrap_or_else(|| format!("/v1/runs/{run_id}/stages/{stage}/artifacts/{name}"));
        let response = self.http_get(&path).await?;
        parse_artifact_response(&response, ContractErrorCode::ArtifactNotFound)
    }

    async fn fetch_converted_run(&self, run_id: &str) -> Result<ConvertedRunView, ContractError> {
        let run_path = format!("/v1/runs/{run_id}");
        let events_path = format!("/v1/runs/{run_id}/events");
        let run_resp = self.http_get(&run_path).await?;
        let events_resp = self.http_get(&events_path).await?;

        if run_resp.status == 404 || events_resp.status == 404 {
            return Err(ContractError::new(
                ContractErrorCode::NotFound,
                format!("run '{run_id}' not found"),
                false,
            ));
        }
        if run_resp.status >= 400 || events_resp.status >= 400 {
            let status = if run_resp.status >= 400 {
                run_resp.status
            } else {
                events_resp.status
            };
            return Err(ContractError::new(
                ContractErrorCode::InternalError,
                format!("event poll failed: HTTP {status}"),
                status >= 500,
            ));
        }

        from_void_box_run_and_events_json(&run_resp.body, &events_resp.body)
    }

    async fn http_get(&self, path: &str) -> Result<HttpResponse, ContractError> {
        self.transport.request("GET", path, "").await
    }

    async fn http_post(&self, path: &str, body: &str) -> Result<HttpResponse, ContractError> {
        self.transport.request("POST", path, body).await
    }

    async fn find_manifest_artifact_path(
        &self,
        run_id: &str,
        stage: Option<&str>,
        name: &str,
    ) -> Result<Option<String>, ContractError> {
        let run_path = format!("/v1/runs/{run_id}");
        let run_resp = self.http_get(&run_path).await?;
        if run_resp.status == 404 {
            return Ok(None);
        }
        if run_resp.status >= 400 {
            return Err(map_http_error(
                run_resp.status,
                &run_resp.body,
                "inspect failed while resolving artifact manifest",
            ));
        }

        manifest_retrieval_path(&run_resp.body, stage, name)
    }
}

/// Transport-level HTTP request abstraction.
///
/// `base_url` (TCP) and the socket path (AF_UNIX) are bound at construction
/// rather than passed per-call. This keeps the per-call surface narrow and
/// avoids the AF_UNIX impl having to ignore an argument it can't consume.
#[async_trait]
pub(crate) trait HttpTransport: Send + Sync {
    async fn request(
        &self,
        method: &str,
        path: &str,
        body: &str,
    ) -> Result<HttpResponse, ContractError>;
}

/// TCP transport with optional bearer-token injection.
///
/// When `bearer_token` is `Some`, every request gets an `Authorization:
/// Bearer <token>` header. The AF_UNIX path explicitly omits this header
/// because the daemon's `enforce_auth` short-circuits on `AuthMode::UnixSocket`,
/// and sending a credential over a transport that doesn't need it widens the
/// blast radius of an accidental leak (e.g. proxy logs).
pub(crate) struct TcpHttpTransport {
    base_url: String,
    bearer_token: Option<String>,
    client: TcpHyperClient,
}

impl TcpHttpTransport {
    pub(crate) fn new(base_url: String, bearer_token: Option<String>) -> Self {
        let client = HyperClient::builder(TokioExecutor::new()).build(HttpConnector::new());
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            bearer_token,
            client,
        }
    }
}

#[async_trait]
impl HttpTransport for TcpHttpTransport {
    async fn request(
        &self,
        method: &str,
        path: &str,
        body: &str,
    ) -> Result<HttpResponse, ContractError> {
        let url = format!("{}{}", self.base_url, path);
        let uri: hyper::Uri = url.parse().map_err(|e| {
            ContractError::new(
                ContractErrorCode::InvalidSpec,
                format!("invalid daemon URL {url:?}: {e}"),
                false,
            )
        })?;
        let body_bytes = Bytes::copy_from_slice(body.as_bytes());
        let request = build_request(method, uri, body_bytes, self.bearer_token.as_deref())?;
        let display_url = url;
        send_with_timeout(self.client.request(request), &display_url).await
    }
}

/// AF_UNIX transport. Wraps the same hyper-util pooled client as
/// [`TcpHttpTransport`]; only the connector differs.
///
/// Deliberately sends no `Authorization` header: the daemon authenticates
/// AF_UNIX peers by uid via the kernel's `0o600` perms, and emitting a
/// credential here would leak it to anywhere the request body is logged.
pub(crate) struct UnixHttpTransport {
    socket_path: PathBuf,
    client: UnixHyperClient,
}

impl UnixHttpTransport {
    pub(crate) fn new(socket_path: PathBuf) -> Self {
        let client = HyperClient::builder(TokioExecutor::new()).build(UnixConnector);
        Self {
            socket_path,
            client,
        }
    }
}

#[async_trait]
impl HttpTransport for UnixHttpTransport {
    async fn request(
        &self,
        method: &str,
        path: &str,
        body: &str,
    ) -> Result<HttpResponse, ContractError> {
        let uri: hyper::Uri = HyperLocalUri::new(&self.socket_path, path).into();
        let display_url = format!("unix://{}", self.socket_path.display());
        let body_bytes = Bytes::copy_from_slice(body.as_bytes());
        // No bearer token over AF_UNIX; see struct doc.
        let request = build_request(method, uri, body_bytes, None)?;
        send_with_timeout(self.client.request(request), &display_url).await
    }
}

fn build_request(
    method: &str,
    uri: hyper::Uri,
    body: Bytes,
    bearer_token: Option<&str>,
) -> Result<HyperRequest<Full<Bytes>>, ContractError> {
    let parsed_method = Method::from_bytes(method.as_bytes()).map_err(|e| {
        ContractError::new(
            ContractErrorCode::InvalidSpec,
            format!("unsupported HTTP method {method:?}: {e}"),
            false,
        )
    })?;
    let mut builder = HyperRequest::builder()
        .method(parsed_method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = bearer_token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Full::new(body)).map_err(|e| {
        ContractError::new(
            ContractErrorCode::InternalError,
            format!("build request: {e}"),
            true,
        )
    })
}

async fn send_with_timeout<F, B>(fut: F, display_url: &str) -> Result<HttpResponse, ContractError>
where
    F: std::future::Future<Output = Result<hyper::Response<B>, hyper_util::client::legacy::Error>>,
    B: hyper::body::Body<Data = Bytes> + Unpin,
    B::Error: std::fmt::Display,
{
    let send = async {
        let resp = fut.await.map_err(|e| {
            ContractError::new(
                ContractErrorCode::InternalError,
                format!("connect to {display_url} failed: {e}"),
                true,
            )
        })?;
        let status = resp.status().as_u16();
        let collected = resp.into_body().collect().await.map_err(|e| {
            ContractError::new(
                ContractErrorCode::InternalError,
                format!("response read failed: {e}"),
                true,
            )
        })?;
        let body = String::from_utf8_lossy(&collected.to_bytes()).into_owned();
        Ok(HttpResponse { status, body })
    };
    match tokio::time::timeout(HTTP_TIMEOUT, send).await {
        Ok(result) => result,
        Err(_) => Err(ContractError::new(
            ContractErrorCode::RetrievalTimeout,
            format!("daemon request to {display_url} timed out"),
            true,
        )),
    }
}

/// URL-scheme dispatch for [`VoidBoxRuntimeClient::new`] and
/// [`HttpSidecarAdapter::new`]. Returns the boxed transport selected by the
/// scheme, after resolving and validating any TCP bearer token.
pub(crate) fn build_transport(
    base_url: &str,
) -> Result<Box<dyn HttpTransport + Send + Sync>, String> {
    let scheme = classify_daemon_url(base_url)?;
    match scheme {
        DaemonScheme::Unix(path) => Ok(Box::new(UnixHttpTransport::new(path))),
        DaemonScheme::Tcp(url) => {
            let resolved = resolve_tcp_token().map_err(|e| e.to_string())?;
            let token = resolved.0.ok_or_else(|| {
                DaemonAddressError::MissingTcpToken {
                    url: url.clone(),
                    searched: token_search_labels(),
                }
                .to_string()
            })?;
            Ok(Box::new(TcpHttpTransport::new(url, Some(token))))
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HttpResponse {
    pub(crate) status: u16,
    pub(crate) body: String,
}

fn handle_from_run_id(run_id: &str) -> String {
    format!("vb:{run_id}")
}

fn run_id_from_handle(handle: &str) -> Result<&str, ContractError> {
    handle.strip_prefix("vb:").ok_or_else(|| {
        ContractError::new(
            ContractErrorCode::NotFound,
            format!("invalid run handle '{handle}'"),
            false,
        )
    })
}

fn filter_events_from_id(
    events: Vec<EventEnvelope>,
    from_event_id: Option<&str>,
) -> Vec<EventEnvelope> {
    let Some(from_id) = from_event_id else {
        return events;
    };
    if let Some(idx) = events.iter().position(|e| e.event_id == from_id) {
        return events.into_iter().skip(idx + 1).collect();
    }
    events
}

fn manifest_retrieval_path(
    run_body: &str,
    stage: Option<&str>,
    name: &str,
) -> Result<Option<String>, ContractError> {
    let value: serde_json::Value = serde_json::from_str(run_body).map_err(|e| {
        ContractError::new(
            ContractErrorCode::InvalidSpec,
            format!("invalid run JSON: {e}"),
            false,
        )
    })?;
    let Some(manifest) = value
        .get("artifact_publication")
        .and_then(|value| value.get("manifest"))
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(None);
    };

    for entry in manifest {
        let entry_name = entry.get("name").and_then(serde_json::Value::as_str);
        let entry_stage = entry.get("stage").and_then(serde_json::Value::as_str);
        let retrieval_path = entry
            .get("retrieval_path")
            .and_then(serde_json::Value::as_str);
        if entry_name == Some(name)
            && retrieval_path.is_some()
            && stage
                .map(|wanted| Some(wanted) == entry_stage)
                .unwrap_or(true)
        {
            return Ok(retrieval_path.map(normalize_retrieval_path));
        }
    }

    Ok(None)
}

fn structured_output_from_run_report(
    run_id: &str,
    value: &serde_json::Value,
) -> Result<Option<CandidateOutput>, ContractError> {
    let output_ready = value
        .get("output_ready")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if !output_ready {
        return Ok(None);
    }

    let Some(output) = value
        .get("report")
        .and_then(|report| report.get("output"))
        .and_then(serde_json::Value::as_str)
    else {
        return Ok(None);
    };

    let trimmed = output.trim_start();
    if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
        return Ok(None);
    }

    parse_structured_output(run_id, output).map(Some)
}

fn parse_artifact_response(
    response: &HttpResponse,
    default_not_found: ContractErrorCode,
) -> Result<Option<String>, ContractError> {
    if response.status == 404 {
        if let Some(err) = parse_api_error(&response.body) {
            return match err.code {
                ContractErrorCode::ArtifactNotFound | ContractErrorCode::NotFound
                    if default_not_found == ContractErrorCode::ArtifactNotFound =>
                {
                    Ok(None)
                }
                ContractErrorCode::StructuredOutputMissing => Err(err),
                _ => Err(err),
            };
        }
        return Ok(None);
    }
    if response.status >= 400 {
        return Err(map_http_error(
            response.status,
            &response.body,
            "artifact retrieval failed",
        ));
    }
    if response.body.trim().is_empty() {
        return Err(ContractError::new(
            default_not_found,
            "artifact body was empty",
            false,
        ));
    }
    Ok(Some(response.body.clone()))
}

fn map_http_error(status: u16, body: &str, fallback: &str) -> ContractError {
    parse_api_error(body).unwrap_or_else(|| {
        ContractError::new(
            ContractErrorCode::InternalError,
            format!("{fallback}: HTTP {status}"),
            status >= 500,
        )
    })
}

fn normalize_retrieval_path(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

fn parse_api_error(body: &str) -> Option<ContractError> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let code = value.get("code")?.as_str()?;
    let message = value
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(code)
        .to_string();
    let retryable = value
        .get("retryable")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    Some(ContractError::new(map_error_code(code), message, retryable))
}

fn map_error_code(code: &str) -> ContractErrorCode {
    match code {
        "INVALID_SPEC" => ContractErrorCode::InvalidSpec,
        "INVALID_POLICY" => ContractErrorCode::InvalidPolicy,
        "NOT_FOUND" => ContractErrorCode::NotFound,
        "ALREADY_TERMINAL" => ContractErrorCode::AlreadyTerminal,
        "RESOURCE_LIMIT_EXCEEDED" => ContractErrorCode::ResourceLimitExceeded,
        "STRUCTURED_OUTPUT_MISSING" => ContractErrorCode::StructuredOutputMissing,
        "STRUCTURED_OUTPUT_MALFORMED" => ContractErrorCode::StructuredOutputMalformed,
        "ARTIFACT_NOT_FOUND" => ContractErrorCode::ArtifactNotFound,
        "ARTIFACT_PUBLICATION_INCOMPLETE" => ContractErrorCode::ArtifactPublicationIncomplete,
        "ARTIFACT_STORE_UNAVAILABLE" => ContractErrorCode::ArtifactStoreUnavailable,
        "RETRIEVAL_TIMEOUT" => ContractErrorCode::RetrievalTimeout,
        _ => ContractErrorCode::InternalError,
    }
}

fn parse_structured_output(run_id: &str, body: &str) -> Result<CandidateOutput, ContractError> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|e| {
        ContractError::new(
            ContractErrorCode::StructuredOutputMalformed,
            format!("invalid structured output JSON: {e}"),
            false,
        )
    })?;
    let metrics = value
        .get("metrics")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            ContractError::new(
                ContractErrorCode::StructuredOutputMalformed,
                "structured output missing metrics object",
                false,
            )
        })?;

    let parsed_metrics = metrics
        .iter()
        .filter_map(|(key, value)| value.as_f64().map(|number| (key.clone(), number)))
        .collect();

    let mut output = CandidateOutput::new(
        run_id.to_string(),
        value
            .get("status")
            .and_then(serde_json::Value::as_str)
            .map(|status| status.eq_ignore_ascii_case("success"))
            .unwrap_or(true),
        parsed_metrics,
    );
    #[cfg(feature = "serde")]
    if let Some(intents) = value.get("intents").and_then(serde_json::Value::as_array) {
        output.intents = intents
            .iter()
            .cloned()
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                ContractError::new(
                    ContractErrorCode::StructuredOutputMalformed,
                    format!("invalid structured output intents: {e}"),
                    false,
                )
            })?;
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{filter_events_from_id, HttpResponse, HttpTransport, VoidBoxRuntimeClient};
    use crate::contract::{
        ContractErrorCode, EventEnvelope, EventType, ExecutionPolicy, RunState, StartRequest,
        StopRequest, SubscribeEventsRequest,
    };
    use async_trait::async_trait;
    use std::collections::{BTreeMap, HashMap};
    use std::sync::{Arc, Mutex};

    struct MockTransport {
        routes: Mutex<HashMap<(String, String), HttpResponse>>,
    }

    impl MockTransport {
        fn new(routes: Vec<(&str, &str, u16, &str)>) -> Self {
            let map = routes
                .into_iter()
                .map(|(m, p, s, b)| {
                    (
                        (m.to_string(), p.to_string()),
                        HttpResponse {
                            status: s,
                            body: b.to_string(),
                        },
                    )
                })
                .collect();
            Self {
                routes: Mutex::new(map),
            }
        }
    }

    #[async_trait]
    impl HttpTransport for MockTransport {
        async fn request(
            &self,
            method: &str,
            path: &str,
            _body: &str,
        ) -> Result<HttpResponse, crate::contract::ContractError> {
            let key = (method.to_string(), path.to_string());
            if let Some(resp) = self.routes.lock().expect("lock").get(&key) {
                return Ok(resp.clone());
            }
            Ok(HttpResponse {
                status: 404,
                body: r#"{"error":"not found"}"#.to_string(),
            })
        }
    }

    #[derive(Clone)]
    struct CaptureTransport {
        response: HttpResponse,
        requests: Arc<Mutex<Vec<(String, String, String)>>>,
    }

    impl CaptureTransport {
        fn new(response: HttpResponse) -> Self {
            Self {
                response,
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait]
    impl HttpTransport for CaptureTransport {
        async fn request(
            &self,
            method: &str,
            path: &str,
            body: &str,
        ) -> Result<HttpResponse, crate::contract::ContractError> {
            self.requests.lock().expect("lock").push((
                method.to_string(),
                path.to_string(),
                body.to_string(),
            ));
            Ok(self.response.clone())
        }
    }

    fn client(routes: Vec<(&str, &str, u16, &str)>) -> VoidBoxRuntimeClient {
        VoidBoxRuntimeClient::with_transport(
            "http://mock:3000".to_string(),
            250,
            Box::new(MockTransport::new(routes)),
        )
    }

    fn policy() -> ExecutionPolicy {
        ExecutionPolicy {
            max_parallel_microvms_per_run: 1,
            max_stage_retries: 0,
            stage_timeout_secs: 60,
            cancel_grace_period_secs: 5,
        }
    }

    #[tokio::test]
    async fn fetches_structured_output_from_stage_output_file() {
        let client = client(vec![
            (
                "GET",
                "/v1/runs/run-123",
                200,
                r#"{"id":"run-123","status":"Completed"}"#,
            ),
            (
                "GET",
                "/v1/runs/run-123/stages/main/output-file",
                200,
                r#"{"status":"success","summary":"ok","metrics":{"latency_p99_ms":87,"cost_usd":0.018},"artifacts":[]}"#,
            ),
        ]);

        let output = client
            .fetch_structured_output("run-123")
            .await
            .expect("fetch")
            .expect("output");

        assert_eq!(output.candidate_id, "run-123");
        assert!(output.succeeded);
        assert_eq!(output.metrics.get("latency_p99_ms"), Some(&87.0));
        assert_eq!(output.metrics.get("cost_usd"), Some(&0.018));
    }

    #[tokio::test]
    async fn returns_none_when_structured_output_file_missing() {
        let client = client(vec![]);

        let output = client
            .fetch_structured_output("run-missing")
            .await
            .expect("fetch");

        assert!(output.is_none());
    }

    #[tokio::test]
    async fn fetch_structured_output_prefers_manifested_result_json() {
        let client = client(vec![
            (
                "GET",
                "/v1/runs/run-123",
                200,
                r#"{
                    "artifact_publication": {
                        "manifest": [
                            {
                                "name": "result.json",
                                "stage": "main",
                                "retrieval_path": "/v1/runs/run-123/stages/main/artifacts/result.json"
                            }
                        ]
                    }
                }"#,
            ),
            (
                "GET",
                "/v1/runs/run-123/stages/main/artifacts/result.json",
                200,
                r#"{"status":"success","summary":"ok","metrics":{"latency_p99_ms":77},"artifacts":[]}"#,
            ),
        ]);

        let output = client
            .fetch_structured_output("run-123")
            .await
            .expect("fetch")
            .expect("output");

        assert_eq!(output.metrics.get("latency_p99_ms"), Some(&77.0));
    }

    #[tokio::test]
    async fn fetch_structured_output_uses_run_report_when_output_ready() {
        let client = client(vec![(
            "GET",
            "/v1/runs/run-service",
            200,
            r#"{
                "id":"run-service",
                "status":"running",
                "output_ready":true,
                "report":{
                    "name":"transform_optimizer",
                    "kind":"agent",
                    "success":true,
                    "output":"{\"status\":\"success\",\"summary\":\"ok\",\"metrics\":{\"latency_p99_ms\":61,\"error_rate\":0.01,\"cpu_pct\":48},\"artifacts\":[]}",
                    "stages":1,
                    "total_cost_usd":0.1,
                    "input_tokens":10,
                    "output_tokens":20
                }
            }"#,
        )]);

        let output = client
            .fetch_structured_output("run-service")
            .await
            .expect("fetch")
            .expect("output");

        assert!(output.succeeded);
        assert_eq!(output.metrics.get("latency_p99_ms"), Some(&61.0));
        assert_eq!(output.metrics.get("error_rate"), Some(&0.01));
        assert_eq!(output.metrics.get("cpu_pct"), Some(&48.0));
    }

    #[tokio::test]
    async fn fetch_structured_output_falls_back_when_report_output_is_guest_path() {
        let client = client(vec![
            (
                "GET",
                "/v1/runs/run-service-path",
                200,
                r#"{
                    "id":"run-service-path",
                    "status":"running",
                    "output_ready":true,
                    "report":{
                        "name":"transform_optimizer",
                        "kind":"agent",
                        "success":true,
                        "output":"/workspace/output.json",
                        "stages":1,
                        "total_cost_usd":0.1,
                        "input_tokens":10,
                        "output_tokens":20
                    },
                    "artifact_publication": {
                        "manifest": [
                            {
                                "name": "result.json",
                                "stage": "main",
                                "retrieval_path": "/v1/runs/run-service-path/stages/main/artifacts/result.json"
                            }
                        ]
                    }
                }"#,
            ),
            (
                "GET",
                "/v1/runs/run-service-path/stages/main/artifacts/result.json",
                200,
                r#"{"status":"success","summary":"ok","metrics":{"latency_p99_ms":59,"error_rate":0.02,"cpu_pct":44},"artifacts":[]}"#,
            ),
        ]);

        let output = client
            .fetch_structured_output("run-service-path")
            .await
            .expect("fetch")
            .expect("output");

        assert!(output.succeeded);
        assert_eq!(output.metrics.get("latency_p99_ms"), Some(&59.0));
        assert_eq!(output.metrics.get("error_rate"), Some(&0.02));
        assert_eq!(output.metrics.get("cpu_pct"), Some(&44.0));
    }

    #[tokio::test]
    async fn fetch_structured_output_uses_report_stage_output_file_when_report_output_is_guest_path(
    ) {
        let client = client(vec![
            (
                "GET",
                "/v1/runs/run-service-stage",
                200,
                r#"{
                    "id":"run-service-stage",
                    "status":"running",
                    "output_ready":true,
                    "report":{
                        "name":"transform_optimizer",
                        "kind":"agent",
                        "success":true,
                        "output":"/workspace/output.json",
                        "stages":1,
                        "total_cost_usd":0.1,
                        "input_tokens":10,
                        "output_tokens":20
                    },
                    "artifact_publication": {
                        "status": "not_started",
                        "manifest": []
                    }
                }"#,
            ),
            (
                "GET",
                "/v1/runs/run-service-stage/stages/main/output-file",
                404,
                r#"{"code":"STRUCTURED_OUTPUT_MISSING","message":"main missing result.json","retryable":false}"#,
            ),
            (
                "GET",
                "/v1/runs/run-service-stage/stages/output/output-file",
                404,
                r#"{"code":"STRUCTURED_OUTPUT_MISSING","message":"output missing result.json","retryable":false}"#,
            ),
            (
                "GET",
                "/v1/runs/run-service-stage/stages/transform_optimizer/output-file",
                200,
                r#"{"status":"success","summary":"ok","metrics":{"latency_p99_ms":52,"error_rate":0.03,"cpu_pct":41},"artifacts":[]}"#,
            ),
        ]);

        let output = client
            .fetch_structured_output("run-service-stage")
            .await
            .expect("fetch")
            .expect("output");

        assert!(output.succeeded);
        assert_eq!(output.metrics.get("latency_p99_ms"), Some(&52.0));
        assert_eq!(output.metrics.get("error_rate"), Some(&0.03));
        assert_eq!(output.metrics.get("cpu_pct"), Some(&41.0));
    }

    #[tokio::test]
    async fn fetch_structured_output_maps_missing_output_error() {
        let client = client(vec![
            (
                "GET",
                "/v1/runs/run-missing-output",
                200,
                r#"{"id":"run-missing-output","status":"Completed"}"#,
            ),
            (
                "GET",
                "/v1/runs/run-missing-output/stages/main/output-file",
                404,
                r#"{"code":"STRUCTURED_OUTPUT_MISSING","message":"missing result.json","retryable":false}"#,
            ),
        ]);

        let err = client
            .fetch_structured_output("run-missing-output")
            .await
            .expect_err("expected missing-output error");

        assert_eq!(err.code, ContractErrorCode::StructuredOutputMissing);
        assert!(!err.retryable);
    }

    #[tokio::test]
    async fn fetch_structured_output_falls_back_to_output_stage_after_main_404() {
        let client = client(vec![
            (
                "GET",
                "/v1/runs/run-output-stage",
                200,
                r#"{"id":"run-output-stage","status":"Completed"}"#,
            ),
            (
                "GET",
                "/v1/runs/run-output-stage/stages/main/output-file",
                404,
                r#"{"code":"STRUCTURED_OUTPUT_MISSING","message":"main missing result.json","retryable":false}"#,
            ),
            (
                "GET",
                "/v1/runs/run-output-stage/stages/output/output-file",
                200,
                r#"{"status":"success","summary":"ok","metrics":{"latency_p99_ms":66},"artifacts":[]}"#,
            ),
        ]);

        let output = client
            .fetch_structured_output("run-output-stage")
            .await
            .expect("fetch")
            .expect("output");

        assert_eq!(output.metrics.get("latency_p99_ms"), Some(&66.0));
    }

    #[tokio::test]
    async fn fetch_structured_output_maps_malformed_output_error() {
        let client = client(vec![
            (
                "GET",
                "/v1/runs/run-malformed",
                200,
                r#"{"id":"run-malformed","status":"Completed"}"#,
            ),
            (
                "GET",
                "/v1/runs/run-malformed/stages/main/output-file",
                200,
                r#"{"status":"success","metrics":not-json}"#,
            ),
        ]);

        let err = client
            .fetch_structured_output("run-malformed")
            .await
            .expect_err("expected malformed-output error");

        assert_eq!(err.code, ContractErrorCode::StructuredOutputMalformed);
    }

    #[tokio::test]
    async fn fetch_named_artifact_uses_manifest_retrieval_path() {
        let client = client(vec![
            (
                "GET",
                "/v1/runs/run-123",
                200,
                r#"{
                    "artifact_publication": {
                        "manifest": [
                            {
                                "name": "report.md",
                                "stage": "main",
                                "retrieval_path": "v1/runs/run-123/stages/main/artifacts/report.md"
                            }
                        ]
                    }
                }"#,
            ),
            (
                "GET",
                "/v1/runs/run-123/stages/main/artifacts/report.md",
                200,
                "# report\nartifact body",
            ),
        ]);

        let artifact = client
            .fetch_named_artifact("run-123", "main", "report.md")
            .await
            .expect("fetch")
            .expect("artifact");

        assert!(artifact.contains("artifact body"));
    }

    #[tokio::test]
    async fn start_returns_handle_and_running_state() {
        let c = client(vec![("POST", "/v1/runs", 200, r#"{"run_id":"run-123"}"#)]);
        let started = c
            .start(StartRequest {
                run_id: "controller-run-1".to_string(),
                workflow_spec: "fixtures/sample.vbrun".to_string(),
                launch_context: None,
                policy: policy(),
            })
            .await
            .expect("start");
        assert_eq!(started.handle, "vb:run-123");
        assert_eq!(started.attempt_id, 1);
        assert_eq!(started.state, RunState::Running);
        assert_eq!(c.poll_interval_ms(), 250);
    }

    #[tokio::test]
    async fn start_serializes_launch_context_into_input_payload() {
        let transport = CaptureTransport::new(HttpResponse {
            status: 200,
            body: r#"{"run_id":"run-123"}"#.to_string(),
        });
        let requests = transport.requests.clone();
        let c = VoidBoxRuntimeClient::with_transport(
            "http://mock:3000".to_string(),
            250,
            Box::new(transport),
        );

        let snapshot = serde_json::json!({
            "execution_id": "exec-message-box",
            "candidate_id": "candidate-1",
            "iteration": 1,
            "entries": [
                {
                    "message_id": "message-1",
                    "intent_id": "intent-1",
                    "from_candidate_id": "candidate-source",
                    "kind": "proposal",
                    "payload": {
                        "summary_text": "summary-one",
                        "strategy_hint": "hint-one"
                    }
                }
            ]
        });

        let started = c
            .start(StartRequest {
                run_id: "controller-run-1".to_string(),
                workflow_spec: "fixtures/sample.vbrun".to_string(),
                launch_context: Some(snapshot.to_string()),
                policy: policy(),
            })
            .await
            .expect("start");

        assert_eq!(started.handle, "vb:run-123");
        let recorded = requests.lock().expect("lock");
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, "POST");
        assert_eq!(recorded[0].1, "/v1/runs");
        let body: serde_json::Value =
            serde_json::from_str(&recorded[0].2).expect("parse request body");
        assert_eq!(
            body.get("file").and_then(serde_json::Value::as_str),
            Some("fixtures/sample.vbrun")
        );
        let expected_input = snapshot.to_string();
        assert_eq!(
            body.get("input").and_then(serde_json::Value::as_str),
            Some(expected_input.as_str())
        );
    }

    #[tokio::test]
    async fn inspect_maps_daemon_run_state() {
        let c = client(vec![(
            "GET",
            "/v1/runs/run-123",
            200,
            include_str!("../../fixtures/voidbox_run_success.json"),
        )]);
        let inspection = c.inspect("vb:run-123").await.expect("inspect");
        assert_eq!(inspection.run_id, "run-2000");
        assert_eq!(inspection.state, RunState::Succeeded);
    }

    #[tokio::test]
    async fn subscribe_events_applies_resume_filter() {
        let c = client(vec![
            (
                "GET",
                "/v1/runs/run-123",
                200,
                include_str!("../../fixtures/voidbox_run_success.json"),
            ),
            (
                "GET",
                "/v1/runs/run-123/events",
                200,
                include_str!("../../fixtures/voidbox_run_events_success.json"),
            ),
        ]);
        let events = c
            .subscribe_events(SubscribeEventsRequest {
                handle: "vb:run-123".to_string(),
                from_event_id: Some("evt_run-2000_1".to_string()),
            })
            .await
            .expect("subscribe");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::RunCompleted);
    }

    #[tokio::test]
    async fn stop_returns_terminal_event() {
        let c = client(vec![
            (
                "POST",
                "/v1/runs/run-123/cancel",
                200,
                include_str!("../../fixtures/voidbox_run_success.json"),
            ),
            (
                "GET",
                "/v1/runs/run-123",
                200,
                include_str!("../../fixtures/voidbox_run_success.json"),
            ),
            (
                "GET",
                "/v1/runs/run-123/events",
                200,
                include_str!("../../fixtures/voidbox_run_events_success.json"),
            ),
        ]);
        let stop = c
            .stop(StopRequest {
                handle: "vb:run-123".to_string(),
                reason: "user".to_string(),
            })
            .await
            .expect("stop");
        assert_eq!(stop.state, RunState::Succeeded);
        assert_eq!(stop.terminal_event_id, "evt_run-2000_2");
    }

    #[tokio::test]
    async fn inspect_404_maps_to_not_found() {
        let c = client(vec![(
            "GET",
            "/v1/runs/run-404",
            404,
            r#"{"error":"not found"}"#,
        )]);
        let err = c
            .inspect("vb:run-404")
            .await
            .expect_err("expected not found");
        assert_eq!(err.code, ContractErrorCode::NotFound);
    }

    #[test]
    fn filter_events_from_id_returns_full_when_marker_missing() {
        let events = vec![EventEnvelope {
            event_id: "evt_1".to_string(),
            event_type: EventType::RunStarted,
            run_id: "run-1".to_string(),
            attempt_id: 1,
            timestamp: "1ms".to_string(),
            seq: 1,
            payload: BTreeMap::new(),
        }];
        let out = filter_events_from_id(events.clone(), Some("evt_missing"));
        assert_eq!(out, events);
    }

    // Env mutation is process-global; serialize tests that set/unset env vars.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_env<F: FnOnce()>(vars: &[(&str, Option<&str>)], f: F) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let saved: Vec<(String, Option<String>)> = vars
            .iter()
            .map(|(k, _)| (k.to_string(), std::env::var(k).ok()))
            .collect();
        for (k, v) in vars {
            match v {
                Some(value) => std::env::set_var(k, value),
                None => std::env::remove_var(k),
            }
        }
        // AssertUnwindSafe: the test closures sometimes hold mutable borrows
        // of outer state; we restore env on the way out regardless of the
        // closure's panic-safety, which is the property we actually need.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        for (k, v) in saved {
            match v {
                Some(value) => std::env::set_var(k, value),
                None => std::env::remove_var(k),
            }
        }
        if let Err(payload) = result {
            std::panic::resume_unwind(payload);
        }
    }

    #[tokio::test]
    async fn dispatch_unix_url_selects_unix_transport() {
        // Build directly so we don't actually dial a socket; just verify the
        // builder accepts the URL and returns a transport.
        let transport = super::build_transport("unix:///tmp/voidbox-disp-test.sock")
            .expect("unix dispatch should succeed without env");
        // The boxed trait object's concrete type isn't observable from here;
        // smoke-check by attempting a request and asserting we get a connect
        // error (proves it's the unix path that ran). Match the structural
        // shape of the error our `send_with_timeout` helper produces — the
        // code, retryable flag, and message prefix are all set by us
        // explicitly, so the assertion doesn't depend on hyper-util's wording.
        let err = transport
            .request("GET", "/v1/health", "")
            .await
            .expect_err("connect should fail against missing socket");
        assert_eq!(err.code, ContractErrorCode::InternalError);
        assert!(err.retryable);
        assert!(
            err.message.starts_with("connect to unix://"),
            "expected connect-failure prefix, got: {}",
            err.message
        );
    }

    #[test]
    fn dispatch_tcp_url_fails_closed_when_no_token_configured() {
        // Point all token sources at a fresh empty dir so resolution returns
        // None; expect build_transport to fail with a clear message.
        let dir = std::env::temp_dir().join(format!(
            "void-control-tcp-no-token-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mut captured: Result<(), String> = Err(String::new());
        with_env(
            &[
                ("VOIDBOX_DAEMON_TOKEN_FILE", None),
                ("VOIDBOX_DAEMON_TOKEN", None),
                ("XDG_CONFIG_HOME", Some(dir.to_str().unwrap())),
                ("HOME", Some(dir.to_str().unwrap())),
            ],
            || {
                captured = super::build_transport("http://127.0.0.1:43100").map(|_| ());
            },
        );
        let err = captured.expect_err("TCP build_transport should fail without a token");
        assert!(err.contains("requires a bearer token"), "err={err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Promoted to `multi_thread`: spawns a listener task that runs in
    // parallel with the client request; multi_thread parity matches how
    // production hyper-util will dispatch.
    #[tokio::test(flavor = "multi_thread")]
    async fn unix_transport_emits_no_authorization_header() {
        // Bind a one-shot unix listener, route a request through the
        // hyper-util AF_UNIX transport, and assert the bytes hyper put on the
        // wire don't contain `Authorization:`. httpmock 0.7 has no AF_UNIX
        // server, and the production hyper-util writes the same HTTP/1.1
        // dialect onto the socket regardless of executor, so a raw read +
        // string-search captures everything we care about.
        //
        // AF_UNIX paths are bounded by `SUN_LEN` (~104 bytes on macOS) so we
        // bind under `/tmp` directly with a short suffix rather than via
        // `env::temp_dir()`, which on macOS resolves to a long
        // `/var/folders/...` path.
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::UnixListener;

        let socket = std::path::PathBuf::from(format!(
            "/tmp/vc-na-{}-{}.sock",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                & 0xfff_ffff
        ));
        let _ = std::fs::remove_file(&socket);
        let listener = UnixListener::bind(&socket).expect("bind");

        let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
        let captured_clone = captured.clone();
        let server_task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut bytes = Vec::new();
            // Read until end-of-headers; that's enough to inspect the
            // request shape. hyper-util sends the full request in one
            // contiguous write for our `Full<Bytes>` body.
            let mut buf = [0u8; 4096];
            loop {
                let n = stream.read(&mut buf).await.expect("read");
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&buf[..n]);
                if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            *captured_clone.lock().unwrap() = bytes;
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
                .await;
            let _ = stream.shutdown().await;
        });

        let transport = super::UnixHttpTransport::new(socket.clone());
        let resp = transport
            .request("GET", "/v1/health", "")
            .await
            .expect("unix request");
        assert_eq!(resp.status, 200);
        server_task.await.expect("server task");

        let request = String::from_utf8_lossy(&captured.lock().unwrap()).into_owned();
        assert!(
            !request.to_ascii_lowercase().contains("authorization:"),
            "AF_UNIX request must not carry an Authorization header; got:\n{request}"
        );
        let _ = std::fs::remove_file(&socket);
    }

    #[tokio::test]
    async fn tcp_transport_emits_authorization_header_when_token_present() {
        use httpmock::prelude::*;

        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/v1/health")
                    .header("authorization", "Bearer hunter2");
                then.status(200).body("{}");
            })
            .await;

        let transport =
            super::TcpHttpTransport::new(server.base_url(), Some("hunter2".to_string()));
        let resp = transport
            .request("GET", "/v1/health", "")
            .await
            .expect("tcp request");
        assert_eq!(resp.status, 200);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn tcp_transport_routes_through_httpmock_end_to_end() {
        // End-to-end check that the new hyper-util TCP transport dispatches
        // correctly against a regular HTTP server. This test also covers
        // AC#7-new (an httpmock test exercises the async TCP path).
        use httpmock::prelude::*;

        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/v1/runs")
                    .header("content-type", "application/json")
                    .json_body(serde_json::json!({"file":"fixtures/sample.vbrun","input":null}));
                then.status(200).body(r#"{"run_id":"run-async-1"}"#);
            })
            .await;

        let c = VoidBoxRuntimeClient::new(server.base_url(), 250);
        let started = c
            .start(StartRequest {
                run_id: "controller-run-1".to_string(),
                workflow_spec: "fixtures/sample.vbrun".to_string(),
                launch_context: None,
                policy: policy(),
            })
            .await
            .expect("start");
        assert_eq!(started.handle, "vb:run-async-1");
        mock.assert_async().await;
    }
}
