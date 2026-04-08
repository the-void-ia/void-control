use std::io;

use super::store::ExecutionStore;
use super::types::{CandidateStatus, ExecutionCandidate, ExecutionSnapshot, ExecutionStatus};

pub struct ReconciliationService<S> {
    store: S,
}

impl<S> ReconciliationService<S>
where
    S: ExecutionStore,
{
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn reload_active_executions(&self) -> io::Result<Vec<ExecutionSnapshot>> {
        self.store
            .list_active_execution_ids()?
            .into_iter()
            .map(|execution_id| self.store.load_execution(&execution_id))
            .collect()
    }

    pub fn reload_queued_candidates(&self) -> io::Result<Vec<ExecutionCandidate>> {
        let mut queued = Vec::new();
        for snapshot in self.reload_active_executions()? {
            if snapshot.execution.status == ExecutionStatus::Paused {
                continue;
            }
            queued.extend(
                snapshot
                    .candidates
                    .into_iter()
                    .filter(|candidate| candidate.status == CandidateStatus::Queued),
            );
        }
        queued.sort_by_key(|candidate| candidate.created_seq);
        Ok(queued)
    }
}
