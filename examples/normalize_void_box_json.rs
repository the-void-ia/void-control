#[cfg(feature = "serde")]
fn main() {
    let run_json = r#"{
        "id":"run-5000",
        "status":"Completed",
        "error":null,
        "events":[
            {"ts_ms":1700000000000,"event_type":"run.started","run_id":"run-5000","seq":1},
            {"ts_ms":1700000003000,"event_type":"run.finished","run_id":"run-5000","seq":2}
        ]
    }"#;

    let converted = void_control::contract::from_void_box_run_json(run_json)
        .expect("json normalization should succeed");
    println!("inspection: {:#?}", converted.inspection);
    println!("events: {:#?}", converted.events);
}

#[cfg(not(feature = "serde"))]
fn main() {
    eprintln!("re-run with: cargo run --features serde --example normalize_void_box_json");
}
