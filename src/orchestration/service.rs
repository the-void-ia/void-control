use std::io;
#[cfg(feature = "serde")]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::contract::{
    ContractError, ExecutionPolicy, RuntimeInspection, StartRequest, StartResult,
};

use super::events::{ControlEventEnvelope, ControlEventType};
#[cfg(feature = "serde")]
use super::message_box;
use super::policy::GlobalConfig;
use super::scoring::{MetricDirection, ScoringConfig, WeightedMetric};
use super::spec::ExecutionSpec;
use super::store::FsExecutionStore;
use super::strategy::{
    IterationEvaluation, StopReason, SupervisionEvaluation, SupervisionStrategy, SwarmStrategy,
};
use super::types::{
    CandidateOutput, CandidateSpec, CandidateStatus, Execution, ExecutionAccumulator,
    ExecutionCandidate, ExecutionStatus, MessageStats, WorkerReviewStatus,
};

#[cfg(feature = "serde")]
use crate::runtime::{LaunchInjectionAdapter, MessageDeliveryAdapter, ProviderLaunchAdapter};
#[cfg(feature = "serde")]
use serde::Serialize;

pub trait ExecutionRuntime {
    fn start_run(&mut self, request: StartRequest) -> Result<StartResult, ContractError>;
    fn inspect_run(&self, handle: &str) -> Result<RuntimeInspection, ContractError>;
    fn take_structured_output(&mut self, run_id: &str) -> StructuredOutputResult;

    fn inline_poll_budget(&self) -> usize {
        40
    }

    fn inline_poll_sleep_ms(&self) -> u64 {
        100
    }

    fn persisted_run_handle(&self, persisted_run_id: &str) -> String {
        persisted_run_id.to_string()
    }

    #[cfg(feature = "serde")]
    fn delivery_run_ref(&self, _handle: &str) -> Option<crate::runtime::VoidBoxRunRef> {
        None
    }
}

#[derive(Debug, Clone)]
pub enum StructuredOutputResult {
    Found(CandidateOutput),
    Missing,
    Error(ContractError),
}

pub struct ExecutionService<R> {
    global: GlobalConfig,
    runtime: R,
    store: FsExecutionStore,
    #[cfg(feature = "serde")]
    launch_adapter: Box<dyn ProviderLaunchAdapter>,
    #[cfg(feature = "serde")]
    delivery_adapter: Option<Box<dyn MessageDeliveryAdapter>>,
    next_execution_id: u64,
    next_candidate_id: u64,
}

enum ExecutionControl {
    Continue,
    Paused,
    Canceled,
}

enum RunPollOutcome {
    Terminal(RuntimeInspection),
    InFlight(RuntimeInspection),
    Canceled,
}

enum DispatchOutcome {
    Output {
        output: CandidateOutput,
        failed: bool,
    },
    InFlight,
    Retryable(io::Error),
    Canceled,
}

struct CandidateStateUpdate<'a> {
    execution_id: &'a str,
    candidate_id: &'a str,
    created_seq: u64,
    iteration: u32,
    status: CandidateStatus,
    runtime_run_id: Option<String>,
    overrides: &'a std::collections::BTreeMap<String, String>,
    succeeded: Option<bool>,
    metrics: &'a std::collections::BTreeMap<String, f64>,
    review_status: Option<WorkerReviewStatus>,
    revision_round: u32,
}

#[derive(Debug, Clone)]
enum StrategyEvaluation {
    Swarm(IterationEvaluation),
    Supervision(SupervisionEvaluation),
}

enum SelectedStrategy {
    Swarm(SwarmStrategy),
    Supervision(SupervisionStrategy),
}

impl SelectedStrategy {
    fn new(spec: &ExecutionSpec) -> Self {
        match spec.mode.as_str() {
            "supervision" => {
                let review_policy = spec
                    .supervision
                    .as_ref()
                    .expect("validated supervision spec")
                    .review_policy
                    .clone();
                Self::Supervision(SupervisionStrategy::new(
                    spec.variation.clone(),
                    review_policy,
                ))
            }
            _ => {
                let scoring = scoring_from_spec(spec);
                Self::Swarm(SwarmStrategy::new(
                    spec.variation.clone(),
                    scoring,
                    spec.policy.convergence.clone(),
                ))
            }
        }
    }

    fn plan_candidates(
        &self,
        accumulator: &ExecutionAccumulator,
        inboxes: &[super::types::CandidateInbox],
        message_stats: Option<&MessageStats>,
    ) -> Vec<super::types::CandidateSpec> {
        match self {
            Self::Swarm(strategy) => strategy.plan_candidates(accumulator, inboxes, message_stats),
            Self::Supervision(strategy) => strategy.plan_candidates(accumulator, inboxes),
        }
    }

    fn evaluate(
        &self,
        accumulator: &ExecutionAccumulator,
        outputs: &[CandidateOutput],
    ) -> StrategyEvaluation {
        match self {
            Self::Swarm(strategy) => {
                StrategyEvaluation::Swarm(strategy.evaluate(accumulator, outputs))
            }
            Self::Supervision(strategy) => {
                StrategyEvaluation::Supervision(strategy.evaluate(accumulator, outputs))
            }
        }
    }

    fn reduce(
        &self,
        accumulator: ExecutionAccumulator,
        _planned_candidates: &[CandidateSpec],
        evaluation: &StrategyEvaluation,
    ) -> ExecutionAccumulator {
        match self {
            Self::Swarm(strategy) => {
                let StrategyEvaluation::Swarm(evaluation) = evaluation else {
                    unreachable!("strategy and evaluation mismatch");
                };
                strategy.reduce(accumulator, evaluation.clone())
            }
            Self::Supervision(strategy) => {
                let StrategyEvaluation::Supervision(evaluation) = evaluation else {
                    unreachable!("strategy and evaluation mismatch");
                };
                strategy.reduce(accumulator, evaluation)
            }
        }
    }

    fn should_stop(
        &self,
        accumulator: &ExecutionAccumulator,
        evaluation: &StrategyEvaluation,
    ) -> Option<StopReason> {
        match self {
            Self::Swarm(strategy) => {
                let StrategyEvaluation::Swarm(evaluation) = evaluation else {
                    unreachable!("strategy and evaluation mismatch");
                };
                strategy.should_stop(accumulator, evaluation)
            }
            Self::Supervision(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionAction {
    Pause,
    Resume,
    Cancel,
}

#[cfg(feature = "serde")]
#[derive(Debug, Clone, Default)]
pub struct PolicyPatch {
    pub max_iterations: Option<u32>,
    pub max_concurrent_candidates: Option<u32>,
}

#[cfg_attr(feature = "serde", derive(Serialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryRunPlan {
    pub candidates_per_iteration: u32,
    pub max_iterations: Option<u32>,
    pub max_child_runs: Option<u32>,
    pub estimated_concurrent_peak: u32,
    pub variation_source: String,
    pub parameter_space_size: Option<u64>,
}

#[cfg_attr(feature = "serde", derive(Serialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryRunResult {
    pub valid: bool,
    pub plan: DryRunPlan,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl<R> ExecutionService<R>
where
    R: ExecutionRuntime,
{
    #[cfg(feature = "serde")]
    fn with_claimed_execution<T>(
        &mut self,
        execution_id: &str,
        operation: impl FnOnce(&mut Self, &str) -> io::Result<T>,
    ) -> io::Result<T> {
        let worker_id = Self::worker_id();
        if !self.store.claim_execution(execution_id, &worker_id)? {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "execution is already claimed",
            ));
        }

        let release_signal = Arc::new(AtomicBool::new(false));
        let refresh_store = self.store.clone();
        let refresh_execution_id = execution_id.to_string();
        let refresh_worker_id = worker_id.clone();
        let refresh_done = release_signal.clone();
        let refresh_interval_ms = claim_refresh_interval_ms();
        let refresher = std::thread::spawn(move || {
            while !refresh_done.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(refresh_interval_ms));
                if refresh_done.load(Ordering::Relaxed) {
                    break;
                }
                let _ = refresh_store.refresh_claim(&refresh_execution_id, &refresh_worker_id);
            }
        });

        let result = operation(self, &worker_id);
        release_signal.store(true, Ordering::Relaxed);
        let _ = refresher.join();
        let release_result = self.store.release_claim(execution_id);
        match (result, release_result) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(err), Ok(())) => Err(err),
            (Ok(_), Err(err)) | (Err(_), Err(err)) => Err(err),
        }
    }

    #[cfg(feature = "serde")]
    fn load_workable_execution(
        &mut self,
        execution_id: &str,
        invalid_message: &str,
    ) -> io::Result<(super::types::ExecutionSnapshot, ExecutionSpec)> {
        let snapshot = self.store.load_execution(execution_id)?;
        if !matches!(
            snapshot.execution.status,
            ExecutionStatus::Pending | ExecutionStatus::Running
        ) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, invalid_message));
        }
        let spec = self.store.load_spec(execution_id)?;
        Ok((snapshot, spec))
    }

    fn check_execution_control(
        &self,
        execution_id: &str,
        worker_id: &str,
    ) -> io::Result<ExecutionControl> {
        let _ = self.store.refresh_claim(execution_id, worker_id);
        let snapshot = self.store.load_execution(execution_id)?;
        Ok(match snapshot.execution.status {
            ExecutionStatus::Pending | ExecutionStatus::Running => ExecutionControl::Continue,
            ExecutionStatus::Paused => ExecutionControl::Paused,
            ExecutionStatus::Canceled => ExecutionControl::Canceled,
            ExecutionStatus::Completed | ExecutionStatus::Failed => ExecutionControl::Canceled,
        })
    }

    fn wait_for_terminal_run(
        &self,
        execution_id: &str,
        worker_id: &str,
        handle: &str,
    ) -> io::Result<RunPollOutcome> {
        let max_polls = self.runtime.inline_poll_budget().max(1);
        let poll_sleep_ms = self.runtime.inline_poll_sleep_ms();

        let mut last = None;
        for poll_idx in 0..max_polls {
            match self.check_execution_control(execution_id, worker_id)? {
                ExecutionControl::Continue => {}
                ExecutionControl::Paused => {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "execution paused",
                    ));
                }
                ExecutionControl::Canceled => return Ok(RunPollOutcome::Canceled),
            }
            let inspection = self
                .runtime
                .inspect_run(handle)
                .map_err(|err| io::Error::other(err.message.clone()))?;
            if inspection.state.is_terminal() {
                return Ok(RunPollOutcome::Terminal(inspection));
            }
            last = Some(inspection);
            if poll_idx + 1 < max_polls && poll_sleep_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(poll_sleep_ms));
            }
        }
        last.map(RunPollOutcome::InFlight).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::WouldBlock,
                "run did not reach terminal state",
            )
        })
    }

    fn worker_id() -> String {
        format!("pid-{}", std::process::id())
    }

    pub fn new(global: GlobalConfig, runtime: R, store: FsExecutionStore) -> Self {
        Self {
            global,
            runtime,
            store,
            #[cfg(feature = "serde")]
            launch_adapter: Box::new(LaunchInjectionAdapter),
            #[cfg(feature = "serde")]
            delivery_adapter: None,
            next_execution_id: 1,
            next_candidate_id: 1,
        }
    }

    #[cfg(feature = "serde")]
    pub fn with_launch_adapter(
        global: GlobalConfig,
        runtime: R,
        store: FsExecutionStore,
        launch_adapter: Box<dyn ProviderLaunchAdapter>,
    ) -> Self {
        Self {
            global,
            runtime,
            store,
            launch_adapter,
            delivery_adapter: None,
            next_execution_id: 1,
            next_candidate_id: 1,
        }
    }

    #[cfg(feature = "serde")]
    pub fn with_delivery_adapter(
        global: GlobalConfig,
        runtime: R,
        store: FsExecutionStore,
        delivery_adapter: Box<dyn MessageDeliveryAdapter>,
    ) -> Self {
        Self {
            global,
            runtime,
            store,
            launch_adapter: Box::new(LaunchInjectionAdapter),
            delivery_adapter: Some(delivery_adapter),
            next_execution_id: 1,
            next_candidate_id: 1,
        }
    }

    pub fn run_to_completion(&mut self, spec: ExecutionSpec) -> io::Result<Execution> {
        spec.validate(&self.global)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err.to_string()))?;

        let execution_id = format!("exec-{}", self.next_execution_id);
        self.next_execution_id += 1;
        let mut execution = Execution::new(&execution_id, &spec.mode, &spec.goal);
        let worker_id = Self::worker_id();
        self.store.create_execution(&execution)?;
        self.append_event(&execution.execution_id, ControlEventType::ExecutionCreated)?;
        self.append_event(
            &execution.execution_id,
            ControlEventType::ExecutionSubmitted,
        )?;
        execution.status = ExecutionStatus::Running;
        self.store.save_execution(&execution)?;
        self.append_event(&execution.execution_id, ControlEventType::ExecutionStarted)?;
        self.execute_execution(
            &mut execution,
            &spec,
            &ExecutionAccumulator::default(),
            &worker_id,
            None,
            false,
        )
    }

    pub fn dry_run(&self, spec: &ExecutionSpec) -> io::Result<DryRunResult> {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        if let Err(err) = spec.validate(&self.global) {
            errors.push(err.to_string());
        }

        if spec.policy.budget.max_cost_usd_millis.is_none() {
            warnings.push("max_cost_usd not set".to_string());
        }

        let parameter_space_size = if spec.variation.source == "parameter_space" {
            Some(
                spec.variation
                    .parameter_space
                    .values()
                    .map(|values| values.len() as u64)
                    .product(),
            )
        } else {
            None
        };

        let max_child_runs = spec
            .policy
            .budget
            .max_iterations
            .map(|iterations| iterations * spec.variation.candidates_per_iteration);

        Ok(DryRunResult {
            valid: errors.is_empty(),
            plan: DryRunPlan {
                candidates_per_iteration: spec.variation.candidates_per_iteration,
                max_iterations: spec.policy.budget.max_iterations,
                max_child_runs,
                estimated_concurrent_peak: spec.policy.concurrency.max_concurrent_candidates,
                variation_source: spec.variation.source.clone(),
                parameter_space_size,
            },
            warnings,
            errors,
        })
    }

    pub fn submit_execution(
        store: &FsExecutionStore,
        execution_id: &str,
        spec: &ExecutionSpec,
    ) -> io::Result<Execution> {
        let execution = Execution::new(execution_id, &spec.mode, &spec.goal);
        store.create_execution(&execution)?;
        store.append_event(
            execution_id,
            &ControlEventEnvelope::new(execution_id, 1, ControlEventType::ExecutionCreated),
        )?;
        store.append_event(
            execution_id,
            &ControlEventEnvelope::new(execution_id, 2, ControlEventType::ExecutionSubmitted),
        )?;
        #[cfg(feature = "serde")]
        store.save_spec(execution_id, spec)?;
        Ok(execution)
    }

    #[cfg(feature = "serde")]
    pub fn process_execution(&mut self, execution_id: &str) -> io::Result<Execution> {
        self.with_claimed_execution(execution_id, |service, worker_id| {
            service.process_execution_claimed(execution_id, worker_id)
        })
    }

    #[cfg(feature = "serde")]
    pub fn dispatch_execution_once(&mut self, execution_id: &str) -> io::Result<Execution> {
        self.with_claimed_execution(execution_id, |service, worker_id| {
            service.dispatch_execution_once_claimed(execution_id, worker_id, false)
        })
    }

    #[cfg(feature = "serde")]
    pub fn bridge_dispatch_execution_once(&mut self, execution_id: &str) -> io::Result<Execution> {
        self.with_claimed_execution(execution_id, |service, worker_id| {
            service.dispatch_execution_once_claimed(execution_id, worker_id, true)
        })
    }

    #[cfg(feature = "serde")]
    pub fn plan_execution(&mut self, execution_id: &str) -> io::Result<Execution> {
        self.with_claimed_execution(execution_id, |service, _worker_id| {
            service.plan_execution_claimed(execution_id)
        })
    }

    #[cfg(feature = "serde")]
    fn process_execution_claimed(
        &mut self,
        execution_id: &str,
        worker_id: &str,
    ) -> io::Result<Execution> {
        let (snapshot, spec) = self.load_workable_execution(
            execution_id,
            "only pending or running executions can be processed",
        )?;
        let mut execution = snapshot.execution;
        execution.status = ExecutionStatus::Running;
        self.store.save_execution(&execution)?;
        self.append_event(execution_id, ControlEventType::ExecutionStarted)?;
        let accumulator = snapshot.accumulator;
        self.execute_execution(&mut execution, &spec, &accumulator, worker_id, None, false)
    }

    #[cfg(feature = "serde")]
    fn dispatch_execution_once_claimed(
        &mut self,
        execution_id: &str,
        worker_id: &str,
        prefer_queued_when_capacity: bool,
    ) -> io::Result<Execution> {
        let (snapshot, spec) = self.load_workable_execution(
            execution_id,
            "only pending or running executions can be dispatched",
        )?;
        let mut execution = snapshot.execution;
        if execution.status == ExecutionStatus::Pending {
            execution.status = ExecutionStatus::Running;
            self.store.save_execution(&execution)?;
            self.append_event(execution_id, ControlEventType::ExecutionStarted)?;
        }
        let accumulator = snapshot.accumulator;
        self.execute_execution(
            &mut execution,
            &spec,
            &accumulator,
            worker_id,
            Some(1),
            prefer_queued_when_capacity,
        )
    }

    #[cfg(feature = "serde")]
    fn plan_execution_claimed(&mut self, execution_id: &str) -> io::Result<Execution> {
        let (snapshot, spec) = self.load_workable_execution(
            execution_id,
            "only pending or running executions can be planned",
        )?;
        let mut execution = snapshot.execution;
        if execution.status == ExecutionStatus::Pending {
            execution.status = ExecutionStatus::Running;
            self.store.save_execution(&execution)?;
            self.append_event(execution_id, ControlEventType::ExecutionStarted)?;
        }

        let accumulator = snapshot.accumulator;
        let iteration = accumulator.completed_iterations;
        let already_planned = snapshot
            .candidates
            .iter()
            .any(|candidate| candidate.iteration == iteration);
        if already_planned {
            return Ok(execution);
        }

        self.plan_iteration_candidates(&execution, &spec, &accumulator, iteration)?;
        Ok(execution)
    }

    fn append_event(&self, execution_id: &str, event_type: ControlEventType) -> io::Result<()> {
        let seq = self.store.load_execution(execution_id)?.events.len() as u64 + 1;
        self.store.append_event(
            execution_id,
            &ControlEventEnvelope::new(execution_id, seq, event_type),
        )
    }

    fn save_candidate_state(&self, update: CandidateStateUpdate<'_>) -> io::Result<()> {
        let mut record = ExecutionCandidate::new(
            update.execution_id,
            update.candidate_id,
            update.created_seq,
            update.iteration,
            update.status,
        );
        record.runtime_run_id = update.runtime_run_id;
        record.overrides = update.overrides.clone();
        record.succeeded = update.succeeded;
        record.metrics = update.metrics.clone();
        record.review_status = update.review_status;
        record.revision_round = update.revision_round;
        self.store.save_candidate(&record)
    }

    fn persist_supervision_reviews(
        &self,
        execution: &Execution,
        iteration: u32,
        candidates: &[ExecutionCandidate],
        evaluation: &SupervisionEvaluation,
    ) -> io::Result<()> {
        for decision in &evaluation.decisions {
            let Some(candidate) = candidates
                .iter()
                .find(|candidate| candidate.candidate_id == decision.candidate_id)
            else {
                continue;
            };
            self.append_event(&execution.execution_id, ControlEventType::ReviewRequested)?;
            let event_type = match decision.status {
                WorkerReviewStatus::Approved => ControlEventType::WorkerApproved,
                WorkerReviewStatus::RevisionRequested => ControlEventType::RevisionRequested,
                WorkerReviewStatus::RetryRequested => ControlEventType::RevisionRequested,
                WorkerReviewStatus::Rejected => ControlEventType::ExecutionFailed,
                WorkerReviewStatus::PendingReview => continue,
            };
            self.save_candidate_state(CandidateStateUpdate {
                execution_id: &execution.execution_id,
                candidate_id: &candidate.candidate_id,
                created_seq: candidate.created_seq,
                iteration,
                status: candidate.status.clone(),
                runtime_run_id: candidate.runtime_run_id.clone(),
                overrides: &candidate.overrides,
                succeeded: candidate.succeeded,
                metrics: &candidate.metrics,
                review_status: Some(decision.status),
                revision_round: decision.revision_round,
            })?;
            self.append_event(&execution.execution_id, event_type)?;
        }

        Ok(())
    }

    #[cfg(feature = "serde")]
    fn load_launch_inbox_snapshot(
        &self,
        execution_id: &str,
        iteration: u32,
        candidate_id: &str,
    ) -> io::Result<crate::orchestration::InboxSnapshot> {
        match self
            .store
            .load_inbox_snapshot(execution_id, iteration, candidate_id)
        {
            Ok(snapshot) => Ok(snapshot),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                Ok(crate::orchestration::InboxSnapshot {
                    execution_id: execution_id.to_string(),
                    candidate_id: candidate_id.to_string(),
                    iteration,
                    entries: Vec::new(),
                })
            }
            Err(err) => Err(err),
        }
    }

    #[cfg(feature = "serde")]
    fn persist_candidate_intents(
        &self,
        execution_id: &str,
        iteration: u32,
        candidate_id: &str,
        intents: &[super::types::CommunicationIntent],
    ) -> io::Result<()> {
        let (valid, rejected) = message_box::normalize_intents(candidate_id, iteration, intents);
        for _ in 0..rejected {
            self.append_event(execution_id, ControlEventType::CommunicationIntentRejected)?;
        }
        for intent in &valid {
            self.store.append_intent(execution_id, intent)?;
            self.append_event(execution_id, ControlEventType::CommunicationIntentEmitted)?;
        }
        for message in message_box::route_intents(&valid) {
            self.store.append_routed_message(execution_id, &message)?;
            self.append_event(execution_id, ControlEventType::MessageRouted)?;
        }
        Ok(())
    }

    #[cfg(feature = "serde")]
    fn drain_delivery_intents(
        &self,
        handle: &str,
    ) -> io::Result<Vec<super::types::CommunicationIntent>> {
        let Some(adapter) = self.delivery_adapter.as_ref() else {
            return Ok(Vec::new());
        };
        let run_ref = self.runtime.delivery_run_ref(handle).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "delivery adapter configured for runtime without delivery run references",
            )
        })?;
        adapter.drain_intents(&run_ref)
    }

    #[cfg(feature = "serde")]
    fn collect_candidate_intents(
        &self,
        handle: &str,
        output_intents: &[super::types::CommunicationIntent],
    ) -> io::Result<Vec<super::types::CommunicationIntent>> {
        let drained = self.drain_delivery_intents(handle)?;
        Ok(message_box::merge_and_dedup(
            drained,
            output_intents.to_vec(),
        ))
    }

    #[cfg(feature = "serde")]
    fn collect_candidate_intents_best_effort(
        &self,
        handle: &str,
        output_intents: &[super::types::CommunicationIntent],
    ) -> Vec<super::types::CommunicationIntent> {
        match self.collect_candidate_intents(handle, output_intents) {
            Ok(intents) => intents,
            Err(err) => {
                eprintln!("candidate intent drain skipped for '{handle}': {err}");
                output_intents.to_vec()
            }
        }
    }

    #[cfg(feature = "serde")]
    fn materialize_iteration_inboxes(
        &self,
        execution_id: &str,
        iteration: u32,
        inboxes: &[super::types::CandidateInbox],
    ) -> io::Result<()> {
        let intents = self.store.load_intents(execution_id)?;
        let messages = self.store.load_routed_messages(execution_id)?;
        for (snapshot, delivered) in message_box::materialize_inbox_snapshots(
            execution_id,
            iteration,
            inboxes,
            &intents,
            &messages,
        ) {
            self.store.save_inbox_snapshot(&snapshot)?;
            for delivered_message in delivered {
                self.store
                    .append_routed_message(execution_id, &delivered_message)?;
                self.append_event(execution_id, ControlEventType::MessageDelivered)?;
            }
        }
        Ok(())
    }

    fn plan_iteration_candidates(
        &mut self,
        execution: &Execution,
        spec: &ExecutionSpec,
        accumulator: &ExecutionAccumulator,
        iteration: u32,
    ) -> io::Result<Vec<super::types::CandidateSpec>> {
        let strategy = SelectedStrategy::new(spec);
        self.append_event(&execution.execution_id, ControlEventType::IterationStarted)?;
        #[cfg(feature = "serde")]
        let inboxes = {
            let intents = self.store.load_intents(&execution.execution_id)?;
            let messages = self.store.load_routed_messages(&execution.execution_id)?;
            message_box::build_candidate_inboxes(
                iteration,
                spec.variation.candidates_per_iteration as usize,
                &intents,
                &messages,
            )
        };
        #[cfg(not(feature = "serde"))]
        let inboxes = (0..spec.variation.candidates_per_iteration.max(1) as usize)
            .map(|idx| super::types::CandidateInbox::new(&format!("candidate-{}", idx + 1)))
            .collect::<Vec<_>>();
        #[cfg(feature = "serde")]
        self.materialize_iteration_inboxes(&execution.execution_id, iteration, &inboxes)?;
        #[cfg(feature = "serde")]
        let message_stats = Some(self.load_message_stats(&execution.execution_id, iteration)?);
        #[cfg(not(feature = "serde"))]
        let message_stats: Option<MessageStats> = None;
        let candidates = strategy.plan_candidates(accumulator, &inboxes, message_stats.as_ref());
        for candidate in &candidates {
            let candidate_seq = self.next_candidate_id;
            self.save_candidate_state(CandidateStateUpdate {
                execution_id: &execution.execution_id,
                candidate_id: &candidate.candidate_id,
                created_seq: candidate_seq,
                iteration,
                status: CandidateStatus::Queued,
                runtime_run_id: None,
                overrides: &candidate.overrides,
                succeeded: None,
                metrics: &Default::default(),
                review_status: None,
                revision_round: 0,
            })?;
            self.append_event(&execution.execution_id, ControlEventType::CandidateQueued)?;
            if spec.mode == "supervision" {
                self.append_event(&execution.execution_id, ControlEventType::WorkerQueued)?;
            }
            self.next_candidate_id += 1;
        }
        Ok(candidates)
    }

    #[cfg(feature = "serde")]
    fn load_message_stats(&self, execution_id: &str, iteration: u32) -> io::Result<MessageStats> {
        let intents = self.store.load_intents(execution_id)?;
        let messages = self.store.load_routed_messages(execution_id)?;
        Ok(message_box::extract_message_stats(
            &intents, &messages, iteration,
        ))
    }

    fn load_or_plan_iteration_candidates(
        &mut self,
        execution: &Execution,
        spec: &ExecutionSpec,
        accumulator: &ExecutionAccumulator,
        iteration: u32,
    ) -> io::Result<Vec<super::types::CandidateSpec>> {
        let persisted: Vec<_> = self
            .store
            .load_candidates(&execution.execution_id)?
            .into_iter()
            .filter(|candidate| candidate.iteration == iteration)
            .collect();
        if persisted.is_empty() {
            return self.plan_iteration_candidates(execution, spec, accumulator, iteration);
        }

        let mut persisted = persisted;
        persisted.sort_by_key(|candidate| candidate.created_seq);
        Ok(persisted
            .into_iter()
            .map(|candidate| super::types::CandidateSpec {
                candidate_id: candidate.candidate_id,
                overrides: candidate.overrides,
            })
            .collect())
    }

    fn dispatch_candidate(
        &mut self,
        execution: &mut Execution,
        spec: &ExecutionSpec,
        worker_id: &str,
        candidate: &CandidateSpec,
        iteration: u32,
        created_seq: u64,
    ) -> io::Result<DispatchOutcome> {
        let run_id = format!("exec-run-candidate-{created_seq}");
        self.append_event(
            &execution.execution_id,
            ControlEventType::CandidateDispatched,
        )?;
        #[cfg(feature = "serde")]
        let launch_inbox = self.load_launch_inbox_snapshot(
            &execution.execution_id,
            iteration,
            &candidate.candidate_id,
        )?;
        #[cfg(feature = "serde")]
        let mut launch_request = StartRequest {
            run_id: run_id.clone(),
            workflow_spec: spec.workflow.template.clone(),
            launch_context: None,
            policy: default_runtime_policy(),
        };
        #[cfg(feature = "serde")]
        if self.delivery_adapter.is_none() {
            launch_request = self
                .launch_adapter
                .prepare_launch_request(launch_request, candidate, &launch_inbox)
                .map_err(|err| io::Error::other(err.message))?;
        }
        #[cfg(feature = "serde")]
        let started = self
            .runtime
            .start_run(launch_request)
            .map_err(|err| io::Error::other(err.message))?;
        #[cfg(feature = "serde")]
        if let Some(adapter) = self.delivery_adapter.as_ref() {
            let run_ref = self
                .runtime
                .delivery_run_ref(&started.handle)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::Unsupported,
                        "delivery adapter configured for runtime without delivery run references",
                    )
                })?;
            adapter.inject_at_launch(&run_ref, candidate, &launch_inbox)?;
        }
        #[cfg(not(feature = "serde"))]
        let started = self
            .runtime
            .start_run(StartRequest {
                run_id: run_id.clone(),
                workflow_spec: spec.workflow.template.clone(),
                launch_context: None,
                policy: default_runtime_policy(),
            })
            .map_err(|err| io::Error::other(err.message))?;
        self.save_candidate_state(CandidateStateUpdate {
            execution_id: &execution.execution_id,
            candidate_id: &candidate.candidate_id,
            created_seq,
            iteration,
            status: CandidateStatus::Running,
            runtime_run_id: Some(started.handle.clone()),
            overrides: &candidate.overrides,
            succeeded: None,
            metrics: &Default::default(),
            review_status: None,
            revision_round: 0,
        })?;

        let inspection =
            match self.wait_for_terminal_run(&execution.execution_id, worker_id, &started.handle) {
                Ok(RunPollOutcome::Terminal(inspection)) => inspection,
                Ok(RunPollOutcome::Canceled) => {
                    self.save_candidate_state(CandidateStateUpdate {
                        execution_id: &execution.execution_id,
                        candidate_id: &candidate.candidate_id,
                        created_seq,
                        iteration,
                        status: CandidateStatus::Canceled,
                        runtime_run_id: Some(run_id),
                        overrides: &candidate.overrides,
                        succeeded: None,
                        metrics: &Default::default(),
                        review_status: None,
                        revision_round: 0,
                    })?;
                    return Ok(DispatchOutcome::Canceled);
                }
                Ok(RunPollOutcome::InFlight(inspection)) => {
                    return self
                        .try_finalize_candidate_from_ready_output(
                            execution,
                            spec,
                            candidate,
                            created_seq,
                            &started.handle,
                            inspection,
                        )?
                        .map_or(Ok(DispatchOutcome::InFlight), Ok);
                }
                Err(err) => return Err(err),
            };

        self.finalize_candidate_after_terminal_inspection(
            execution,
            spec,
            candidate,
            created_seq,
            &started.handle,
            inspection,
        )
    }

    fn resume_running_candidate(
        &mut self,
        execution: &mut Execution,
        spec: &ExecutionSpec,
        worker_id: &str,
        candidate: &ExecutionCandidate,
    ) -> io::Result<DispatchOutcome> {
        let persisted_run_id = candidate.runtime_run_id.as_deref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "running candidate '{}' missing persisted runtime run id",
                    candidate.candidate_id
                ),
            )
        })?;
        let handle = self.runtime.persisted_run_handle(persisted_run_id);
        let inspection =
            match self.wait_for_terminal_run(&execution.execution_id, worker_id, &handle) {
                Ok(RunPollOutcome::Terminal(inspection)) => inspection,
                Ok(RunPollOutcome::Canceled) => {
                    self.save_candidate_state(CandidateStateUpdate {
                        execution_id: &execution.execution_id,
                        candidate_id: &candidate.candidate_id,
                        created_seq: candidate.created_seq,
                        iteration: candidate.iteration,
                        status: CandidateStatus::Canceled,
                        runtime_run_id: Some(persisted_run_id.to_string()),
                        overrides: &candidate.overrides,
                        succeeded: None,
                        metrics: &Default::default(),
                        review_status: None,
                        revision_round: 0,
                    })?;
                    return Ok(DispatchOutcome::Canceled);
                }
                Ok(RunPollOutcome::InFlight(inspection)) => {
                    return self
                        .try_finalize_candidate_from_ready_output(
                            execution,
                            spec,
                            &CandidateSpec {
                                candidate_id: candidate.candidate_id.clone(),
                                overrides: candidate.overrides.clone(),
                            },
                            candidate.created_seq,
                            &handle,
                            inspection,
                        )?
                        .map_or(Ok(DispatchOutcome::InFlight), Ok);
                }
                Err(err) => return Err(err),
            };

        let candidate_spec = CandidateSpec {
            candidate_id: candidate.candidate_id.clone(),
            overrides: candidate.overrides.clone(),
        };
        self.finalize_candidate_after_terminal_inspection(
            execution,
            spec,
            &candidate_spec,
            candidate.created_seq,
            &handle,
            inspection,
        )
    }

    fn try_finalize_candidate_from_ready_output(
        &mut self,
        execution: &mut Execution,
        spec: &ExecutionSpec,
        candidate: &CandidateSpec,
        created_seq: u64,
        handle: &str,
        inspection: RuntimeInspection,
    ) -> io::Result<Option<DispatchOutcome>> {
        if inspection.state.is_terminal() {
            return self
                .finalize_candidate_after_terminal_inspection(
                    execution,
                    spec,
                    candidate,
                    created_seq,
                    handle,
                    inspection,
                )
                .map(Some);
        }
        if inspection.active_stage_count > 0 || inspection.active_microvm_count > 0 {
            return Ok(None);
        }

        match self.runtime.take_structured_output(&inspection.run_id) {
            StructuredOutputResult::Found(mut output) => {
                #[cfg(feature = "serde")]
                let intents = self.collect_candidate_intents_best_effort(handle, &output.intents);
                self.save_candidate_state(CandidateStateUpdate {
                    execution_id: &execution.execution_id,
                    candidate_id: &candidate.candidate_id,
                    created_seq,
                    iteration: execution.completed_iterations,
                    status: CandidateStatus::Completed,
                    runtime_run_id: Some(inspection.run_id.clone()),
                    overrides: &candidate.overrides,
                    succeeded: Some(output.succeeded),
                    metrics: &output.metrics,
                    review_status: None,
                    revision_round: 0,
                })?;
                output.candidate_id = candidate.candidate_id.clone();
                self.append_event(
                    &execution.execution_id,
                    ControlEventType::CandidateOutputCollected,
                )?;
                #[cfg(feature = "serde")]
                self.persist_candidate_intents(
                    &execution.execution_id,
                    execution.completed_iterations,
                    &candidate.candidate_id,
                    &intents,
                )?;
                Ok(Some(DispatchOutcome::Output {
                    output,
                    failed: false,
                }))
            }
            StructuredOutputResult::Missing => Ok(None),
            StructuredOutputResult::Error(err) => match err.code {
                crate::contract::ContractErrorCode::StructuredOutputMissing
                | crate::contract::ContractErrorCode::ArtifactNotFound
                | crate::contract::ContractErrorCode::NotFound => Ok(None),
                crate::contract::ContractErrorCode::ArtifactPublicationIncomplete
                | crate::contract::ContractErrorCode::ArtifactStoreUnavailable
                | crate::contract::ContractErrorCode::RetrievalTimeout
                    if err.retryable =>
                {
                    Ok(Some(DispatchOutcome::Retryable(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        err.message,
                    ))))
                }
                _ => {
                    self.save_candidate_state(CandidateStateUpdate {
                        execution_id: &execution.execution_id,
                        candidate_id: &candidate.candidate_id,
                        created_seq,
                        iteration: execution.completed_iterations,
                        status: CandidateStatus::Failed,
                        runtime_run_id: Some(inspection.run_id.clone()),
                        overrides: &candidate.overrides,
                        succeeded: Some(false),
                        metrics: &Default::default(),
                        review_status: None,
                        revision_round: 0,
                    })?;
                    self.append_event(
                        &execution.execution_id,
                        ControlEventType::CandidateOutputCollected,
                    )?;
                    Ok(Some(DispatchOutcome::Output {
                        output: CandidateOutput::new(
                            candidate.candidate_id.clone(),
                            false,
                            Default::default(),
                        ),
                        failed: true,
                    }))
                }
            },
        }
    }

    fn finalize_candidate_after_terminal_inspection(
        &mut self,
        execution: &mut Execution,
        spec: &ExecutionSpec,
        candidate: &CandidateSpec,
        created_seq: u64,
        handle: &str,
        inspection: crate::contract::RuntimeInspection,
    ) -> io::Result<DispatchOutcome> {
        match self.runtime.take_structured_output(&inspection.run_id) {
            StructuredOutputResult::Found(mut output) => {
                #[cfg(feature = "serde")]
                let intents = self.collect_candidate_intents_best_effort(handle, &output.intents);
                self.save_candidate_state(CandidateStateUpdate {
                    execution_id: &execution.execution_id,
                    candidate_id: &candidate.candidate_id,
                    created_seq,
                    iteration: execution.completed_iterations,
                    status: CandidateStatus::Completed,
                    runtime_run_id: Some(inspection.run_id.clone()),
                    overrides: &candidate.overrides,
                    succeeded: Some(output.succeeded),
                    metrics: &output.metrics,
                    review_status: None,
                    revision_round: 0,
                })?;
                output.candidate_id = candidate.candidate_id.clone();
                self.append_event(
                    &execution.execution_id,
                    ControlEventType::CandidateOutputCollected,
                )?;
                #[cfg(feature = "serde")]
                self.persist_candidate_intents(
                    &execution.execution_id,
                    execution.completed_iterations,
                    &candidate.candidate_id,
                    &intents,
                )?;
                Ok(DispatchOutcome::Output {
                    output,
                    failed: false,
                })
            }
            StructuredOutputResult::Missing => {
                #[cfg(feature = "serde")]
                let intents = self.collect_candidate_intents_best_effort(handle, &[]);
                let failed = inspection.state == crate::contract::RunState::Failed
                    || spec.policy.missing_output_policy == "mark_failed";
                self.save_candidate_state(CandidateStateUpdate {
                    execution_id: &execution.execution_id,
                    candidate_id: &candidate.candidate_id,
                    created_seq,
                    iteration: execution.completed_iterations,
                    status: if failed {
                        CandidateStatus::Failed
                    } else {
                        CandidateStatus::Completed
                    },
                    runtime_run_id: Some(inspection.run_id.clone()),
                    overrides: &candidate.overrides,
                    succeeded: Some(!failed),
                    metrics: &Default::default(),
                    review_status: None,
                    revision_round: 0,
                })?;
                self.append_event(
                    &execution.execution_id,
                    ControlEventType::CandidateOutputCollected,
                )?;
                #[cfg(feature = "serde")]
                self.persist_candidate_intents(
                    &execution.execution_id,
                    execution.completed_iterations,
                    &candidate.candidate_id,
                    &intents,
                )?;
                Ok(DispatchOutcome::Output {
                    output: CandidateOutput::new(
                        candidate.candidate_id.clone(),
                        !failed,
                        Default::default(),
                    ),
                    failed,
                })
            }
            StructuredOutputResult::Error(err) => match err.code {
                crate::contract::ContractErrorCode::StructuredOutputMissing => {
                    #[cfg(feature = "serde")]
                    let intents = self.collect_candidate_intents_best_effort(handle, &[]);
                    let failed = inspection.state == crate::contract::RunState::Failed
                        || spec.policy.missing_output_policy == "mark_failed";
                    self.save_candidate_state(CandidateStateUpdate {
                        execution_id: &execution.execution_id,
                        candidate_id: &candidate.candidate_id,
                        created_seq,
                        iteration: execution.completed_iterations,
                        status: if failed {
                            CandidateStatus::Failed
                        } else {
                            CandidateStatus::Completed
                        },
                        runtime_run_id: Some(inspection.run_id.clone()),
                        overrides: &candidate.overrides,
                        succeeded: Some(!failed),
                        metrics: &Default::default(),
                        review_status: None,
                        revision_round: 0,
                    })?;
                    self.append_event(
                        &execution.execution_id,
                        ControlEventType::CandidateOutputCollected,
                    )?;
                    #[cfg(feature = "serde")]
                    self.persist_candidate_intents(
                        &execution.execution_id,
                        execution.completed_iterations,
                        &candidate.candidate_id,
                        &intents,
                    )?;
                    Ok(DispatchOutcome::Output {
                        output: CandidateOutput::new(
                            candidate.candidate_id.clone(),
                            !failed,
                            Default::default(),
                        ),
                        failed,
                    })
                }
                crate::contract::ContractErrorCode::ArtifactPublicationIncomplete
                | crate::contract::ContractErrorCode::ArtifactStoreUnavailable
                | crate::contract::ContractErrorCode::RetrievalTimeout
                    if err.retryable =>
                {
                    Ok(DispatchOutcome::Retryable(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        err.message,
                    )))
                }
                _ => {
                    self.save_candidate_state(CandidateStateUpdate {
                        execution_id: &execution.execution_id,
                        candidate_id: &candidate.candidate_id,
                        created_seq,
                        iteration: execution.completed_iterations,
                        status: CandidateStatus::Failed,
                        runtime_run_id: Some(inspection.run_id.clone()),
                        overrides: &candidate.overrides,
                        succeeded: Some(false),
                        metrics: &Default::default(),
                        review_status: None,
                        revision_round: 0,
                    })?;
                    self.append_event(
                        &execution.execution_id,
                        ControlEventType::CandidateOutputCollected,
                    )?;
                    Ok(DispatchOutcome::Output {
                        output: CandidateOutput::new(
                            candidate.candidate_id.clone(),
                            false,
                            Default::default(),
                        ),
                        failed: true,
                    })
                }
            },
        }
    }

    fn execute_execution(
        &mut self,
        execution: &mut Execution,
        spec: &ExecutionSpec,
        starting_accumulator: &ExecutionAccumulator,
        worker_id: &str,
        dispatch_limit: Option<usize>,
        prefer_queued_when_capacity: bool,
    ) -> io::Result<Execution> {
        let strategy = SelectedStrategy::new(spec);
        let mut accumulator = starting_accumulator.clone();
        let mut iteration = accumulator.completed_iterations;
        let mut retry_used = false;
        let mut dispatches_used = 0usize;

        if spec.mode == "supervision"
            && !self
                .store
                .load_execution(&execution.execution_id)?
                .events
                .iter()
                .any(|event| event.event_type == ControlEventType::SupervisorAssigned)
        {
            self.append_event(
                &execution.execution_id,
                ControlEventType::SupervisorAssigned,
            )?;
        }

        while iteration < spec.policy.budget.max_iterations.unwrap_or(0) {
            match self.check_execution_control(&execution.execution_id, worker_id)? {
                ExecutionControl::Continue => {}
                ExecutionControl::Paused => {
                    execution.status = ExecutionStatus::Paused;
                    self.store.save_execution(execution)?;
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "execution paused",
                    ));
                }
                ExecutionControl::Canceled => {
                    execution.status = ExecutionStatus::Canceled;
                    self.store.save_execution(execution)?;
                    self.append_event(
                        &execution.execution_id,
                        ControlEventType::ExecutionCanceled,
                    )?;
                    return Ok(execution.clone());
                }
            }
            let candidates =
                self.load_or_plan_iteration_candidates(execution, spec, &accumulator, iteration)?;
            let persisted_candidates = self.store.load_candidates(&execution.execution_id)?;
            let mut candidate_entries = Vec::with_capacity(candidates.len());
            for candidate in &candidates {
                let candidate_record = persisted_candidates
                    .iter()
                    .find(|saved| {
                        saved.iteration == iteration && saved.candidate_id == candidate.candidate_id
                    })
                    .cloned()
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::NotFound,
                            format!("missing persisted candidate '{}'", candidate.candidate_id),
                        )
                    })?;
                candidate_entries.push((candidate, candidate_record));
            }
            if dispatch_limit == Some(1) && prefer_queued_when_capacity {
                let running_count = candidate_entries
                    .iter()
                    .filter(|(_, candidate_record)| {
                        candidate_record.status == CandidateStatus::Running
                    })
                    .count();
                let has_capacity =
                    running_count < spec.policy.concurrency.max_concurrent_candidates as usize;
                candidate_entries.sort_by_key(|(_, candidate_record)| {
                    let priority = match candidate_record.status {
                        CandidateStatus::Queued if has_capacity => 0u8,
                        CandidateStatus::Running if has_capacity => 1u8,
                        CandidateStatus::Running => 0u8,
                        CandidateStatus::Queued => 1u8,
                        CandidateStatus::Completed
                        | CandidateStatus::Failed
                        | CandidateStatus::Canceled => 2u8,
                    };
                    (priority, candidate_record.created_seq)
                });
            }

            for (candidate, candidate_record) in candidate_entries {
                let candidate_seq = candidate_record.created_seq;
                match self.check_execution_control(&execution.execution_id, worker_id)? {
                    ExecutionControl::Continue => {}
                    ExecutionControl::Paused => {
                        execution.status = ExecutionStatus::Paused;
                        self.store.save_execution(execution)?;
                        return Err(io::Error::new(
                            io::ErrorKind::WouldBlock,
                            "execution paused",
                        ));
                    }
                    ExecutionControl::Canceled => {
                        execution.status = ExecutionStatus::Canceled;
                        self.store.save_execution(execution)?;
                        self.append_event(
                            &execution.execution_id,
                            ControlEventType::ExecutionCanceled,
                        )?;
                        return Ok(execution.clone());
                    }
                }
                if let Some(limit) = dispatch_limit {
                    if dispatches_used >= limit {
                        self.store.save_execution(execution)?;
                        return Ok(execution.clone());
                    }
                }
                let outcome = match candidate_record.status {
                    CandidateStatus::Queued => self.dispatch_candidate(
                        execution,
                        spec,
                        worker_id,
                        candidate,
                        iteration,
                        candidate_seq,
                    )?,
                    CandidateStatus::Running => self.resume_running_candidate(
                        execution,
                        spec,
                        worker_id,
                        &candidate_record,
                    )?,
                    CandidateStatus::Completed
                    | CandidateStatus::Failed
                    | CandidateStatus::Canceled => continue,
                };
                match outcome {
                    DispatchOutcome::Output { output, failed } => {
                        let _ = (output, failed);
                        dispatches_used += 1;
                    }
                    DispatchOutcome::InFlight => {
                        dispatches_used += 1;
                    }
                    DispatchOutcome::Retryable(err) => {
                        execution.status = ExecutionStatus::Pending;
                        self.store.save_execution(execution)?;
                        return Err(err);
                    }
                    DispatchOutcome::Canceled => {
                        execution.status = ExecutionStatus::Canceled;
                        self.store.save_execution(execution)?;
                        self.append_event(
                            &execution.execution_id,
                            ControlEventType::ExecutionCanceled,
                        )?;
                        return Ok(execution.clone());
                    }
                }
            }

            let persisted_iteration_candidates: Vec<_> = self
                .store
                .load_candidates(&execution.execution_id)?
                .into_iter()
                .filter(|candidate| candidate.iteration == iteration)
                .collect();
            let has_pending_candidates = persisted_iteration_candidates.iter().any(|candidate| {
                matches!(
                    candidate.status,
                    CandidateStatus::Queued | CandidateStatus::Running
                )
            });
            if has_pending_candidates {
                self.store.save_execution(execution)?;
                return Ok(execution.clone());
            }

            let outputs: Vec<_> = persisted_iteration_candidates
                .iter()
                .map(|candidate| {
                    CandidateOutput::new(
                        candidate.candidate_id.clone(),
                        candidate.succeeded.unwrap_or(false),
                        candidate.metrics.clone(),
                    )
                })
                .collect();
            let iteration_failures = persisted_iteration_candidates
                .iter()
                .filter(|candidate| candidate.succeeded == Some(false))
                .count() as u32;
            let evaluation = strategy.evaluate(&accumulator, &outputs);
            match &evaluation {
                StrategyEvaluation::Swarm(swarm_evaluation) => {
                    self.append_event(&execution.execution_id, ControlEventType::CandidateScored)?;
                    accumulator = strategy.reduce(accumulator, &candidates, &evaluation);
                    accumulator.failure_counts.total_candidate_failures = accumulator
                        .failure_counts
                        .total_candidate_failures
                        .saturating_sub(
                            swarm_evaluation
                                .ranked_candidates
                                .iter()
                                .filter(|candidate| !candidate.pass)
                                .count() as u32,
                        )
                        + iteration_failures;
                }
                StrategyEvaluation::Supervision(supervision_evaluation) => {
                    self.persist_supervision_reviews(
                        execution,
                        iteration,
                        &persisted_iteration_candidates,
                        supervision_evaluation,
                    )?;
                    accumulator = strategy.reduce(accumulator, &candidates, &evaluation);
                    accumulator.completed_iterations += 1;
                    accumulator.failure_counts.total_candidate_failures += iteration_failures;
                    if let Some(approved) = supervision_evaluation
                        .decisions
                        .iter()
                        .find(|decision| decision.status == WorkerReviewStatus::Approved)
                    {
                        accumulator.best_candidate_id = Some(approved.candidate_id.clone());
                    }
                }
            }
            execution.completed_iterations = accumulator.completed_iterations;
            execution.failure_counts = accumulator.failure_counts.clone();
            execution.result_best_candidate_id = accumulator.best_candidate_id.clone();
            self.store
                .save_accumulator(&execution.execution_id, &accumulator)?;

            let all_failed = outputs.iter().all(|output| !output.succeeded);
            if matches!(evaluation, StrategyEvaluation::Swarm(_)) && all_failed {
                match spec.policy.iteration_failure_policy.as_str() {
                    "continue" => {
                        iteration += 1;
                        continue;
                    }
                    "retry_iteration" if !retry_used => {
                        retry_used = true;
                        self.store
                            .clear_iteration_candidates(&execution.execution_id, iteration)?;
                        accumulator.completed_iterations =
                            accumulator.completed_iterations.saturating_sub(1);
                        execution.completed_iterations = accumulator.completed_iterations;
                        continue;
                    }
                    _ => {
                        execution.status = ExecutionStatus::Failed;
                        self.store.save_execution(execution)?;
                        self.append_event(
                            &execution.execution_id,
                            ControlEventType::ExecutionFailed,
                        )?;
                        return Ok(execution.clone());
                    }
                }
            }

            if iteration_failures >= spec.policy.max_candidate_failures_per_iteration {
                execution.status = ExecutionStatus::Failed;
                self.store.save_execution(execution)?;
                self.append_event(&execution.execution_id, ControlEventType::ExecutionFailed)?;
                return Ok(execution.clone());
            }

            match &evaluation {
                StrategyEvaluation::Swarm(_) => {
                    if strategy.should_stop(&accumulator, &evaluation).is_some() {
                        execution.status = ExecutionStatus::Completed;
                        self.store.save_execution(execution)?;
                        self.append_event(
                            &execution.execution_id,
                            ControlEventType::IterationCompleted,
                        )?;
                        self.append_event(
                            &execution.execution_id,
                            ControlEventType::ExecutionCompleted,
                        )?;
                        return Ok(execution.clone());
                    }
                }
                StrategyEvaluation::Supervision(evaluation) => {
                    let has_rejected = evaluation
                        .decisions
                        .iter()
                        .any(|decision| decision.status == WorkerReviewStatus::Rejected);
                    if evaluation.final_approval_ready
                        && accumulator.supervision_final_approval == Some(true)
                    {
                        execution.status = ExecutionStatus::Completed;
                        self.store.save_execution(execution)?;
                        self.append_event(
                            &execution.execution_id,
                            ControlEventType::IterationCompleted,
                        )?;
                        self.append_event(
                            &execution.execution_id,
                            ControlEventType::ExecutionFinalized,
                        )?;
                        self.append_event(
                            &execution.execution_id,
                            ControlEventType::ExecutionCompleted,
                        )?;
                        return Ok(execution.clone());
                    }
                    if has_rejected {
                        execution.status = ExecutionStatus::Failed;
                        self.store.save_execution(execution)?;
                        self.append_event(
                            &execution.execution_id,
                            ControlEventType::ExecutionFailed,
                        )?;
                        return Ok(execution.clone());
                    }
                }
            }

            self.append_event(
                &execution.execution_id,
                ControlEventType::IterationCompleted,
            )?;
            iteration += 1;
        }

        execution.status = if execution.result_best_candidate_id.is_some() {
            ExecutionStatus::Completed
        } else {
            ExecutionStatus::Failed
        };
        self.store.save_execution(execution)?;
        self.append_event(
            &execution.execution_id,
            if execution.status == ExecutionStatus::Completed {
                ControlEventType::ExecutionCompleted
            } else {
                ControlEventType::ExecutionFailed
            },
        )?;
        Ok(execution.clone())
    }
}

#[cfg(feature = "serde")]
fn claim_refresh_interval_ms() -> u64 {
    std::env::var("VOID_CONTROL_CLAIM_REFRESH_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(5_000)
}

impl ExecutionService<crate::runtime::MockRuntime> {
    pub fn update_execution_status(
        store: &FsExecutionStore,
        execution_id: &str,
        action: ExecutionAction,
    ) -> io::Result<Execution> {
        let mut snapshot = store.load_execution(execution_id)?;
        let next_status = match (action, &snapshot.execution.status) {
            (ExecutionAction::Pause, ExecutionStatus::Running) => ExecutionStatus::Paused,
            (ExecutionAction::Resume, ExecutionStatus::Paused) => ExecutionStatus::Running,
            (ExecutionAction::Cancel, ExecutionStatus::Running | ExecutionStatus::Paused) => {
                ExecutionStatus::Canceled
            }
            (ExecutionAction::Cancel, ExecutionStatus::Pending) => ExecutionStatus::Canceled,
            (ExecutionAction::Pause, _) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "pause is only valid for running executions",
                ))
            }
            (ExecutionAction::Resume, _) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "resume is only valid for paused executions",
                ))
            }
            (ExecutionAction::Cancel, _) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "cancel is only valid for pending, running, or paused executions",
                ))
            }
        };
        snapshot.execution.status = next_status;
        store.save_execution(&snapshot.execution)?;
        let event_type = match action {
            ExecutionAction::Pause => ControlEventType::ExecutionPaused,
            ExecutionAction::Resume => ControlEventType::ExecutionResumed,
            ExecutionAction::Cancel => ControlEventType::ExecutionCanceled,
        };
        store.append_event(
            execution_id,
            &ControlEventEnvelope::new(execution_id, snapshot.events.len() as u64 + 1, event_type),
        )?;
        Ok(snapshot.execution)
    }

    #[cfg(feature = "serde")]
    pub fn patch_execution_policy(
        store: &FsExecutionStore,
        execution_id: &str,
        patch: PolicyPatch,
        global: &GlobalConfig,
    ) -> io::Result<ExecutionSpec> {
        let snapshot = store.load_execution(execution_id)?;
        if !matches!(
            snapshot.execution.status,
            ExecutionStatus::Pending | ExecutionStatus::Running | ExecutionStatus::Paused
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "policy updates are only valid for pending, running, or paused executions",
            ));
        }
        let mut spec = store.load_spec(execution_id)?;
        if let Some(max_iterations) = patch.max_iterations {
            spec.policy.budget.max_iterations = Some(max_iterations);
        }
        if let Some(max_concurrent_candidates) = patch.max_concurrent_candidates {
            spec.policy.concurrency.max_concurrent_candidates = max_concurrent_candidates;
        }
        spec.validate(global)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err.to_string()))?;
        store.save_spec(execution_id, &spec)?;
        Ok(spec)
    }
}

fn default_runtime_policy() -> ExecutionPolicy {
    ExecutionPolicy {
        max_parallel_microvms_per_run: 1,
        max_stage_retries: 1,
        stage_timeout_secs: 60,
        cancel_grace_period_secs: 5,
    }
}

fn scoring_from_spec(spec: &ExecutionSpec) -> ScoringConfig {
    ScoringConfig {
        metrics: spec
            .evaluation
            .weights
            .iter()
            .map(|(name, weight)| WeightedMetric {
                name: name.clone(),
                weight: weight.abs(),
                direction: if *weight < 0.0 {
                    MetricDirection::Minimize
                } else {
                    MetricDirection::Maximize
                },
            })
            .collect(),
        pass_threshold: spec.evaluation.pass_threshold.unwrap_or(0.0),
        tie_break_metric: spec.evaluation.tie_breaking.clone(),
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use crate::runtime::MockRuntime;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[test]
    fn claimed_execution_refresh_prevents_other_worker_from_stealing_stale_claim() {
        let execution_id = "exec-claim-refresh";
        let root = temp_store_dir("claim-refresh");
        let claim_path = root.join(execution_id).join("claim.txt");
        let store = FsExecutionStore::new(root);
        store
            .create_execution(&Execution::new(execution_id, "swarm", "goal"))
            .expect("create execution");

        std::env::set_var("VOID_CONTROL_CLAIM_TTL_MS", "40");
        std::env::set_var("VOID_CONTROL_CLAIM_REFRESH_MS", "5");

        let worker_store = store.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut service = ExecutionService::new(
                GlobalConfig {
                    max_concurrent_child_runs: 1,
                },
                MockRuntime::new(),
                worker_store,
            );
            service
                .with_claimed_execution(execution_id, |_service, _worker_id| {
                    started_tx.send(()).expect("signal start");
                    release_rx
                        .recv_timeout(Duration::from_millis(500))
                        .expect("wait for release signal");
                    Ok(())
                })
                .expect("claimed operation should succeed");
        });

        started_rx.recv().expect("wait for claim");
        let initial_claim = fs::read_to_string(&claim_path).expect("read initial claim");
        wait_for_claim_refresh(&claim_path, &initial_claim);

        let stolen = store
            .claim_execution(execution_id, "other-worker")
            .expect("claim should be denied while refreshed");
        release_tx.send(()).expect("release worker");

        assert!(!stolen, "other worker should not steal a refreshed claim");

        worker.join().expect("worker thread should finish");
        std::env::remove_var("VOID_CONTROL_CLAIM_TTL_MS");
        std::env::remove_var("VOID_CONTROL_CLAIM_REFRESH_MS");
    }

    fn temp_store_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("void-control-service-{label}-{nanos}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn wait_for_claim_refresh(claim_path: &std::path::Path, initial_claim: &str) {
        let started = std::time::Instant::now();
        loop {
            let current = fs::read_to_string(claim_path).expect("read refreshed claim");
            if current != initial_claim {
                return;
            }
            assert!(
                started.elapsed() < Duration::from_millis(200),
                "claim refresh did not happen before timeout"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
