use std::io::{Read, Write};
use std::net::TcpStream;

use crate::contract::{
    from_void_box_run_and_events_json, from_void_box_run_json, ContractError, ContractErrorCode,
    ConvertedRunView, EventEnvelope, EventType, RunState, RuntimeInspection, StartRequest,
    StartResult, StopRequest, StopResult, SubscribeEventsRequest,
};

pub struct VoidBoxRuntimeClient {
    base_url: String,
    poll_interval_ms: u64,
    transport: Box<dyn HttpTransport + Send + Sync>,
}

impl VoidBoxRuntimeClient {
    pub fn new(base_url: String, poll_interval_ms: u64) -> Self {
        Self {
            base_url,
            poll_interval_ms,
            transport: Box::new(TcpHttpTransport),
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
            transport,
        }
    }

    pub fn poll_interval_ms(&self) -> u64 {
        self.poll_interval_ms
    }

    pub fn start(&self, request: StartRequest) -> Result<StartResult, ContractError> {
        request.policy.validate().map_err(|msg| {
            ContractError::new(ContractErrorCode::InvalidPolicy, msg, false)
        })?;

        let payload = serde_json::json!({
            "file": request.workflow_spec,
            "input": serde_json::Value::Null
        })
        .to_string();

        let response = self.http_post("/v1/runs", &payload)?;
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

    pub fn stop(&self, request: StopRequest) -> Result<StopResult, ContractError> {
        let run_id = run_id_from_handle(&request.handle)?;
        let cancel_path = format!("/v1/runs/{run_id}/cancel");
        let cancel_resp = self.http_post(&cancel_path, "{}")?;

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

        let converted = self.fetch_converted_run(run_id)?;
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

    pub fn inspect(&self, handle: &str) -> Result<RuntimeInspection, ContractError> {
        let run_id = run_id_from_handle(handle)?;
        let run_path = format!("/v1/runs/{run_id}");
        let run_resp = self.http_get(&run_path)?;

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

    pub fn subscribe_events(
        &self,
        request: SubscribeEventsRequest,
    ) -> Result<Vec<EventEnvelope>, ContractError> {
        let run_id = run_id_from_handle(&request.handle)?;
        let converted = self.fetch_converted_run(run_id)?;
        Ok(filter_events_from_id(
            converted.events,
            request.from_event_id.as_deref(),
        ))
    }

    fn fetch_converted_run(&self, run_id: &str) -> Result<ConvertedRunView, ContractError> {
        let run_path = format!("/v1/runs/{run_id}");
        let events_path = format!("/v1/runs/{run_id}/events");
        let run_resp = self.http_get(&run_path)?;
        let events_resp = self.http_get(&events_path)?;

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

    fn http_get(&self, path: &str) -> Result<HttpResponse, ContractError> {
        self.transport.request(&self.base_url, "GET", path, "")
    }

    fn http_post(&self, path: &str, body: &str) -> Result<HttpResponse, ContractError> {
        self.transport.request(&self.base_url, "POST", path, body)
    }
}

trait HttpTransport {
    fn request(
        &self,
        base_url: &str,
        method: &str,
        path: &str,
        body: &str,
    ) -> Result<HttpResponse, ContractError>;
}

struct TcpHttpTransport;

impl HttpTransport for TcpHttpTransport {
    fn request(
        &self,
        base_url: &str,
        method: &str,
        path: &str,
        body: &str,
    ) -> Result<HttpResponse, ContractError> {
        let (host, port) = parse_host_port(base_url)?;
        let addr = format!("{host}:{port}");
        let mut stream = TcpStream::connect(&addr).map_err(|e| {
            ContractError::new(
                ContractErrorCode::InternalError,
                format!("connect to {addr} failed: {e}"),
                true,
            )
        })?;

        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(request.as_bytes()).map_err(|e| {
            ContractError::new(
                ContractErrorCode::InternalError,
                format!("request write failed: {e}"),
                true,
            )
        })?;

        let mut response = String::new();
        stream.read_to_string(&mut response).map_err(|e| {
            ContractError::new(
                ContractErrorCode::InternalError,
                format!("response read failed: {e}"),
                true,
            )
        })?;

        parse_http_response(&response)
    }
}

#[derive(Debug, Clone)]
struct HttpResponse {
    status: u16,
    body: String,
}

fn parse_http_response(raw: &str) -> Result<HttpResponse, ContractError> {
    let (head, body) = raw.split_once("\r\n\r\n").ok_or_else(|| {
        ContractError::new(
            ContractErrorCode::InvalidSpec,
            "invalid HTTP response format",
            false,
        )
    })?;

    let mut lines = head.lines();
    let status_line = lines.next().unwrap_or_default();
    let mut parts = status_line.split_whitespace();
    let _http = parts.next();
    let status = parts
        .next()
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| {
            ContractError::new(
                ContractErrorCode::InvalidSpec,
                "invalid HTTP status line",
                false,
            )
        })?;

    Ok(HttpResponse {
        status,
        body: body.to_string(),
    })
}

fn parse_host_port(base_url: &str) -> Result<(String, u16), ContractError> {
    let stripped = base_url.strip_prefix("http://").ok_or_else(|| {
        ContractError::new(
            ContractErrorCode::InvalidSpec,
            "base_url must start with http://",
            false,
        )
    })?;
    let host_port = stripped.split('/').next().unwrap_or(stripped);
    let (host, port) = match host_port.split_once(':') {
        Some((host, port)) => {
            let parsed = port.parse::<u16>().map_err(|_| {
                ContractError::new(
                    ContractErrorCode::InvalidSpec,
                    format!("invalid port in base_url '{base_url}'"),
                    false,
                )
            })?;
            (host.to_string(), parsed)
        }
        None => (host_port.to_string(), 80),
    };
    Ok((host, port))
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

fn filter_events_from_id(events: Vec<EventEnvelope>, from_event_id: Option<&str>) -> Vec<EventEnvelope> {
    let Some(from_id) = from_event_id else {
        return events;
    };
    if let Some(idx) = events.iter().position(|e| e.event_id == from_id) {
        return events.into_iter().skip(idx + 1).collect();
    }
    events
}

#[cfg(test)]
mod tests {
    use super::{filter_events_from_id, HttpResponse, HttpTransport, VoidBoxRuntimeClient};
    use crate::contract::{
        ContractErrorCode, EventEnvelope, EventType, ExecutionPolicy, RunState, StartRequest,
        StopRequest, SubscribeEventsRequest,
    };
    use std::collections::{BTreeMap, HashMap};
    use std::sync::Mutex;

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

    impl HttpTransport for MockTransport {
        fn request(
            &self,
            _base_url: &str,
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

    #[test]
    fn start_returns_handle_and_running_state() {
        let c = client(vec![("POST", "/v1/runs", 200, r#"{"run_id":"run-123"}"#)]);
        let started = c
            .start(StartRequest {
                run_id: "controller-run-1".to_string(),
                workflow_spec: "fixtures/sample.vbrun".to_string(),
                policy: policy(),
            })
            .expect("start");
        assert_eq!(started.handle, "vb:run-123");
        assert_eq!(started.attempt_id, 1);
        assert_eq!(started.state, RunState::Running);
        assert_eq!(c.poll_interval_ms(), 250);
    }

    #[test]
    fn inspect_maps_daemon_run_state() {
        let c = client(vec![(
            "GET",
            "/v1/runs/run-123",
            200,
            include_str!("../../fixtures/voidbox_run_success.json"),
        )]);
        let inspection = c.inspect("vb:run-123").expect("inspect");
        assert_eq!(inspection.run_id, "run-2000");
        assert_eq!(inspection.state, RunState::Succeeded);
    }

    #[test]
    fn subscribe_events_applies_resume_filter() {
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
            .expect("subscribe");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::RunCompleted);
    }

    #[test]
    fn stop_returns_terminal_event() {
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
            .expect("stop");
        assert_eq!(stop.state, RunState::Succeeded);
        assert_eq!(stop.terminal_event_id, "evt_run-2000_2");
    }

    #[test]
    fn inspect_404_maps_to_not_found() {
        let c = client(vec![("GET", "/v1/runs/run-404", 404, r#"{"error":"not found"}"#)]);
        let err = c.inspect("vb:run-404").expect_err("expected not found");
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
}
