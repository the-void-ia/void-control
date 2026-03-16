use std::collections::BTreeMap;

use void_control::contract::{
    from_void_box_run, VoidBoxPayloadValue, VoidBoxRunEventRaw, VoidBoxRunRaw,
};

fn main() {
    let mut payload = BTreeMap::new();
    payload.insert(
        "message".to_string(),
        VoidBoxPayloadValue::String("run created".to_string()),
    );

    let raw = VoidBoxRunRaw {
        id: "run-1700000000".to_string(),
        status: "Completed".to_string(),
        error: None,
        events: vec![
            VoidBoxRunEventRaw {
                ts_ms: 1700000000000,
                event_type: "run.started".to_string(),
                run_id: Some("run-1700000000".to_string()),
                seq: Some(1),
                payload: Some(payload),
            },
            VoidBoxRunEventRaw {
                ts_ms: 1700000004500,
                event_type: "run.finished".to_string(),
                run_id: Some("run-1700000000".to_string()),
                seq: Some(2),
                payload: None,
            },
        ],
    };

    match from_void_box_run(&raw) {
        Ok(converted) => {
            println!("inspection: {:#?}", converted.inspection);
            println!("diagnostics: {:#?}", converted.diagnostics);
            println!("events:");
            for event in &converted.events {
                println!("  - {:?}", event);
            }
        }
        Err(err) => {
            eprintln!("conversion error: {:?}", err);
            std::process::exit(1);
        }
    }
}

