use crate::contract::{ExecutionPolicy, RunState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartRequest {
    pub run_id: String,
    pub workflow_spec: String,
    pub launch_context: Option<String>,
    pub policy: ExecutionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartResult {
    pub handle: String,
    pub attempt_id: u32,
    pub state: RunState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopRequest {
    pub handle: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopResult {
    pub state: RunState,
    pub terminal_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeInspection {
    pub run_id: String,
    pub attempt_id: u32,
    pub state: RunState,
    pub active_stage_count: u32,
    pub active_microvm_count: u32,
    pub started_at: String,
    pub updated_at: String,
    pub terminal_reason: Option<String>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscribeEventsRequest {
    pub handle: String,
    pub from_event_id: Option<String>,
}
