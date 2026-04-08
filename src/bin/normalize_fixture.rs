use std::collections::BTreeMap;
use std::env;
use std::fs;

use void_control::contract::{
    from_void_box_run, VoidBoxPayloadValue, VoidBoxRunEventRaw, VoidBoxRunRaw,
};

fn main() {
    let path = match env::args().nth(1) {
        Some(value) => value,
        None => {
            eprintln!("usage: cargo run --bin normalize_fixture -- <fixture-path>");
            std::process::exit(2);
        }
    };

    let text = match fs::read_to_string(&path) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("failed to read fixture '{}': {}", path, err);
            std::process::exit(1);
        }
    };

    let raw = match parse_fixture(&text) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("fixture parse error: {}", err);
            std::process::exit(1);
        }
    };

    match from_void_box_run(&raw) {
        Ok(converted) => {
            println!("inspection: {:#?}", converted.inspection);
            println!("diagnostics: {:#?}", converted.diagnostics);
            println!("events:");
            for event in converted.events {
                println!("  - {:?}", event);
            }
        }
        Err(err) => {
            eprintln!("conversion error: {:?}", err);
            std::process::exit(1);
        }
    }
}

fn parse_fixture(input: &str) -> Result<VoidBoxRunRaw, String> {
    let mut id: Option<String> = None;
    let mut status: Option<String> = None;
    let mut error: Option<String> = None;
    let mut events = Vec::new();

    for (idx, line) in input.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(value) = trimmed.strip_prefix("id=") {
            id = Some(value.to_string());
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("status=") {
            status = Some(value.to_string());
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("error=") {
            error = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("event|") {
            events.push(parse_event_line(value, line_no)?);
            continue;
        }

        return Err(format!("line {}: unknown directive '{}'", line_no, trimmed));
    }

    let id = id.ok_or_else(|| "missing required field 'id='".to_string())?;
    let status = status.ok_or_else(|| "missing required field 'status='".to_string())?;

    Ok(VoidBoxRunRaw {
        id,
        status,
        error,
        events,
    })
}

fn parse_event_line(value: &str, line_no: usize) -> Result<VoidBoxRunEventRaw, String> {
    let mut ts_ms: Option<u64> = None;
    let mut event_type: Option<String> = None;
    let mut run_id: Option<String> = None;
    let mut seq: Option<u64> = None;
    let mut payload: Option<BTreeMap<String, VoidBoxPayloadValue>> = None;

    for part in value.split('|') {
        let (key, raw) = part
            .split_once('=')
            .ok_or_else(|| format!("line {}: invalid event token '{}'", line_no, part))?;

        match key {
            "ts_ms" => {
                ts_ms = Some(
                    raw.parse::<u64>()
                        .map_err(|_| format!("line {}: invalid ts_ms '{}'", line_no, raw))?,
                );
            }
            "event_type" => event_type = Some(raw.to_string()),
            "run_id" => {
                if !raw.is_empty() {
                    run_id = Some(raw.to_string());
                }
            }
            "seq" => {
                if !raw.is_empty() {
                    seq = Some(
                        raw.parse::<u64>()
                            .map_err(|_| format!("line {}: invalid seq '{}'", line_no, raw))?,
                    );
                }
            }
            "payload" => {
                if !raw.is_empty() {
                    payload = Some(parse_payload(raw, line_no)?);
                }
            }
            _ => {
                return Err(format!(
                    "line {}: unsupported event field '{}'",
                    line_no, key
                ));
            }
        }
    }

    Ok(VoidBoxRunEventRaw {
        ts_ms: ts_ms.ok_or_else(|| format!("line {}: event missing ts_ms", line_no))?,
        event_type: event_type
            .ok_or_else(|| format!("line {}: event missing event_type", line_no))?,
        run_id,
        seq,
        payload,
    })
}

fn parse_payload(
    value: &str,
    line_no: usize,
) -> Result<BTreeMap<String, VoidBoxPayloadValue>, String> {
    let mut map = BTreeMap::new();
    for pair in value.split(',') {
        let (key, raw) = pair
            .split_once(':')
            .ok_or_else(|| format!("line {}: invalid payload pair '{}'", line_no, pair))?;
        map.insert(key.to_string(), parse_payload_value(raw));
    }
    Ok(map)
}

fn parse_payload_value(raw: &str) -> VoidBoxPayloadValue {
    if raw.eq_ignore_ascii_case("null") {
        return VoidBoxPayloadValue::Null;
    }
    if raw.eq_ignore_ascii_case("true") {
        return VoidBoxPayloadValue::Bool(true);
    }
    if raw.eq_ignore_ascii_case("false") {
        return VoidBoxPayloadValue::Bool(false);
    }
    if let Ok(value) = raw.parse::<i64>() {
        return VoidBoxPayloadValue::Integer(value);
    }
    if let Ok(value) = raw.parse::<u64>() {
        return VoidBoxPayloadValue::Unsigned(value);
    }
    if let Ok(value) = raw.parse::<f64>() {
        return VoidBoxPayloadValue::Float(value);
    }
    VoidBoxPayloadValue::String(raw.to_string())
}
