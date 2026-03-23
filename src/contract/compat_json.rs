#[cfg(feature = "serde")]
use std::collections::BTreeMap;

#[cfg(feature = "serde")]
use serde::Deserialize;

#[cfg(feature = "serde")]
use crate::contract::{
    from_void_box_run, ContractError, ContractErrorCode, ConvertedRunView, VoidBoxPayloadValue,
    VoidBoxRunEventRaw, VoidBoxRunRaw,
};

#[cfg(feature = "serde")]
#[derive(Debug, Clone, Deserialize)]
struct DaemonRunStateJson {
    id: String,
    status: String,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    events: Vec<DaemonRunEventJson>,
}

#[cfg(feature = "serde")]
#[derive(Debug, Clone, Deserialize)]
struct DaemonRunEventJson {
    ts_ms: u64,
    #[serde(default)]
    event_type: String,
    #[serde(default)]
    run_id: Option<String>,
    #[serde(default)]
    seq: Option<u64>,
    #[serde(default)]
    payload: Option<serde_json::Value>,
}

#[cfg(feature = "serde")]
pub fn from_void_box_run_json(run_json: &str) -> Result<ConvertedRunView, ContractError> {
    let run: DaemonRunStateJson = serde_json::from_str(run_json).map_err(|e| {
        ContractError::new(
            ContractErrorCode::InvalidSpec,
            format!("invalid run JSON: {e}"),
            false,
        )
    })?;

    from_void_box_run(&to_raw_run(run))
}

#[cfg(feature = "serde")]
pub fn from_void_box_run_and_events_json(
    run_json: &str,
    events_json: &str,
) -> Result<ConvertedRunView, ContractError> {
    let run: DaemonRunStateJson = serde_json::from_str(run_json).map_err(|e| {
        ContractError::new(
            ContractErrorCode::InvalidSpec,
            format!("invalid run JSON: {e}"),
            false,
        )
    })?;
    let events: Vec<DaemonRunEventJson> = serde_json::from_str(events_json).map_err(|e| {
        ContractError::new(
            ContractErrorCode::InvalidSpec,
            format!("invalid events JSON: {e}"),
            false,
        )
    })?;

    from_void_box_run(&to_raw_run_with_events(run, events))
}

#[cfg(feature = "serde")]
fn to_raw_run(run: DaemonRunStateJson) -> VoidBoxRunRaw {
    let DaemonRunStateJson {
        id,
        status,
        error,
        events,
    } = run;
    VoidBoxRunRaw {
        id,
        status,
        error,
        events: events.into_iter().map(to_raw_event).collect(),
    }
}

#[cfg(feature = "serde")]
fn to_raw_run_with_events(
    run: DaemonRunStateJson,
    events: Vec<DaemonRunEventJson>,
) -> VoidBoxRunRaw {
    let DaemonRunStateJson {
        id, status, error, ..
    } = run;
    VoidBoxRunRaw {
        id,
        status,
        error,
        events: events.into_iter().map(to_raw_event).collect(),
    }
}

#[cfg(feature = "serde")]
fn to_raw_event(event: DaemonRunEventJson) -> VoidBoxRunEventRaw {
    VoidBoxRunEventRaw {
        ts_ms: event.ts_ms,
        event_type: event.event_type,
        run_id: event.run_id,
        seq: event.seq,
        payload: event.payload.map(payload_to_map),
    }
}

#[cfg(feature = "serde")]
fn payload_to_map(value: serde_json::Value) -> BTreeMap<String, VoidBoxPayloadValue> {
    let mut out = BTreeMap::new();
    let serde_json::Value::Object(map) = value.clone() else {
        out.insert("value".to_string(), json_to_payload_value(value));
        return out;
    };

    for (key, value) in map {
        out.insert(key, json_to_payload_value(value));
    }
    out
}

#[cfg(feature = "serde")]
fn json_to_payload_value(value: serde_json::Value) -> VoidBoxPayloadValue {
    match value {
        serde_json::Value::String(v) => VoidBoxPayloadValue::String(v),
        serde_json::Value::Bool(v) => VoidBoxPayloadValue::Bool(v),
        serde_json::Value::Number(num) => {
            if let Some(i) = num.as_i64() {
                VoidBoxPayloadValue::Integer(i)
            } else if let Some(u) = num.as_u64() {
                VoidBoxPayloadValue::Unsigned(u)
            } else if let Some(f) = num.as_f64() {
                VoidBoxPayloadValue::Float(f)
            } else {
                VoidBoxPayloadValue::Unsupported(num.to_string())
            }
        }
        serde_json::Value::Null => VoidBoxPayloadValue::Null,
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            VoidBoxPayloadValue::Unsupported(value.to_string())
        }
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::{from_void_box_run_and_events_json, from_void_box_run_json};
    use crate::contract::{ContractErrorCode, EventType, RunState};
    use std::fs;

    fn fixture(path: &str) -> String {
        fs::read_to_string(path).expect("read fixture")
    }

    #[test]
    fn parses_run_json_success() {
        let run = fixture("fixtures/voidbox_run_success.json");
        let converted = from_void_box_run_json(&run).expect("conversion");
        assert_eq!(converted.inspection.state, RunState::Succeeded);
        assert_eq!(converted.events.len(), 2);
        assert!(converted
            .events
            .iter()
            .any(|e| e.event_type == EventType::RunCompleted));
    }

    #[test]
    fn parses_run_and_events_json_success() {
        let run = fixture("fixtures/voidbox_run_success.json");
        let events = fixture("fixtures/voidbox_run_events_success.json");
        let converted = from_void_box_run_and_events_json(&run, &events).expect("conversion");
        assert_eq!(converted.events.len(), 2);
        assert_eq!(converted.events[0].seq, 1);
        assert_eq!(converted.events[1].seq, 2);
    }

    #[test]
    fn invalid_json_is_invalid_spec() {
        let err = from_void_box_run_json("{bad").expect_err("expected parse error");
        assert_eq!(err.code, ContractErrorCode::InvalidSpec);
    }

    #[test]
    fn unknown_event_is_dropped_in_diagnostics() {
        let run = fixture("fixtures/voidbox_run_unknown_event.json");
        let converted = from_void_box_run_json(&run).expect("conversion");
        assert_eq!(converted.diagnostics.dropped_unknown_event_types, 1);
        assert_eq!(converted.events.len(), 1);
    }

    #[test]
    fn bad_seq_is_invalid_spec() {
        let run = fixture("fixtures/voidbox_run_bad_seq.json");
        let err = from_void_box_run_json(&run).expect_err("expected seq failure");
        assert_eq!(err.code, ContractErrorCode::InvalidSpec);
    }
}
