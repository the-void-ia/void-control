use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::contract::{
    ContractError, ContractErrorCode, EventEnvelope, EventType, RunState, RuntimeInspection,
    StartRequest, StartResult, StopRequest, StopResult, SubscribeEventsRequest,
};

#[derive(Debug, Clone)]
struct RunRecord {
    run_id: String,
    handle: String,
    attempt_id: u32,
    state: RunState,
    started_at: String,
    updated_at: String,
    terminal_reason: Option<String>,
    exit_code: Option<i32>,
    events: Vec<EventEnvelope>,
    next_seq: u64,
}

impl RunRecord {
    fn new(run_id: String) -> Self {
        let now = now_rfc3339_like();
        Self {
            handle: format!("run-handle:{run_id}"),
            run_id,
            attempt_id: 1,
            state: RunState::Running,
            started_at: now.clone(),
            updated_at: now,
            terminal_reason: None,
            exit_code: None,
            events: Vec::new(),
            next_seq: 1,
        }
    }

    fn push_event(&mut self, event_type: EventType, payload: BTreeMap<String, String>) -> String {
        let event_id = format!("evt_{}_{}", self.run_id, self.next_seq);
        let event = EventEnvelope {
            event_id: event_id.clone(),
            event_type,
            run_id: self.run_id.clone(),
            attempt_id: self.attempt_id,
            timestamp: now_rfc3339_like(),
            seq: self.next_seq,
            payload,
        };
        self.updated_at = event.timestamp.clone();
        self.next_seq += 1;
        self.events.push(event);
        event_id
    }
}

#[derive(Debug, Default)]
pub struct MockRuntime {
    runs: Vec<RunRecord>,
}

impl MockRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&mut self, request: StartRequest) -> Result<StartResult, ContractError> {
        request.policy.validate().map_err(|msg| {
            ContractError::new(ContractErrorCode::InvalidPolicy, msg, false)
        })?;

        if let Some(existing) = self.runs.iter_mut().find(|r| r.run_id == request.run_id) {
            if existing.state.is_terminal() {
                return Err(ContractError::new(
                    ContractErrorCode::AlreadyTerminal,
                    "run is already terminal",
                    false,
                ));
            }

            return Ok(StartResult {
                handle: existing.handle.clone(),
                attempt_id: existing.attempt_id,
                state: existing.state,
            });
        }

        let mut record = RunRecord::new(request.run_id);
        record.push_event(EventType::RunStarted, BTreeMap::new());
        let result = StartResult {
            handle: record.handle.clone(),
            attempt_id: record.attempt_id,
            state: record.state,
        };
        self.runs.push(record);
        Ok(result)
    }

    pub fn stop(&mut self, request: StopRequest) -> Result<StopResult, ContractError> {
        let Some(record) = self.runs.iter_mut().find(|r| r.handle == request.handle) else {
            return Err(ContractError::new(
                ContractErrorCode::NotFound,
                "run handle not found",
                false,
            ));
        };

        if record.state.is_terminal() {
            let terminal_event_id = record
                .events
                .iter()
                .rev()
                .find(|e| e.is_terminal())
                .map(|e| e.event_id.clone())
                .unwrap_or_else(|| "evt_missing_terminal".to_string());
            return Ok(StopResult {
                state: record.state,
                terminal_event_id,
            });
        }

        record.state = RunState::Canceled;
        record.terminal_reason = Some(request.reason);
        record.exit_code = Some(130);
        let terminal_event_id = record.push_event(EventType::RunCanceled, BTreeMap::new());
        Ok(StopResult {
            state: record.state,
            terminal_event_id,
        })
    }

    pub fn inspect(&self, handle: &str) -> Result<RuntimeInspection, ContractError> {
        let Some(record) = self.runs.iter().find(|r| r.handle == handle) else {
            return Err(ContractError::new(
                ContractErrorCode::NotFound,
                "run handle not found",
                false,
            ));
        };

        Ok(RuntimeInspection {
            run_id: record.run_id.clone(),
            attempt_id: record.attempt_id,
            state: record.state,
            active_stage_count: if record.state == RunState::Running { 1 } else { 0 },
            active_microvm_count: if record.state == RunState::Running { 1 } else { 0 },
            started_at: record.started_at.clone(),
            updated_at: record.updated_at.clone(),
            terminal_reason: record.terminal_reason.clone(),
            exit_code: record.exit_code,
        })
    }

    pub fn subscribe_events(
        &self,
        request: SubscribeEventsRequest,
    ) -> Result<Vec<EventEnvelope>, ContractError> {
        let Some(record) = self.runs.iter().find(|r| r.handle == request.handle) else {
            return Err(ContractError::new(
                ContractErrorCode::NotFound,
                "run handle not found",
                false,
            ));
        };

        if let Some(from_event_id) = request.from_event_id {
            if let Some(idx) = record
                .events
                .iter()
                .position(|event| event.event_id == from_event_id)
            {
                return Ok(record.events[idx + 1..].to_vec());
            }
        }

        Ok(record.events.clone())
    }
}

fn now_rfc3339_like() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}Z")
}

#[cfg(test)]
mod tests {
    use super::MockRuntime;
    use crate::contract::{ContractErrorCode, EventType, ExecutionPolicy, RunState};
    use crate::contract::{StartRequest, StopRequest, SubscribeEventsRequest};

    fn policy() -> ExecutionPolicy {
        ExecutionPolicy {
            max_parallel_microvms_per_run: 4,
            max_stage_retries: 1,
            stage_timeout_secs: 300,
            cancel_grace_period_secs: 10,
        }
    }

    #[test]
    fn start_is_idempotent_for_active_run() {
        let mut runtime = MockRuntime::new();
        let req = StartRequest {
            run_id: "run-1".to_string(),
            workflow_spec: "workflow".to_string(),
            policy: policy(),
        };

        let first = runtime.start(req.clone()).expect("first start");
        let second = runtime.start(req).expect("second start");

        assert_eq!(first.handle, second.handle);
        assert_eq!(first.attempt_id, second.attempt_id);
    }

    #[test]
    fn stop_is_idempotent_for_terminal_run() {
        let mut runtime = MockRuntime::new();
        let req = StartRequest {
            run_id: "run-2".to_string(),
            workflow_spec: "workflow".to_string(),
            policy: policy(),
        };
        let started = runtime.start(req).expect("start");

        let stop_req = StopRequest {
            handle: started.handle.clone(),
            reason: "user requested".to_string(),
        };
        let first = runtime.stop(stop_req.clone()).expect("first stop");
        let second = runtime.stop(stop_req).expect("second stop");

        assert_eq!(first.state, RunState::Canceled);
        assert_eq!(first.terminal_event_id, second.terminal_event_id);
    }

    #[test]
    fn subscribe_supports_resume_from_event_id() {
        let mut runtime = MockRuntime::new();
        let req = StartRequest {
            run_id: "run-3".to_string(),
            workflow_spec: "workflow".to_string(),
            policy: policy(),
        };
        let started = runtime.start(req).expect("start");
        let stop = runtime
            .stop(StopRequest {
                handle: started.handle.clone(),
                reason: "cancel".to_string(),
            })
            .expect("stop");

        let resumed = runtime
            .subscribe_events(SubscribeEventsRequest {
                handle: started.handle,
                from_event_id: Some(stop.terminal_event_id),
            })
            .expect("subscribe");

        assert!(resumed.is_empty());
    }

    #[test]
    fn rejects_invalid_policy() {
        let mut runtime = MockRuntime::new();
        let err = runtime
            .start(StartRequest {
                run_id: "run-4".to_string(),
                workflow_spec: "workflow".to_string(),
                policy: ExecutionPolicy {
                    max_parallel_microvms_per_run: 0,
                    max_stage_retries: 1,
                    stage_timeout_secs: 100,
                    cancel_grace_period_secs: 5,
                },
            })
            .expect_err("expected invalid policy");

        assert_eq!(err.code, ContractErrorCode::InvalidPolicy);
    }

    #[test]
    fn emits_expected_terminal_event() {
        let mut runtime = MockRuntime::new();
        let started = runtime
            .start(StartRequest {
                run_id: "run-5".to_string(),
                workflow_spec: "workflow".to_string(),
                policy: policy(),
            })
            .expect("start");
        runtime
            .stop(StopRequest {
                handle: started.handle.clone(),
                reason: "cancel".to_string(),
            })
            .expect("stop");

        let events = runtime
            .subscribe_events(SubscribeEventsRequest {
                handle: started.handle,
                from_event_id: None,
            })
            .expect("subscribe");
        assert!(events.iter().any(|e| e.event_type == EventType::RunStarted));
        assert!(events.iter().any(|e| e.event_type == EventType::RunCanceled));
    }
}

