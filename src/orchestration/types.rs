#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "serde")]
use serde_json::Value;

#[cfg_attr(feature = "serde", derive(Serialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Canceled,
}

#[cfg_attr(feature = "serde", derive(Serialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Execution {
    pub execution_id: String,
    pub mode: String,
    pub goal: String,
    pub status: ExecutionStatus,
    pub result_best_candidate_id: Option<String>,
    pub completed_iterations: u32,
    pub failure_counts: FailureCounts,
}

impl Execution {
    pub fn new(execution_id: &str, mode: &str, goal: &str) -> Self {
        Self {
            execution_id: execution_id.to_string(),
            mode: mode.to_string(),
            goal: goal.to_string(),
            status: ExecutionStatus::Pending,
            result_best_candidate_id: None,
            completed_iterations: 0,
            failure_counts: FailureCounts::default(),
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Canceled,
}

#[cfg_attr(feature = "serde", derive(Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionCandidate {
    pub execution_id: String,
    pub candidate_id: String,
    pub created_seq: u64,
    pub iteration: u32,
    pub status: CandidateStatus,
    pub runtime_run_id: Option<String>,
    pub overrides: std::collections::BTreeMap<String, String>,
    pub succeeded: Option<bool>,
    pub metrics: std::collections::BTreeMap<String, f64>,
}

impl ExecutionCandidate {
    pub fn new(
        execution_id: &str,
        candidate_id: &str,
        created_seq: u64,
        iteration: u32,
        status: CandidateStatus,
    ) -> Self {
        Self {
            execution_id: execution_id.to_string(),
            candidate_id: candidate_id.to_string(),
            created_seq,
            iteration,
            status,
            runtime_run_id: None,
            overrides: std::collections::BTreeMap::new(),
            succeeded: None,
            metrics: std::collections::BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutionAccumulator {
    pub scoring_history_len: u32,
    pub completed_iterations: u32,
    pub message_backlog: Vec<String>,
    pub leader_proposals: Vec<crate::orchestration::variation::VariationProposal>,
    pub iterations_without_improvement: u32,
    pub best_candidate_id: Option<String>,
    pub best_candidate_overrides: std::collections::BTreeMap<String, String>,
    pub search_phase: Option<String>,
    pub explored_signatures: Vec<String>,
    pub failure_counts: FailureCounts,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionSnapshot {
    pub execution: Execution,
    pub events: Vec<crate::orchestration::events::ControlEventEnvelope>,
    pub accumulator: ExecutionAccumulator,
    pub candidates: Vec<ExecutionCandidate>,
}

#[cfg_attr(feature = "serde", derive(Serialize))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FailureCounts {
    pub total_candidate_failures: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateInbox {
    pub candidate_id: String,
    pub messages: Vec<String>,
}

impl CandidateInbox {
    pub fn new(candidate_id: &str) -> Self {
        Self {
            candidate_id: candidate_id.to_string(),
            messages: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateSpec {
    pub candidate_id: String,
    pub overrides: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CandidateOutput {
    pub candidate_id: String,
    pub succeeded: bool,
    pub metrics: std::collections::BTreeMap<String, f64>,
    #[cfg(feature = "serde")]
    pub intents: Vec<CommunicationIntent>,
}

impl CandidateOutput {
    pub fn new(
        candidate_id: impl Into<String>,
        succeeded: bool,
        metrics: std::collections::BTreeMap<String, f64>,
    ) -> Self {
        Self {
            candidate_id: candidate_id.into(),
            succeeded,
            metrics,
            #[cfg(feature = "serde")]
            intents: Vec::new(),
        }
    }

    #[cfg(feature = "serde")]
    pub fn with_intents(mut self, intents: Vec<CommunicationIntent>) -> Self {
        self.intents = intents;
        self
    }
}

#[cfg(feature = "serde")]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationIntentKind {
    Proposal,
    Signal,
    Evaluation,
}

#[cfg(feature = "serde")]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationIntentAudience {
    Leader,
    Broadcast,
}

#[cfg(feature = "serde")]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationIntentPriority {
    Low,
    Normal,
    High,
}

#[cfg(feature = "serde")]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct CommunicationIntent {
    pub intent_id: String,
    pub from_candidate_id: String,
    pub iteration: u32,
    pub kind: CommunicationIntentKind,
    pub audience: CommunicationIntentAudience,
    pub payload: Value,
    pub priority: CommunicationIntentPriority,
    pub ttl_iterations: u32,
    pub caused_by: Option<String>,
    pub context: Option<Value>,
}

#[cfg(feature = "serde")]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoutedMessageStatus {
    Routed,
    Delivered,
    Expired,
    Dropped,
}

#[cfg(feature = "serde")]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct RoutedMessage {
    pub message_id: String,
    pub intent_id: String,
    pub to: String,
    pub delivery_iteration: u32,
    pub routing_reason: String,
    pub status: RoutedMessageStatus,
}

#[cfg(feature = "serde")]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct InboxEntry {
    pub message_id: String,
    pub intent_id: String,
    pub from_candidate_id: String,
    pub kind: CommunicationIntentKind,
    pub payload: Value,
}

#[cfg(feature = "serde")]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct InboxSnapshot {
    pub execution_id: String,
    pub candidate_id: String,
    pub iteration: u32,
    pub entries: Vec<InboxEntry>,
}
