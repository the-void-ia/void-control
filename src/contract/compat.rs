use std::collections::BTreeMap;

use crate::contract::{
    ContractError, ContractErrorCode, EventEnvelope, EventSequenceTracker, EventType, RunState,
    RuntimeInspection,
};

#[derive(Debug, Clone, PartialEq)]
pub enum VoidBoxPayloadValue {
    String(String),
    Bool(bool),
    Integer(i64),
    Unsigned(u64),
    Float(f64),
    Null,
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoidBoxRunEventRaw {
    pub ts_ms: u64,
    pub event_type: String,
    pub run_id: Option<String>,
    pub seq: Option<u64>,
    pub payload: Option<BTreeMap<String, VoidBoxPayloadValue>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoidBoxRunRaw {
    pub id: String,
    pub status: String,
    pub error: Option<String>,
    pub events: Vec<VoidBoxRunEventRaw>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConversionDiagnostics {
    pub dropped_unknown_event_types: usize,
    pub dropped_missing_run_id: usize,
    pub seq_fallback_assigned: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedRunView {
    pub inspection: RuntimeInspection,
    pub events: Vec<EventEnvelope>,
    pub diagnostics: ConversionDiagnostics,
}

pub fn map_void_box_status(status: &str) -> Option<RunState> {
    match status.to_ascii_lowercase().as_str() {
        "pending" => Some(RunState::Pending),
        "starting" => Some(RunState::Starting),
        "running" => Some(RunState::Running),
        "completed" | "succeeded" | "success" => Some(RunState::Succeeded),
        "failed" => Some(RunState::Failed),
        "cancelled" | "canceled" => Some(RunState::Canceled),
        _ => None,
    }
}

pub fn map_void_box_event_type(event_type: &str) -> Option<EventType> {
    match event_type {
        "run.started" => Some(EventType::RunStarted),
        "run.finished" => Some(EventType::RunCompleted),
        "run.failed" => Some(EventType::RunFailed),
        "run.cancelled" | "run.canceled" => Some(EventType::RunCanceled),
        _ => None,
    }
}

pub fn from_void_box_run(run: &VoidBoxRunRaw) -> Result<ConvertedRunView, ContractError> {
    let state = map_void_box_status(&run.status).ok_or_else(|| {
        ContractError::new(
            ContractErrorCode::InvalidSpec,
            format!("unknown void-box status '{}'", run.status),
            false,
        )
    })?;

    let mut diagnostics = ConversionDiagnostics::default();
    let mut events = Vec::new();
    let mut last_seq: Option<u64> = None;

    for raw in &run.events {
        let mapped_type = match map_void_box_event_type(&raw.event_type) {
            Some(value) => value,
            None => {
                diagnostics.dropped_unknown_event_types += 1;
                continue;
            }
        };

        if raw.run_id.is_none() {
            diagnostics.dropped_missing_run_id += 1;
            continue;
        }

        let seq = match raw.seq {
            Some(explicit) => explicit,
            None => {
                diagnostics.seq_fallback_assigned += 1;
                last_seq.unwrap_or(0) + 1
            }
        };
        last_seq = Some(seq);

        events.push(EventEnvelope {
            event_id: format!("evt_{}_{}", run.id, seq),
            event_type: mapped_type,
            run_id: run.id.clone(),
            attempt_id: 1,
            timestamp: ts_ms_to_string(raw.ts_ms),
            seq,
            payload: flatten_payload(raw.payload.as_ref()),
        });

    }

    let mut tracker = EventSequenceTracker::default();
    for event in &events {
        tracker.observe(event).map_err(|e| {
            ContractError::new(ContractErrorCode::InvalidSpec, e, false)
        })?;
    }

    let (started_at, updated_at) = derive_started_updated(&events);

    let inspection = RuntimeInspection {
        run_id: run.id.clone(),
        attempt_id: 1,
        state,
        active_stage_count: 0,
        active_microvm_count: 0,
        started_at,
        updated_at,
        terminal_reason: run.error.clone(),
        exit_code: None,
    };

    Ok(ConvertedRunView {
        inspection,
        events,
        diagnostics,
    })
}

fn flatten_payload(
    payload: Option<&BTreeMap<String, VoidBoxPayloadValue>>,
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(payload) = payload else {
        return out;
    };

    for (key, value) in payload {
        let maybe = match value {
            VoidBoxPayloadValue::String(v) => Some(v.clone()),
            VoidBoxPayloadValue::Bool(v) => Some(v.to_string()),
            VoidBoxPayloadValue::Integer(v) => Some(v.to_string()),
            VoidBoxPayloadValue::Unsigned(v) => Some(v.to_string()),
            VoidBoxPayloadValue::Float(v) => Some(v.to_string()),
            VoidBoxPayloadValue::Null => None,
            VoidBoxPayloadValue::Unsupported(_) => None,
        };
        if let Some(value) = maybe {
            out.insert(key.clone(), value);
        }
    }
    out
}

fn derive_started_updated(events: &[EventEnvelope]) -> (String, String) {
    if events.is_empty() {
        return ("0Z".to_string(), "0Z".to_string());
    }

    let mut min = u64::MAX;
    let mut max = 0u64;
    for event in events {
        if let Some(ms) = parse_ts_ms(&event.timestamp) {
            if ms < min {
                min = ms;
            }
            if ms > max {
                max = ms;
            }
        }
    }

    if min == u64::MAX {
        return ("0Z".to_string(), "0Z".to_string());
    }

    (ts_ms_to_string(min), ts_ms_to_string(max))
}

fn parse_ts_ms(ts: &str) -> Option<u64> {
    ts.strip_suffix("ms")?.parse::<u64>().ok()
}

fn ts_ms_to_string(ts_ms: u64) -> String {
    format!("{ts_ms}ms")
}

#[cfg(test)]
mod tests {
    use super::{
        from_void_box_run, map_void_box_event_type, map_void_box_status, VoidBoxPayloadValue,
        VoidBoxRunEventRaw, VoidBoxRunRaw,
    };
    use crate::contract::{ContractErrorCode, EventType, RunState};
    use std::collections::BTreeMap;

    fn make_event(ts_ms: u64, event_type: &str, seq: Option<u64>) -> VoidBoxRunEventRaw {
        VoidBoxRunEventRaw {
            ts_ms,
            event_type: event_type.to_string(),
            run_id: Some("run-1".to_string()),
            seq,
            payload: None,
        }
    }

    #[test]
    fn maps_void_box_status_values() {
        assert_eq!(map_void_box_status("Pending"), Some(RunState::Pending));
        assert_eq!(map_void_box_status("Starting"), Some(RunState::Starting));
        assert_eq!(map_void_box_status("Running"), Some(RunState::Running));
        assert_eq!(map_void_box_status("Completed"), Some(RunState::Succeeded));
        assert_eq!(map_void_box_status("Succeeded"), Some(RunState::Succeeded));
        assert_eq!(map_void_box_status("Failed"), Some(RunState::Failed));
        assert_eq!(map_void_box_status("Cancelled"), Some(RunState::Canceled));
    }

    #[test]
    fn maps_void_box_terminal_event_strings() {
        assert_eq!(
            map_void_box_event_type("run.started"),
            Some(EventType::RunStarted)
        );
        assert_eq!(
            map_void_box_event_type("run.finished"),
            Some(EventType::RunCompleted)
        );
        assert_eq!(
            map_void_box_event_type("run.failed"),
            Some(EventType::RunFailed)
        );
        assert_eq!(
            map_void_box_event_type("run.cancelled"),
            Some(EventType::RunCanceled)
        );
    }

    #[test]
    fn maps_completed_run_to_succeeded_with_terminal_event() {
        let run = VoidBoxRunRaw {
            id: "run-1".to_string(),
            status: "Completed".to_string(),
            error: None,
            events: vec![make_event(1000, "run.finished", Some(1))],
        };

        let converted = from_void_box_run(&run).expect("conversion");
        assert_eq!(converted.inspection.state, RunState::Succeeded);
        assert_eq!(converted.events.len(), 1);
        assert_eq!(converted.events[0].event_type, EventType::RunCompleted);
    }

    #[test]
    fn maps_cancelled_run_status_to_canceled() {
        let run = VoidBoxRunRaw {
            id: "run-1".to_string(),
            status: "Cancelled".to_string(),
            error: Some("stopped".to_string()),
            events: vec![make_event(1000, "run.cancelled", Some(1))],
        };

        let converted = from_void_box_run(&run).expect("conversion");
        assert_eq!(converted.inspection.state, RunState::Canceled);
    }

    #[test]
    fn drops_unknown_event_types_and_counts_them() {
        let run = VoidBoxRunRaw {
            id: "run-1".to_string(),
            status: "Running".to_string(),
            error: None,
            events: vec![
                make_event(1000, "run.started", Some(1)),
                make_event(1001, "workflow.planned", Some(2)),
            ],
        };

        let converted = from_void_box_run(&run).expect("conversion");
        assert_eq!(converted.events.len(), 1);
        assert_eq!(converted.diagnostics.dropped_unknown_event_types, 1);
    }

    #[test]
    fn errors_on_unknown_status() {
        let run = VoidBoxRunRaw {
            id: "run-1".to_string(),
            status: "Queued".to_string(),
            error: None,
            events: vec![],
        };

        let err = from_void_box_run(&run).expect_err("unknown status");
        assert_eq!(err.code, ContractErrorCode::InvalidSpec);
    }

    #[test]
    fn fills_attempt_id_with_one() {
        let run = VoidBoxRunRaw {
            id: "run-1".to_string(),
            status: "Running".to_string(),
            error: None,
            events: vec![make_event(1000, "run.started", Some(1))],
        };

        let converted = from_void_box_run(&run).expect("conversion");
        assert_eq!(converted.inspection.attempt_id, 1);
        assert_eq!(converted.events[0].attempt_id, 1);
    }

    #[test]
    fn uses_fallback_seq_when_missing_and_keeps_monotonicity() {
        let run = VoidBoxRunRaw {
            id: "run-1".to_string(),
            status: "Running".to_string(),
            error: None,
            events: vec![
                make_event(1000, "run.started", None),
                make_event(1001, "run.finished", None),
            ],
        };

        let converted = from_void_box_run(&run).expect("conversion");
        assert_eq!(converted.events[0].seq, 1);
        assert_eq!(converted.events[1].seq, 2);
        assert_eq!(converted.diagnostics.seq_fallback_assigned, 2);
    }

    #[test]
    fn errors_when_explicit_seq_is_non_monotonic() {
        let run = VoidBoxRunRaw {
            id: "run-1".to_string(),
            status: "Running".to_string(),
            error: None,
            events: vec![
                make_event(1000, "run.started", Some(2)),
                make_event(1001, "run.finished", Some(1)),
            ],
        };

        let err = from_void_box_run(&run).expect_err("non monotonic");
        assert_eq!(err.code, ContractErrorCode::InvalidSpec);
    }

    #[test]
    fn inspection_timestamps_come_from_event_bounds() {
        let run = VoidBoxRunRaw {
            id: "run-1".to_string(),
            status: "Running".to_string(),
            error: None,
            events: vec![
                make_event(1200, "run.started", Some(1)),
                make_event(1100, "run.finished", Some(2)),
            ],
        };

        let converted = from_void_box_run(&run).expect("conversion");
        assert_eq!(converted.inspection.started_at, "1100ms");
        assert_eq!(converted.inspection.updated_at, "1200ms");
    }

    #[test]
    fn terminal_reason_from_run_error() {
        let run = VoidBoxRunRaw {
            id: "run-1".to_string(),
            status: "Failed".to_string(),
            error: Some("boom".to_string()),
            events: vec![make_event(1000, "run.failed", Some(1))],
        };

        let converted = from_void_box_run(&run).expect("conversion");
        assert_eq!(converted.inspection.terminal_reason.as_deref(), Some("boom"));
    }

    #[test]
    fn drops_missing_run_id() {
        let mut bad = make_event(1000, "run.started", Some(1));
        bad.run_id = None;

        let run = VoidBoxRunRaw {
            id: "run-1".to_string(),
            status: "Running".to_string(),
            error: None,
            events: vec![bad],
        };

        let converted = from_void_box_run(&run).expect("conversion");
        assert!(converted.events.is_empty());
        assert_eq!(converted.diagnostics.dropped_missing_run_id, 1);
    }

    #[test]
    fn flattens_scalar_payload_values() {
        let mut payload = BTreeMap::new();
        payload.insert("a".to_string(), VoidBoxPayloadValue::String("x".to_string()));
        payload.insert("b".to_string(), VoidBoxPayloadValue::Bool(true));
        payload.insert("c".to_string(), VoidBoxPayloadValue::Unsupported("{}".to_string()));

        let run = VoidBoxRunRaw {
            id: "run-1".to_string(),
            status: "Running".to_string(),
            error: None,
            events: vec![VoidBoxRunEventRaw {
                ts_ms: 1000,
                event_type: "run.started".to_string(),
                run_id: Some("run-1".to_string()),
                seq: Some(1),
                payload: Some(payload),
            }],
        };

        let converted = from_void_box_run(&run).expect("conversion");
        assert_eq!(converted.events[0].payload.get("a"), Some(&"x".to_string()));
        assert_eq!(converted.events[0].payload.get("b"), Some(&"true".to_string()));
        assert!(!converted.events[0].payload.contains_key("c"));
    }
}
