use std::collections::{BTreeMap, VecDeque};

use super::types::ExecutionAccumulator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedCandidate {
    pub execution_id: String,
    pub candidate_id: String,
    pub created_seq: u64,
}

impl QueuedCandidate {
    pub fn new(execution_id: &str, candidate_id: &str, created_seq: u64) -> Self {
        Self {
            execution_id: execution_id.to_string(),
            candidate_id: candidate_id.to_string(),
            created_seq,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerDecision {
    Enqueued,
    RejectedBudgetExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchGrant {
    pub execution_id: String,
    pub candidate_id: String,
}

#[derive(Debug, Default)]
struct ExecutionQueue {
    paused: bool,
    queued: VecDeque<QueuedCandidate>,
    running: usize,
    max_concurrent: usize,
}

pub struct GlobalScheduler {
    max_concurrent_child_runs: usize,
    active_slots: usize,
    executions: BTreeMap<String, ExecutionQueue>,
}

impl GlobalScheduler {
    pub fn new(max_concurrent_child_runs: usize) -> Self {
        Self {
            max_concurrent_child_runs,
            active_slots: 0,
            executions: BTreeMap::new(),
        }
    }

    pub fn enqueue(&mut self, candidate: QueuedCandidate) {
        self.executions
            .entry(candidate.execution_id.clone())
            .or_insert_with(|| ExecutionQueue {
                paused: false,
                queued: VecDeque::new(),
                running: 0,
                max_concurrent: usize::MAX,
            })
            .queued
            .push_back(candidate);
    }

    pub fn register_execution(
        &mut self,
        execution_id: &str,
        paused: bool,
        running: usize,
        max_concurrent: usize,
    ) {
        let queue = self
            .executions
            .entry(execution_id.to_string())
            .or_insert_with(|| ExecutionQueue {
                paused,
                queued: VecDeque::new(),
                running,
                max_concurrent,
            });
        queue.paused = paused;
        queue.running = running;
        queue.max_concurrent = max_concurrent;
    }

    pub fn enqueue_if_budget_allows(
        &mut self,
        candidate: QueuedCandidate,
        accumulator: &ExecutionAccumulator,
        max_iterations: u32,
    ) -> SchedulerDecision {
        if accumulator.completed_iterations >= max_iterations {
            return SchedulerDecision::RejectedBudgetExceeded;
        }
        self.enqueue(candidate);
        SchedulerDecision::Enqueued
    }

    pub fn next_dispatch(&mut self) -> Option<DispatchGrant> {
        if self.active_slots >= self.max_concurrent_child_runs {
            return None;
        }

        let next = self
            .executions
            .iter()
            .filter(|(_, queue)| !queue.paused)
            .filter(|(_, queue)| queue.running < queue.max_concurrent)
            .filter_map(|(execution_id, queue)| {
                queue.queued.front().map(|candidate| {
                    (
                        candidate.created_seq,
                        execution_id.clone(),
                        candidate.candidate_id.clone(),
                    )
                })
            })
            .min_by_key(|(created_seq, _, _)| *created_seq)?;

        let queue = self.executions.get_mut(&next.1)?;
        queue.queued.pop_front();
        Some(DispatchGrant {
            execution_id: next.1,
            candidate_id: next.2,
        })
    }

    pub fn mark_running(&mut self, grant: &DispatchGrant) {
        if let Some(queue) = self.executions.get_mut(&grant.execution_id) {
            queue.running += 1;
            self.active_slots += 1;
        }
    }

    pub fn release(&mut self, execution_id: &str, _candidate_id: &str) {
        if let Some(queue) = self.executions.get_mut(execution_id) {
            if queue.running > 0 {
                queue.running -= 1;
            }
        }
        if self.active_slots > 0 {
            self.active_slots -= 1;
        }
    }

    pub fn pause_execution(&mut self, execution_id: &str) {
        if let Some(queue) = self.executions.get_mut(execution_id) {
            queue.paused = true;
            self.active_slots = self.active_slots.saturating_sub(queue.running);
            queue.running = 0;
        }
    }

    pub fn execution_queue_depth(&self, execution_id: &str) -> usize {
        self.executions
            .get(execution_id)
            .map(|queue| queue.queued.len())
            .unwrap_or(0)
    }

    pub fn active_slots(&self) -> usize {
        self.active_slots
    }
}
