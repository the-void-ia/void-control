use super::types::{Execution, ExecutionAccumulator, ExecutionSnapshot, ExecutionStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlEventType {
    ExecutionCreated,
    ExecutionSubmitted,
    ExecutionStarted,
    IterationStarted,
    CandidateQueued,
    CandidateDispatched,
    CandidateOutputCollected,
    CandidateScored,
    IterationCompleted,
    ExecutionCompleted,
    ExecutionFailed,
    ExecutionPaused,
    ExecutionResumed,
    ExecutionCanceled,
    ExecutionStalled,
    CommunicationIntentEmitted,
    CommunicationIntentRejected,
    MessageRouted,
    MessageDelivered,
    MessageExpired,
}

impl ControlEventType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExecutionCreated => "ExecutionCreated",
            Self::ExecutionSubmitted => "ExecutionSubmitted",
            Self::ExecutionStarted => "ExecutionStarted",
            Self::IterationStarted => "IterationStarted",
            Self::CandidateQueued => "CandidateQueued",
            Self::CandidateDispatched => "CandidateDispatched",
            Self::CandidateOutputCollected => "CandidateOutputCollected",
            Self::CandidateScored => "CandidateScored",
            Self::IterationCompleted => "IterationCompleted",
            Self::ExecutionCompleted => "ExecutionCompleted",
            Self::ExecutionFailed => "ExecutionFailed",
            Self::ExecutionPaused => "ExecutionPaused",
            Self::ExecutionResumed => "ExecutionResumed",
            Self::ExecutionCanceled => "ExecutionCanceled",
            Self::ExecutionStalled => "ExecutionStalled",
            Self::CommunicationIntentEmitted => "CommunicationIntentEmitted",
            Self::CommunicationIntentRejected => "CommunicationIntentRejected",
            Self::MessageRouted => "MessageRouted",
            Self::MessageDelivered => "MessageDelivered",
            Self::MessageExpired => "MessageExpired",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ExecutionCreated" => Some(Self::ExecutionCreated),
            "ExecutionSubmitted" => Some(Self::ExecutionSubmitted),
            "ExecutionStarted" => Some(Self::ExecutionStarted),
            "IterationStarted" => Some(Self::IterationStarted),
            "CandidateQueued" => Some(Self::CandidateQueued),
            "CandidateDispatched" => Some(Self::CandidateDispatched),
            "CandidateOutputCollected" => Some(Self::CandidateOutputCollected),
            "CandidateScored" => Some(Self::CandidateScored),
            "IterationCompleted" => Some(Self::IterationCompleted),
            "ExecutionCompleted" => Some(Self::ExecutionCompleted),
            "ExecutionFailed" => Some(Self::ExecutionFailed),
            "ExecutionPaused" => Some(Self::ExecutionPaused),
            "ExecutionResumed" => Some(Self::ExecutionResumed),
            "ExecutionCanceled" => Some(Self::ExecutionCanceled),
            "ExecutionStalled" => Some(Self::ExecutionStalled),
            "CommunicationIntentEmitted" => Some(Self::CommunicationIntentEmitted),
            "CommunicationIntentRejected" => Some(Self::CommunicationIntentRejected),
            "MessageRouted" => Some(Self::MessageRouted),
            "MessageDelivered" => Some(Self::MessageDelivered),
            "MessageExpired" => Some(Self::MessageExpired),
            _ => None,
        }
    }

    pub fn advances_state(self) -> bool {
        !matches!(
            self,
            Self::ExecutionSubmitted
                | Self::CandidateQueued
                | Self::CandidateDispatched
                | Self::CandidateOutputCollected
                | Self::ExecutionStalled
                | Self::CommunicationIntentEmitted
                | Self::CommunicationIntentRejected
                | Self::MessageRouted
                | Self::MessageDelivered
                | Self::MessageExpired
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlEventEnvelope {
    pub execution_id: String,
    pub seq: u64,
    pub event_type: ControlEventType,
}

impl ControlEventEnvelope {
    pub fn new(execution_id: &str, seq: u64, event_type: ControlEventType) -> Self {
        Self {
            execution_id: execution_id.to_string(),
            seq,
            event_type,
        }
    }
}

impl ExecutionSnapshot {
    pub fn replay(mut execution: Execution, events: &[ControlEventEnvelope]) -> ExecutionSnapshot {
        let mut accumulator = ExecutionAccumulator::default();

        for event in events {
            match event.event_type {
                ControlEventType::ExecutionCreated | ControlEventType::ExecutionSubmitted => {}
                ControlEventType::ExecutionStarted | ControlEventType::IterationStarted => {
                    execution.status = ExecutionStatus::Running;
                }
                ControlEventType::CandidateQueued
                | ControlEventType::CandidateDispatched
                | ControlEventType::CandidateOutputCollected => {}
                ControlEventType::CandidateScored => {
                    accumulator.scoring_history_len += 1;
                }
                ControlEventType::IterationCompleted => {
                    accumulator.completed_iterations += 1;
                }
                ControlEventType::ExecutionCompleted => {
                    execution.status = ExecutionStatus::Completed;
                }
                ControlEventType::ExecutionFailed => {
                    execution.status = ExecutionStatus::Failed;
                }
                ControlEventType::ExecutionPaused => {
                    execution.status = ExecutionStatus::Paused;
                }
                ControlEventType::ExecutionResumed => {
                    execution.status = ExecutionStatus::Running;
                }
                ControlEventType::ExecutionCanceled => {
                    execution.status = ExecutionStatus::Canceled;
                }
                ControlEventType::ExecutionStalled
                | ControlEventType::CommunicationIntentEmitted
                | ControlEventType::CommunicationIntentRejected
                | ControlEventType::MessageRouted
                | ControlEventType::MessageDelivered
                | ControlEventType::MessageExpired => {}
            }
        }

        ExecutionSnapshot {
            execution,
            events: events.to_vec(),
            accumulator,
            candidates: Vec::new(),
        }
    }
}
