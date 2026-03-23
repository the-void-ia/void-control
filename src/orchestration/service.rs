use std::io;

use crate::contract::{ContractError, ExecutionPolicy, RuntimeInspection, StartRequest, StartResult};

use super::events::{ControlEventEnvelope, ControlEventType};
#[cfg(feature = "serde")]
use super::message_box;
use super::policy::GlobalConfig;
use super::scoring::{MetricDirection, ScoringConfig, WeightedMetric};
use super::spec::ExecutionSpec;
use super::store::FsExecutionStore;
use super::strategy::{IterationEvaluation, SearchStrategy, StopReason, SwarmStrategy};
use super::types::{
    CandidateOutput, CandidateSpec, CandidateStatus, Execution, ExecutionAccumulator,
    ExecutionCandidate, ExecutionStatus, MessageStats,
};

#[cfg(feature = "serde")]
use crate::runtime::{LaunchInjectionAdapter, ProviderLaunchAdapter};
#[cfg(feature = "serde")]
use serde::Serialize;

pub trait ExecutionRuntime {
    fn start_run(&mut self, request: StartRequest) -> Result<StartResult, ContractError>;
    fn inspect_run(&self, handle: &str) -> Result<RuntimeInspection, ContractError>;
    fn take_structured_output(&mut self, run_id: &str) -> StructuredOutputResult;
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
    next_execution_id: u64,
    next_candidate_id: u64,
}

enum ExecutionControl {
    Continue,
    Paused,
    Canceled,
}

enum DispatchOutcome {
    Output {
        output: CandidateOutput,
        failed: bool,
    },
    Paused(io::Error),
    Retryable(io::Error),
    Canceled,
}

enum SelectedStrategy {
    Swarm(SwarmStrategy),
    Search(SearchStrategy),
}

impl SelectedStrategy {
    fn new(spec: &ExecutionSpec) -> Self {
        let scoring = scoring_from_spec(spec);
        match spec.mode.as_str() {
            "search" => Self::Search(SearchStrategy::new(
                spec.variation.clone(),
                scoring,
                spec.policy.convergence.clone(),
            )),
            _ => Self::Swarm(SwarmStrategy::new(
                spec.variation.clone(),
                scoring,
                spec.policy.convergence.clone(),
            )),
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
            Self::Search(strategy) => strategy.plan_candidates(accumulator, inboxes, message_stats),
        }
    }

    fn evaluate(
        &self,
        accumulator: &ExecutionAccumulator,
        outputs: &[CandidateOutput],
    ) -> IterationEvaluation {
        match self {
            Self::Swarm(strategy) => strategy.evaluate(accumulator, outputs),
            Self::Search(strategy) => strategy.evaluate(accumulator, outputs),
        }
    }

    fn reduce(
        &self,
        accumulator: ExecutionAccumulator,
        planned_candidates: &[CandidateSpec],
        evaluation: IterationEvaluation,
    ) -> ExecutionAccumulator {
        match self {
            Self::Swarm(strategy) => strategy.reduce(accumulator, evaluation),
            Self::Search(strategy) => strategy.reduce(accumulator, planned_candidates, evaluation),
        }
    }

    fn should_stop(
        &self,
        accumulator: &ExecutionAccumulator,
        evaluation: &IterationEvaluation,
    ) -> Option<StopReason> {
        match self {
            Self::Swarm(strategy) => strategy.should_stop(accumulator, evaluation),
            Self::Search(strategy) => strategy.should_stop(accumulator, evaluation),
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

        let result = operation(self, &worker_id);
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
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                invalid_message,
            ));
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
    ) -> io::Result<Option<crate::contract::RuntimeInspection>> {
        const MAX_POLLS: usize = 40;
        const POLL_SLEEP_MS: u64 = 100;

        let mut last = None;
        for _ in 0..MAX_POLLS {
            match self.check_execution_control(execution_id, worker_id)? {
                ExecutionControl::Continue => {}
                ExecutionControl::Paused => {
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "execution paused"));
                }
                ExecutionControl::Canceled => return Ok(None),
            }
            let inspection = self
                .runtime
                .inspect_run(handle)
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err.message.clone()))?;
            if inspection.state.is_terminal() {
                return Ok(Some(inspection));
            }
            last = Some(inspection);
            std::thread::sleep(std::time::Duration::from_millis(POLL_SLEEP_MS));
        }

        let message = match last {
            Some(inspection) => format!(
                "run '{}' did not reach terminal state, last state was {:?}",
                inspection.run_id, inspection.state
            ),
            None => "run did not reach terminal state".to_string(),
        };
        Err(io::Error::new(io::ErrorKind::WouldBlock, message))
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
        self.append_event(&execution.execution_id, ControlEventType::ExecutionSubmitted)?;
        execution.status = ExecutionStatus::Running;
        self.store.save_execution(&execution)?;
        self.append_event(&execution.execution_id, ControlEventType::ExecutionStarted)?;
        self.execute_execution(
            &mut execution,
            &spec,
            &ExecutionAccumulator::default(),
            &worker_id,
            None,
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
            service.dispatch_execution_once_claimed(execution_id, worker_id)
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
        self.execute_execution(&mut execution, &spec, &accumulator, worker_id, None)
    }

    #[cfg(feature = "serde")]
    fn dispatch_execution_once_claimed(
        &mut self,
        execution_id: &str,
        worker_id: &str,
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
        self.execute_execution(&mut execution, &spec, &accumulator, worker_id, Some(1))
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
        self.store
            .append_event(execution_id, &ControlEventEnvelope::new(execution_id, seq, event_type))
    }

    fn save_candidate_state(
        &self,
        execution_id: &str,
        candidate_id: &str,
        created_seq: u64,
        iteration: u32,
        status: CandidateStatus,
        runtime_run_id: Option<String>,
        overrides: &std::collections::BTreeMap<String, String>,
        succeeded: Option<bool>,
        metrics: &std::collections::BTreeMap<String, f64>,
    ) -> io::Result<()> {
        let mut record =
            ExecutionCandidate::new(execution_id, candidate_id, created_seq, iteration, status);
        record.runtime_run_id = runtime_run_id;
        record.overrides = overrides.clone();
        record.succeeded = succeeded;
        record.metrics = metrics.clone();
        self.store.save_candidate(&record)
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
        output: &CandidateOutput,
    ) -> io::Result<()> {
        let (valid, rejected) =
            message_box::normalize_intents(candidate_id, iteration, &output.intents);
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
            self.save_candidate_state(
                &execution.execution_id,
                &candidate.candidate_id,
                candidate_seq,
                iteration,
                CandidateStatus::Queued,
                None,
                &candidate.overrides,
                None,
                &Default::default(),
            )?;
            self.append_event(&execution.execution_id, ControlEventType::CandidateQueued)?;
            self.next_candidate_id += 1;
        }
        Ok(candidates)
    }

    #[cfg(feature = "serde")]
    fn load_message_stats(
        &self,
        execution_id: &str,
        iteration: u32,
    ) -> io::Result<MessageStats> {
        let intents = self.store.load_intents(execution_id)?;
        let messages = self.store.load_routed_messages(execution_id)?;
        Ok(message_box::extract_message_stats(&intents, &messages, iteration))
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
        self.append_event(&execution.execution_id, ControlEventType::CandidateDispatched)?;
        #[cfg(feature = "serde")]
        let launch_inbox = self.load_launch_inbox_snapshot(
            &execution.execution_id,
            iteration,
            &candidate.candidate_id,
        )?;
        #[cfg(feature = "serde")]
        let launch_request = self.launch_adapter.prepare_launch_request(
            StartRequest {
                run_id: run_id.clone(),
                workflow_spec: spec.workflow.template.clone(),
                launch_context: None,
                policy: default_runtime_policy(),
            },
            candidate,
            &launch_inbox,
        );
        #[cfg(feature = "serde")]
        let started = self
            .runtime
            .start_run(launch_request)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err.message))?;
        #[cfg(not(feature = "serde"))]
        let started = self
            .runtime
            .start_run(StartRequest {
                run_id: run_id.clone(),
                workflow_spec: spec.workflow.template.clone(),
                launch_context: None,
                policy: default_runtime_policy(),
            })
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err.message))?;
        self.save_candidate_state(
            &execution.execution_id,
            &candidate.candidate_id,
            created_seq,
            iteration,
            CandidateStatus::Running,
            Some(run_id.clone()),
            &candidate.overrides,
            None,
            &Default::default(),
        )?;

        let inspection = match self.wait_for_terminal_run(&execution.execution_id, worker_id, &started.handle)
        {
            Ok(Some(inspection)) => inspection,
            Ok(None) => {
                self.save_candidate_state(
                    &execution.execution_id,
                    &candidate.candidate_id,
                    created_seq,
                    iteration,
                    CandidateStatus::Canceled,
                    Some(run_id),
                    &candidate.overrides,
                    None,
                    &Default::default(),
                )?;
                return Ok(DispatchOutcome::Canceled);
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                return Ok(DispatchOutcome::Paused(err));
            }
            Err(err) => return Err(err),
        };

        if inspection.state == crate::contract::RunState::Failed {
            self.save_candidate_state(
                &execution.execution_id,
                &candidate.candidate_id,
                created_seq,
                iteration,
                CandidateStatus::Failed,
                Some(inspection.run_id.clone()),
                &candidate.overrides,
                Some(false),
                &Default::default(),
            )?;
            self.append_event(
                &execution.execution_id,
                ControlEventType::CandidateOutputCollected,
            )?;
            return Ok(DispatchOutcome::Output {
                output: CandidateOutput::new(
                    candidate.candidate_id.clone(),
                    false,
                    Default::default(),
                ),
                failed: true,
            });
        }

        match self.runtime.take_structured_output(&inspection.run_id) {
            StructuredOutputResult::Found(mut output) => {
                self.save_candidate_state(
                    &execution.execution_id,
                    &candidate.candidate_id,
                    created_seq,
                    iteration,
                    CandidateStatus::Completed,
                    Some(inspection.run_id.clone()),
                    &candidate.overrides,
                    Some(output.succeeded),
                    &output.metrics,
                )?;
                output.candidate_id = candidate.candidate_id.clone();
                self.append_event(
                    &execution.execution_id,
                    ControlEventType::CandidateOutputCollected,
                )?;
                #[cfg(feature = "serde")]
                self.persist_candidate_intents(
                    &execution.execution_id,
                    iteration,
                    &candidate.candidate_id,
                    &output,
                )?;
                Ok(DispatchOutcome::Output {
                    output,
                    failed: false,
                })
            }
            StructuredOutputResult::Missing => {
                let failed = spec.policy.missing_output_policy == "mark_failed";
                self.save_candidate_state(
                    &execution.execution_id,
                    &candidate.candidate_id,
                    created_seq,
                    iteration,
                    if failed {
                        CandidateStatus::Failed
                    } else {
                        CandidateStatus::Completed
                    },
                    Some(inspection.run_id.clone()),
                    &candidate.overrides,
                    Some(!failed),
                    &Default::default(),
                )?;
                self.append_event(
                    &execution.execution_id,
                    ControlEventType::CandidateOutputCollected,
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
                    let failed = spec.policy.missing_output_policy == "mark_failed";
                    self.save_candidate_state(
                        &execution.execution_id,
                        &candidate.candidate_id,
                        created_seq,
                        iteration,
                        if failed {
                            CandidateStatus::Failed
                        } else {
                            CandidateStatus::Completed
                        },
                        Some(inspection.run_id.clone()),
                        &candidate.overrides,
                        Some(!failed),
                        &Default::default(),
                    )?;
                    self.append_event(
                        &execution.execution_id,
                        ControlEventType::CandidateOutputCollected,
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
                    self.save_candidate_state(
                        &execution.execution_id,
                        &candidate.candidate_id,
                        created_seq,
                        iteration,
                        CandidateStatus::Failed,
                        Some(inspection.run_id.clone()),
                        &candidate.overrides,
                        Some(false),
                        &Default::default(),
                    )?;
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
    ) -> io::Result<Execution> {
        let strategy = SelectedStrategy::new(spec);
        let mut accumulator = starting_accumulator.clone();
        let mut iteration = accumulator.completed_iterations;
        let mut retry_used = false;
        let mut dispatches_used = 0usize;

        while iteration < spec.policy.budget.max_iterations.unwrap_or(0) {
            match self.check_execution_control(&execution.execution_id, worker_id)? {
                ExecutionControl::Continue => {}
                ExecutionControl::Paused => {
                    execution.status = ExecutionStatus::Paused;
                    self.store.save_execution(execution)?;
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "execution paused"));
                }
                ExecutionControl::Canceled => {
                    execution.status = ExecutionStatus::Canceled;
                    self.store.save_execution(execution)?;
                    self.append_event(&execution.execution_id, ControlEventType::ExecutionCanceled)?;
                    return Ok(execution.clone());
                }
            }
            let candidates =
                self.load_or_plan_iteration_candidates(execution, spec, &accumulator, iteration)?;

            for candidate in &candidates {
                let candidate_record = self
                    .store
                    .load_candidates(&execution.execution_id)?
                    .into_iter()
                    .find(|saved| {
                        saved.iteration == iteration && saved.candidate_id == candidate.candidate_id
                    })
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::NotFound,
                            format!("missing persisted candidate '{}'", candidate.candidate_id),
                        )
                    })?;
                if candidate_record.status != CandidateStatus::Queued {
                    continue;
                }
                let candidate_seq = candidate_record.created_seq;
                match self.check_execution_control(&execution.execution_id, worker_id)? {
                    ExecutionControl::Continue => {}
                    ExecutionControl::Paused => {
                        execution.status = ExecutionStatus::Paused;
                        self.store.save_execution(execution)?;
                        return Err(io::Error::new(io::ErrorKind::WouldBlock, "execution paused"));
                    }
                    ExecutionControl::Canceled => {
                        execution.status = ExecutionStatus::Canceled;
                        self.store.save_execution(execution)?;
                        self.append_event(&execution.execution_id, ControlEventType::ExecutionCanceled)?;
                        return Ok(execution.clone());
                    }
                }
                if let Some(limit) = dispatch_limit {
                    if dispatches_used >= limit {
                        self.store.save_execution(execution)?;
                        return Ok(execution.clone());
                    }
                }
                match self.dispatch_candidate(
                    execution,
                    spec,
                    worker_id,
                    &candidate,
                    iteration,
                    candidate_seq,
                )? {
                    DispatchOutcome::Output { output, failed } => {
                        let _ = (output, failed);
                        dispatches_used += 1;
                    }
                    DispatchOutcome::Paused(err) => {
                        execution.status = ExecutionStatus::Paused;
                        self.store.save_execution(execution)?;
                        return Err(err);
                    }
                    DispatchOutcome::Retryable(err) => {
                        execution.status = ExecutionStatus::Pending;
                        self.store.save_execution(execution)?;
                        return Err(err);
                    }
                    DispatchOutcome::Canceled => {
                        execution.status = ExecutionStatus::Canceled;
                        self.store.save_execution(execution)?;
                        self.append_event(&execution.execution_id, ControlEventType::ExecutionCanceled)?;
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
                matches!(candidate.status, CandidateStatus::Queued | CandidateStatus::Running)
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
            self.append_event(&execution.execution_id, ControlEventType::CandidateScored)?;
            accumulator = strategy.reduce(accumulator, &candidates, evaluation.clone());
            accumulator.failure_counts.total_candidate_failures = accumulator
                .failure_counts
                .total_candidate_failures
                .saturating_sub(
                    evaluation
                        .ranked_candidates
                        .iter()
                        .filter(|candidate| !candidate.pass)
                        .count() as u32,
                )
                + iteration_failures;
            execution.completed_iterations = accumulator.completed_iterations;
            execution.failure_counts = accumulator.failure_counts.clone();
            execution.result_best_candidate_id = accumulator.best_candidate_id.clone();
            self.store.save_accumulator(&execution.execution_id, &accumulator)?;

            let all_failed = outputs.iter().all(|output| !output.succeeded);
            if all_failed {
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
                        self.append_event(&execution.execution_id, ControlEventType::ExecutionFailed)?;
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

            if strategy.should_stop(&accumulator, &evaluation).is_some() {
                execution.status = ExecutionStatus::Completed;
                self.store.save_execution(execution)?;
                self.append_event(&execution.execution_id, ControlEventType::IterationCompleted)?;
                self.append_event(&execution.execution_id, ControlEventType::ExecutionCompleted)?;
                return Ok(execution.clone());
            }

            self.append_event(&execution.execution_id, ControlEventType::IterationCompleted)?;
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
            &ControlEventEnvelope::new(
                execution_id,
                snapshot.events.len() as u64 + 1,
                event_type,
            ),
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
