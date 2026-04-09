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

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerReviewStatus {
    PendingReview,
    Approved,
    RevisionRequested,
    RetryRequested,
    Rejected,
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
    pub review_status: Option<WorkerReviewStatus>,
    pub revision_round: u32,
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
            review_status: None,
            revision_round: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutionAccumulator {
    pub scoring_history_len: u32,
    pub completed_iterations: u32,
    pub leader_proposals: Vec<crate::orchestration::variation::VariationProposal>,
    pub iterations_without_improvement: u32,
    pub best_candidate_id: Option<String>,
    pub best_candidate_overrides: std::collections::BTreeMap<String, String>,
    pub failure_counts: FailureCounts,
    pub supervision_reviews: std::collections::BTreeMap<String, WorkerReviewStatus>,
    pub supervision_revision_rounds: std::collections::BTreeMap<String, u32>,
    pub supervision_final_approval: Option<bool>,
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

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MessageStats {
    pub iteration: u32,
    pub total_messages: usize,
    pub leader_messages: usize,
    pub broadcast_messages: usize,
    pub proposal_count: usize,
    pub signal_count: usize,
    pub evaluation_count: usize,
    pub high_priority_count: usize,
    pub normal_priority_count: usize,
    pub low_priority_count: usize,
    pub delivered_count: usize,
    pub dropped_count: usize,
    pub expired_count: usize,
    pub unique_sources: usize,
    pub unique_intent_count: usize,
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
