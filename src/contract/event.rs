use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventType {
    RunStarted,
    StageStarted,
    StageCompleted,
    StageFailed,
    MicroVmSpawned,
    MicroVmExited,
    RunCompleted,
    RunFailed,
    RunCanceled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventEnvelope {
    pub event_id: String,
    pub event_type: EventType,
    pub run_id: String,
    pub attempt_id: u32,
    pub timestamp: String,
    pub seq: u64,
    pub payload: BTreeMap<String, String>,
}

impl EventEnvelope {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.event_type,
            EventType::RunCompleted | EventType::RunFailed | EventType::RunCanceled
        )
    }
}

#[derive(Debug, Default)]
pub struct EventSequenceTracker {
    last_seq: Option<u64>,
}

impl EventSequenceTracker {
    pub fn observe(&mut self, event: &EventEnvelope) -> Result<(), &'static str> {
        if let Some(last_seq) = self.last_seq {
            if event.seq <= last_seq {
                return Err("event sequence must be strictly increasing");
            }
        }
        self.last_seq = Some(event.seq);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{EventEnvelope, EventSequenceTracker, EventType};
    use std::collections::BTreeMap;

    fn event(seq: u64) -> EventEnvelope {
        EventEnvelope {
            event_id: format!("evt_{seq}"),
            event_type: EventType::StageStarted,
            run_id: "run_1".to_string(),
            attempt_id: 1,
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            seq,
            payload: BTreeMap::new(),
        }
    }

    #[test]
    fn enforces_increasing_sequence() {
        let mut tracker = EventSequenceTracker::default();
        assert!(tracker.observe(&event(1)).is_ok());
        assert!(tracker.observe(&event(2)).is_ok());
        assert!(tracker.observe(&event(2)).is_err());
    }

    #[test]
    fn marks_terminal_types() {
        let mut completed = event(1);
        completed.event_type = EventType::RunCompleted;
        assert!(completed.is_terminal());
    }
}
