# Iteration Orchestration Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the v0.2 iteration control plane so `void-control` can create and manage multi-iteration `Execution`s above the existing run-level `void-box` client.

**Architecture:** Add a new orchestration layer under `src/orchestration/` that is explicitly separate from the existing `src/contract/` and `src/runtime/` run-level boundary. The orchestration layer owns `ExecutionSpec` validation, durable execution state, event reduction, scheduling, candidate dispatch, artifact collection, scoring, checkpointing, and policy updates, while continuing to consume `VoidBoxRuntimeClient` only through the current runtime contract.

**Tech Stack:** Rust 2021, existing `serde` feature gates, current TCP/HTTP runtime client, filesystem-backed persistence for the first cut, and Cargo test integration.

---

## Scope Check

The spec spans multiple subsystems:

- orchestration domain model and persistence
- control-loop execution engine and scheduler
- strategy/evaluation/variation logic
- API and bridge exposure
- observability, pause/resume, reconciliation, and dry-run

This is large enough that it could be split into separate plans. To keep momentum and preserve cross-cutting invariants, this document keeps one plan but breaks it into independently shippable chunks. Each chunk should end in passing tests and a focused commit.

## File Map

### Existing files to keep as-is conceptually

- Modify: `src/lib.rs`
- Modify: `src/bin/voidctl.rs`
- Modify: `src/bridge.rs`
- Modify: `src/runtime/mock.rs`
- Modify: `src/runtime/void_box.rs`
- Modify: `Cargo.toml`
- Modify: `README.md`
- Modify: `tests/void_box_contract.rs`

### New orchestration module tree

- Create: `src/orchestration/mod.rs`
  Responsibility: public exports for execution orchestration.
- Create: `src/orchestration/types.rs`
  Responsibility: `Execution`, `Iteration`, `Candidate`, `ExecutionResult`, `ExecutionStatus`, accumulator, artifact refs.
- Create: `src/orchestration/spec.rs`
  Responsibility: `ExecutionSpec`, mode-specific config, validation, dry-run input parsing.
- Create: `src/orchestration/policy.rs`
  Responsibility: orchestration policy model from spec section 14 and mutable/immutable policy checks.
- Create: `src/orchestration/events.rs`
  Responsibility: control-plane event envelope, event types, payload structs, event reduction helpers.
- Create: `src/orchestration/store.rs`
  Responsibility: persistence traits for executions, events, queues, accumulator, and reconciliation snapshots.
- Create: `src/orchestration/store/fs.rs`
  Responsibility: filesystem-backed store for the first implementation.
- Create: `src/orchestration/strategy.rs`
  Responsibility: `IterationStrategy` trait and registry.
- Create: `src/orchestration/strategy/swarm.rs`
  Responsibility: `SwarmStrategy`, inbox materialization, candidate planning hooks.
- Create: `src/orchestration/scoring.rs`
  Responsibility: deterministic scoring, ranking, tie-breaking.
- Create: `src/orchestration/variation.rs`
  Responsibility: parameter-space, explicit, and leader-directed candidate override generation.
- Create: `src/orchestration/artifacts.rs`
  Responsibility: artifact retrieval, parsing, and mode-aware validation.
- Create: `src/orchestration/scheduler.rs`
  Responsibility: global pool, per-execution limits, queue ordering, dispatch admission.
- Create: `src/orchestration/loop.rs`
  Responsibility: shared control loop, dispatch/collect/evaluate/reduce/stop sequencing.
- Create: `src/orchestration/reconcile.rs`
  Responsibility: restart recovery, replay, orphaned handle handling.
- Create: `src/orchestration/service.rs`
  Responsibility: high-level service API for create/list/inspect/pause/resume/cancel/patch-policy/dry-run.
- Create: `src/orchestration/http.rs`
  Responsibility: HTTP request/response models for execution endpoints if bridge continues using `tiny_http`.

### New tests

- Create: `tests/execution_spec_validation.rs`
- Create: `tests/execution_dry_run.rs`
- Create: `tests/execution_event_replay.rs`
- Create: `tests/execution_scheduler.rs`
- Create: `tests/execution_pause_resume.rs`
- Create: `tests/execution_policy_patch.rs`
- Create: `tests/execution_swarm_strategy.rs`
- Create: `tests/execution_artifact_collection.rs`
- Create: `tests/execution_reconciliation.rs`

### Optional later UI follow-up

- Defer from this plan unless explicitly requested:
  `web/void-control-ux/*`

The spec includes UI visibility, but the backend orchestration contract should land first so the UI can bind to stable execution endpoints rather than internal scaffolding.

## Delivery Strategy

Implement in this order:

1. domain model, persistence shape, and validation
2. pure strategy/evaluation logic
3. scheduler and control loop using `MockRuntime`
4. artifact retrieval and reconciliation
5. HTTP/API exposure
6. operational features: pause/resume, policy patch, dry-run, observability

That order keeps early milestones testable without requiring a full live daemon.

## Chunk 1: Domain Model, Validation, and Persistence Skeleton

### Task 1: Add orchestration module exports

**Files:**
- Modify: `src/lib.rs`
- Create: `src/orchestration/mod.rs`

- [ ] **Step 1: Write the failing compile test by referencing the future module**

Add a smoke test in `src/lib.rs` or `tests/execution_spec_validation.rs` that imports:

```rust
use void_control::orchestration::ExecutionSpec;
```

Expected: compile failure because `orchestration` does not exist.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test execution_spec_validation -- --nocapture`
Expected: compile error mentioning missing `orchestration`.

- [ ] **Step 3: Add minimal module exports**

Create:

```rust
// src/orchestration/mod.rs
pub mod spec;

pub use spec::ExecutionSpec;
```

Modify `src/lib.rs`:

```rust
pub mod orchestration;
```

- [ ] **Step 4: Run test to verify the module resolves**

Run: `cargo test execution_spec_validation -- --nocapture`
Expected: next failure moves to missing `ExecutionSpec` details.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/orchestration/mod.rs tests/execution_spec_validation.rs
git commit -m "orchestration: add module scaffold"
```

### Task 2: Implement `ExecutionSpec` and policy validation

**Files:**
- Create: `src/orchestration/spec.rs`
- Create: `src/orchestration/policy.rs`
- Test: `tests/execution_spec_validation.rs`

- [ ] **Step 1: Write failing validation tests from spec section 14 and 25**

Cover at least:

```rust
#[test]
fn rejects_unbounded_execution() {}

#[test]
fn rejects_concurrency_above_global_pool() {}

#[test]
fn rejects_threshold_without_min_score() {}

#[test]
fn accepts_exhaustive_with_max_iterations() {}

#[test]
fn rejects_unknown_mode() {}
```

- [ ] **Step 2: Run targeted tests**

Run: `cargo test --test execution_spec_validation -- --nocapture`
Expected: failures for missing types and `validate()`.

- [ ] **Step 3: Implement minimal validation**

Define:

```rust
pub struct ExecutionSpec {
    pub mode: String,
    pub goal: String,
    pub workflow: WorkflowTemplateRef,
    pub policy: OrchestrationPolicy,
    pub evaluation: EvaluationConfig,
    pub variation: VariationConfig,
    pub swarm: Option<SwarmModeConfig>,
}

impl ExecutionSpec {
    pub fn validate(&self, global: &GlobalConfig) -> Result<(), SpecValidationError> {
        // enforce section 14.6 and section 25.3
    }
}
```

Implement only rules already stated in the spec. Do not infer extra policy semantics.

- [ ] **Step 4: Re-run tests**

Run: `cargo test --test execution_spec_validation -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/orchestration/spec.rs src/orchestration/policy.rs tests/execution_spec_validation.rs
git commit -m "orchestration: validate execution specs"
```

### Task 3: Add execution state and control-plane events

**Files:**
- Create: `src/orchestration/types.rs`
- Create: `src/orchestration/events.rs`
- Test: `tests/execution_event_replay.rs`

- [ ] **Step 1: Write failing tests for event-sourced state reduction**

Cover:

```rust
#[test]
fn execution_state_advances_from_control_plane_events() {}

#[test]
fn warning_events_do_not_advance_execution_state() {}

#[test]
fn accumulator_is_reconstructible_from_event_log() {}
```

- [ ] **Step 2: Run tests to verify failures**

Run: `cargo test --test execution_event_replay -- --nocapture`
Expected: missing reducer/types.

- [ ] **Step 3: Implement event and reducer types**

Include:

- `ExecutionCreated`
- `IterationPlanned`
- `IterationStarted`
- `CandidateScheduled`
- `CandidateQueued`
- `CandidateDispatched`
- `CandidateCompleted`
- `CandidateScored`
- `IterationCompleted`
- `ExecutionCompleted`
- `ExecutionFailed`
- `ExecutionCanceled`
- operational side-channel events from section 22

Keep warning events persisted but excluded from state transitions.

- [ ] **Step 4: Re-run tests**

Run: `cargo test --test execution_event_replay -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/orchestration/types.rs src/orchestration/events.rs tests/execution_event_replay.rs
git commit -m "orchestration: add execution state and events"
```

### Task 4: Add a filesystem-backed orchestration store

**Files:**
- Create: `src/orchestration/store.rs`
- Create: `src/orchestration/store/fs.rs`
- Test: `tests/execution_event_replay.rs`

- [ ] **Step 1: Write failing persistence tests**

Cover:

```rust
#[test]
fn store_round_trips_execution_and_events() {}

#[test]
fn store_can_reload_accumulator_after_restart() {}
```

- [ ] **Step 2: Run targeted tests**

Run: `cargo test --test execution_event_replay store_ -- --nocapture`
Expected: missing store trait/implementation.

- [ ] **Step 3: Implement a narrow persistence interface**

Start with:

```rust
pub trait ExecutionStore {
    fn create_execution(&self, execution: &Execution) -> Result<()>;
    fn append_event(&self, event: &ControlEventEnvelope) -> Result<()>;
    fn load_execution(&self, execution_id: &str) -> Result<ExecutionSnapshot>;
    fn list_active_executions(&self) -> Result<Vec<Execution>>;
    fn save_accumulator(&self, execution_id: &str, acc: &ExecutionAccumulator) -> Result<()>;
}
```

Use one directory per execution under a configured root such as `target/tmp/executions/`.

- [ ] **Step 4: Re-run tests**

Run: `cargo test --test execution_event_replay -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/orchestration/store.rs src/orchestration/store/fs.rs tests/execution_event_replay.rs
git commit -m "orchestration: add filesystem execution store"
```

## Chunk 2: Pure Strategy, Evaluation, and Variation

### Task 5: Implement deterministic scoring and ranking

**Files:**
- Create: `src/orchestration/scoring.rs`
- Test: `tests/execution_swarm_strategy.rs`

- [ ] **Step 1: Write failing tests for weighted metrics and tie-breaking**

Cover:

```rust
#[test]
fn weighted_metrics_normalizes_within_iteration() {}

#[test]
fn failed_candidate_scores_zero() {}

#[test]
fn best_result_uses_tie_breaking_after_score() {}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test execution_swarm_strategy scoring -- --nocapture`
Expected: failures for missing scorer.

- [ ] **Step 3: Implement scoring exactly per section 15**

Provide a scorer API:

```rust
pub trait ScoringFunction {
    fn score_iteration(&self, outputs: &[CandidateOutput]) -> Vec<ScoringResult>;
}
```

Do not compare normalized scores across iterations for `best_result`; use the raw metric comparison rule from section 15.5 plus section 19.3.

- [ ] **Step 4: Re-run tests**

Run: `cargo test --test execution_swarm_strategy scoring -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/orchestration/scoring.rs tests/execution_swarm_strategy.rs
git commit -m "orchestration: add deterministic scoring"
```

### Task 6: Implement candidate variation generators

**Files:**
- Create: `src/orchestration/variation.rs`
- Test: `tests/execution_swarm_strategy.rs`

- [ ] **Step 1: Write failing tests for variation sources**

Cover:

```rust
#[test]
fn parameter_space_random_respects_candidates_per_iteration() {}

#[test]
fn parameter_space_sequential_preserves_order() {}

#[test]
fn explicit_variation_cycles_through_overrides() {}

#[test]
fn leader_directed_proposals_are_validated_before_use() {}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test execution_swarm_strategy variation -- --nocapture`
Expected: failures for missing generator.

- [ ] **Step 3: Implement minimal generators**

Important rules:

- `parameter_space` supports `random` and `sequential`
- `explicit` cycles through provided sets
- `leader_directed` only consumes validated proposals from `intents.json`
- override application is shallow replacement using dot-paths

- [ ] **Step 4: Re-run tests**

Run: `cargo test --test execution_swarm_strategy variation -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/orchestration/variation.rs tests/execution_swarm_strategy.rs
git commit -m "orchestration: add candidate variation generators"
```

### Task 7: Add `IterationStrategy` and `SwarmStrategy`

**Files:**
- Create: `src/orchestration/strategy.rs`
- Create: `src/orchestration/strategy/swarm.rs`
- Test: `tests/execution_swarm_strategy.rs`

- [ ] **Step 1: Write failing tests for pure swarm behavior**

Cover:

```rust
#[test]
fn swarm_materializes_inboxes_from_message_backlog() {}

#[test]
fn swarm_plans_candidates_from_variation_source() {}

#[test]
fn swarm_should_stop_on_threshold() {}

#[test]
fn swarm_should_stop_on_plateau() {}

#[test]
fn swarm_reduce_updates_best_result_and_failure_counts() {}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test execution_swarm_strategy -- --nocapture`
Expected: failures for missing trait/strategy.

- [ ] **Step 3: Implement trait and registry**

Base trait:

```rust
pub trait IterationStrategy {
    fn materialize_inboxes(&self, accumulator: &ExecutionAccumulator) -> Vec<CandidateInbox>;
    fn plan_candidates(&self, accumulator: &ExecutionAccumulator, inboxes: &[CandidateInbox]) -> Vec<CandidateSpec>;
    fn evaluate(&self, accumulator: &ExecutionAccumulator, outputs: &[CandidateOutput]) -> IterationEvaluation;
    fn should_stop(&self, accumulator: &ExecutionAccumulator, evaluation: &IterationEvaluation) -> Option<StopReason>;
    fn reduce(&self, accumulator: ExecutionAccumulator, evaluation: IterationEvaluation) -> ExecutionAccumulator;
}
```

Register only `swarm` in the first implementation. Reject `search` and `tournament` during validation with a clear “named but not implemented” error until those modes exist.

- [ ] **Step 4: Re-run tests**

Run: `cargo test --test execution_swarm_strategy -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/orchestration/strategy.rs src/orchestration/strategy/swarm.rs tests/execution_swarm_strategy.rs
git commit -m "orchestration: add swarm iteration strategy"
```

## Chunk 3: Scheduler and Control Loop

### Task 8: Extend `MockRuntime` to support orchestrator tests

**Files:**
- Modify: `src/runtime/mock.rs`
- Test: `tests/execution_scheduler.rs`

- [ ] **Step 1: Write failing orchestrator-facing tests**

Cover:

```rust
#[test]
fn mock_runtime_can_complete_runs_with_structured_outputs() {}

#[test]
fn mock_runtime_can_simulate_failure_timeout_and_missing_output() {}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test execution_scheduler mock_runtime -- --nocapture`
Expected: existing mock is too shallow.

- [ ] **Step 3: Add deterministic test hooks**

Add helper APIs for tests only, for example:

```rust
#[cfg(test)]
impl MockRuntime {
    pub fn seed_run_outcome(&mut self, run_id: &str, outcome: SeededOutcome) { /* ... */ }
}
```

Do not complicate the production runtime contract. Keep this support behind `#[cfg(test)]` or an orchestration-test-only constructor.

- [ ] **Step 4: Re-run tests**

Run: `cargo test --test execution_scheduler mock_runtime -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/runtime/mock.rs tests/execution_scheduler.rs
git commit -m "runtime: extend mock for execution orchestration tests"
```

### Task 9: Implement the two-level scheduler

**Files:**
- Create: `src/orchestration/scheduler.rs`
- Test: `tests/execution_scheduler.rs`

- [ ] **Step 1: Write failing scheduler tests from section 21**

Cover:

```rust
#[test]
fn preserves_plan_candidates_order_within_execution() {}

#[test]
fn dispatches_across_executions_fifo_by_candidate_creation_time() {}

#[test]
fn releases_slots_immediately_on_completion() {}

#[test]
fn paused_execution_keeps_queue_but_releases_slots() {}

#[test]
fn exhausted_budget_prevents_queue_entry() {}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test execution_scheduler scheduler -- --nocapture`
Expected: failures for missing scheduler.

- [ ] **Step 3: Implement scheduler primitives**

Suggested structure:

```rust
pub struct GlobalScheduler {
    max_concurrent_child_runs: usize,
    queues: BTreeMap<String, ExecutionQueue>,
}

impl GlobalScheduler {
    pub fn enqueue(&mut self, execution_id: &str, candidate: QueuedCandidate) -> QueueDecision;
    pub fn dispatchable(&self) -> Vec<DispatchGrant>;
    pub fn release(&mut self, execution_id: &str, candidate_id: &str);
}
```

Persist enough queue metadata to recover after restart.

- [ ] **Step 4: Re-run tests**

Run: `cargo test --test execution_scheduler scheduler -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/orchestration/scheduler.rs tests/execution_scheduler.rs
git commit -m "orchestration: add execution scheduler"
```

### Task 10: Implement the shared execution control loop

**Files:**
- Create: `src/orchestration/loop.rs`
- Create: `src/orchestration/service.rs`
- Test: `tests/execution_scheduler.rs`

- [ ] **Step 1: Write failing end-to-end loop tests with `MockRuntime`**

Cover:

```rust
#[test]
fn runs_single_iteration_and_completes_with_best_result() {}

#[test]
fn runs_multiple_iterations_until_threshold() {}

#[test]
fn short_circuits_iteration_after_failure_limit() {}

#[test]
fn marks_execution_failed_when_all_candidates_fail_and_policy_says_fail() {}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test execution_scheduler -- --nocapture`
Expected: failures for missing service/loop.

- [ ] **Step 3: Implement infrastructure methods**

Required orchestration flow:

```rust
create_execution()
-> persist ExecutionCreated
-> plan iteration
-> queue candidates
-> scheduler grants dispatch slots
-> runtime.start(...)
-> collect terminal events and artifacts
-> strategy.evaluate(...)
-> strategy.should_stop(...)
-> strategy.reduce(...)
-> persist accumulator and next state
```

Keep `dispatch_candidates()` and `collect_outputs()` as shared infrastructure, not strategy methods.

- [ ] **Step 4: Re-run tests**

Run: `cargo test --test execution_scheduler -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/orchestration/loop.rs src/orchestration/service.rs tests/execution_scheduler.rs
git commit -m "orchestration: add execution control loop"
```

## Chunk 4: Artifact Retrieval, Failure Semantics, and Reconciliation

### Task 11: Add artifact retrieval and candidate completion mapping

**Files:**
- Create: `src/orchestration/artifacts.rs`
- Modify: `src/runtime/void_box.rs`
- Test: `tests/execution_artifact_collection.rs`

- [ ] **Step 1: Write failing tests for section 18 behavior**

Cover:

```rust
#[test]
fn waits_for_terminal_event_before_fetching_result() {}

#[test]
fn parses_result_json_metrics_and_artifact_refs() {}

#[test]
fn emits_output_error_for_missing_or_malformed_result() {}

#[test]
fn leader_directed_intents_are_read_from_output_contract() {}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test execution_artifact_collection -- --nocapture`
Expected: failures for missing retrieval method.

- [ ] **Step 3: Extend runtime client with artifact fetch support**

Add a narrow method on `VoidBoxRuntimeClient`:

```rust
pub fn fetch_stage_output_file(&self, run_id: &str, stage: &str) -> Result<Vec<u8>, ContractError>;
```

Map response errors into orchestrator-visible output diagnostics.

- [ ] **Step 4: Implement artifact parsing**

Represent:

- `result.json`
- embedded or staged `intents.json`
- retrieval timeout handling
- reference-only persistence, not full artifact duplication

- [ ] **Step 5: Re-run tests**

Run: `cargo test --test execution_artifact_collection -- --nocapture`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/orchestration/artifacts.rs src/runtime/void_box.rs tests/execution_artifact_collection.rs
git commit -m "orchestration: collect structured candidate outputs"
```

### Task 12: Implement failure semantics and timeout handling

**Files:**
- Modify: `src/orchestration/loop.rs`
- Test: `tests/execution_artifact_collection.rs`
- Test: `tests/execution_scheduler.rs`

- [ ] **Step 1: Write failing tests for section 17 decision paths**

Cover:

```rust
#[test]
fn missing_output_can_mark_failed() {}

#[test]
fn missing_output_can_mark_incomplete_without_failure_count() {}

#[test]
fn candidate_timeout_cancels_run() {}

#[test]
fn iteration_failure_policy_continue_advances_despite_all_failures() {}

#[test]
fn iteration_failure_policy_retry_retries_once() {}
```

- [ ] **Step 2: Run tests**

Run: `cargo test execution_ -- --nocapture`
Expected: failures in new policy paths.

- [ ] **Step 3: Implement only the spec-defined decisions**

Important limits:

- `retry_iteration` is hardcoded to one retry in v0.2
- timeout defaults from workflow template if policy field is unset
- all failures must emit explicit control-plane diagnostics

- [ ] **Step 4: Re-run tests**

Run: `cargo test execution_ -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/orchestration/loop.rs tests/execution_artifact_collection.rs tests/execution_scheduler.rs
git commit -m "orchestration: enforce execution failure semantics"
```

### Task 13: Implement restart reconciliation

**Files:**
- Create: `src/orchestration/reconcile.rs`
- Modify: `src/orchestration/service.rs`
- Test: `tests/execution_reconciliation.rs`

- [ ] **Step 1: Write failing reconciliation tests**

Cover:

```rust
#[test]
fn reloads_non_terminal_executions_after_restart() {}

#[test]
fn resumes_event_stream_from_last_seen_id() {}

#[test]
fn marks_unknown_handles_as_orphaned() {}

#[test]
fn paused_execution_remains_paused_after_restart() {}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test execution_reconciliation -- --nocapture`
Expected: failures for missing reconciliation service.

- [ ] **Step 3: Implement reconciliation using store plus runtime inspection**

Rules to enforce:

- control-plane events remain source of truth
- direct runtime inspection is for repair/re-sync only
- replay should rebuild accumulator and queue state

- [ ] **Step 4: Re-run tests**

Run: `cargo test --test execution_reconciliation -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/orchestration/reconcile.rs src/orchestration/service.rs tests/execution_reconciliation.rs
git commit -m "orchestration: add execution reconciliation"
```

## Chunk 5: API Surface and Operational Controls

### Task 14: Add dry-run service and validation endpoint

**Files:**
- Modify: `src/orchestration/service.rs`
- Create: `src/orchestration/http.rs`
- Modify: `src/bridge.rs`
- Test: `tests/execution_dry_run.rs`

- [ ] **Step 1: Write failing dry-run tests**

Cover:

```rust
#[test]
fn dry_run_validates_without_creating_execution() {}

#[test]
fn dry_run_returns_plan_warnings_and_errors() {}

#[test]
fn dry_run_reports_parameter_space_cardinality() {}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test execution_dry_run -- --nocapture`
Expected: failures for missing endpoint/service.

- [ ] **Step 3: Implement dry-run response model**

Include:

- `valid`
- `mode`
- computed plan summary
- warnings
- errors

Do not contact `void-box` during dry-run.

- [ ] **Step 4: Expose endpoint in bridge**

Add:

- `POST /v1/executions/dry-run`

Keep the existing `/v1/launch` path intact for run-level workflows.

- [ ] **Step 5: Re-run tests**

Run: `cargo test --features serde --test execution_dry_run -- --nocapture`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/orchestration/service.rs src/orchestration/http.rs src/bridge.rs tests/execution_dry_run.rs
git commit -m "bridge: add execution dry-run endpoint"
```

### Task 15: Add execution lifecycle endpoints

**Files:**
- Modify: `src/bridge.rs`
- Modify: `src/bin/voidctl.rs`
- Test: `tests/execution_pause_resume.rs`

- [ ] **Step 1: Write failing API tests**

Cover:

```rust
#[test]
fn can_create_and_inspect_execution() {}

#[test]
fn can_pause_and_resume_execution() {}

#[test]
fn can_cancel_running_or_paused_execution() {}

#[test]
fn inspect_exposes_queue_depth_and_wait_time() {}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --features serde --test execution_pause_resume -- --nocapture`
Expected: missing endpoints.

- [ ] **Step 3: Implement HTTP endpoints**

Add:

- `POST /v1/executions`
- `GET /v1/executions/{id}`
- `GET /v1/executions`
- `POST /v1/executions/{id}/pause`
- `POST /v1/executions/{id}/resume`
- `POST /v1/executions/{id}/cancel`

Do not overload the run-level endpoints with execution semantics.

- [ ] **Step 4: Add `voidctl` commands**

Add CLI support for:

- `/execution create <spec_file>`
- `/execution status <id>`
- `/execution pause <id>`
- `/execution resume <id>`
- `/execution cancel <id>`

Avoid breaking current run-oriented commands.

- [ ] **Step 5: Re-run tests**

Run: `cargo test --features serde --test execution_pause_resume -- --nocapture`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/bridge.rs src/bin/voidctl.rs tests/execution_pause_resume.rs
git commit -m "bridge: add execution lifecycle endpoints"
```

### Task 16: Add policy patching and observability events

**Files:**
- Modify: `src/orchestration/service.rs`
- Modify: `src/bridge.rs`
- Test: `tests/execution_policy_patch.rs`

- [ ] **Step 1: Write failing tests for mutable and immutable policy fields**

Cover:

```rust
#[test]
fn patches_budget_and_concurrency_for_running_execution() {}

#[test]
fn rejects_mutation_of_convergence_and_evaluation() {}

#[test]
fn rejects_new_limits_below_consumed_values() {}

#[test]
fn emits_policy_updated_event() {}

#[test]
fn emits_budget_warning_and_stall_events() {}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --features serde --test execution_policy_patch -- --nocapture`
Expected: failures for missing patch service.

- [ ] **Step 3: Implement policy patch path**

Add:

- `PATCH /v1/executions/{id}/policy`

Emit `PolicyUpdated`, `IterationBudgetWarning`, and `ExecutionStalled` through the control-plane event log.

- [ ] **Step 4: Re-run tests**

Run: `cargo test --features serde --test execution_policy_patch -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/orchestration/service.rs src/bridge.rs tests/execution_policy_patch.rs
git commit -m "orchestration: add policy patching and observability"
```

## Chunk 6: Finish, Integrate, and Document

### Task 17: Wire exports and feature gates cleanly

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing compile/test command matrix note**

Record the intended matrix:

- `cargo test`
- `cargo test --features serde`

Expected: orchestration core should compile without requiring bridge-only serde HTTP code when feasible.

- [ ] **Step 2: Implement feature gating carefully**

Keep:

- pure orchestration logic available without HTTP server dependencies where possible
- filesystem store and JSON parsing under `serde` only if strictly required

Avoid making the entire library impossible to test without `serde` unless unavoidable.

- [ ] **Step 3: Run full unit suite**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 4: Run serde suite**

Run: `cargo test --features serde`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/lib.rs
git commit -m "build: wire orchestration modules and features"
```

### Task 18: Add integration coverage for execution contract behavior

**Files:**
- Modify: `tests/void_box_contract.rs`
- Optionally create: `tests/execution_contract.rs`

- [ ] **Step 1: Write failing contract-style tests against bridge**

Cover:

- dry-run side-effect freedom
- execution create/inspect/pause/resume/cancel
- policy patch validation
- final result provenance fields

- [ ] **Step 2: Run tests with `serde`**

Run: `cargo test --features serde execution_contract -- --nocapture`
Expected: failures for any missing bridge contract behavior.

- [ ] **Step 3: Implement missing glue only**

Do not move orchestration logic into the bridge. Keep bridge thin.

- [ ] **Step 4: Re-run tests**

Run: `cargo test --features serde execution_contract -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/void_box_contract.rs tests/execution_contract.rs
git commit -m "test: cover execution bridge contract"
```

### Task 19: Update docs and developer guidance

**Files:**
- Modify: `README.md`
- Optionally create: `docs/execution-orchestration.md`

- [ ] **Step 1: Document the new execution API and storage model**

Include:

- difference between `Run` and `Execution`
- current supported mode: `swarm`
- unsupported named modes: `search`, `tournament`
- persistence location and reconciliation behavior
- command examples

- [ ] **Step 2: Document verification commands**

Include exactly:

```bash
cargo test
cargo test --features serde
```

And any new targeted commands for execution tests.

- [ ] **Step 3: Re-read for consistency with spec**

Check that docs do not claim:

- distributed scheduling
- non-swarm strategy implementation
- LLM-based scoring

- [ ] **Step 4: Commit**

```bash
git add README.md docs/execution-orchestration.md
git commit -m "docs: describe execution orchestration"
```

## Cross-Cutting Design Constraints

- Keep the run-level runtime contract separate from the execution-level orchestration contract.
- Treat control-plane events as the primary execution truth; runtime inspection is reconciliation-only.
- Keep strategy methods pure and side-effect free.
- Avoid deep coupling between HTTP handlers and orchestration internals.
- Implement only `SwarmStrategy` initially even though the spec names future modes.
- Persist references to artifacts, not full artifact bodies.
- Do not infer execution success from logs alone.

## Risks and Open Decisions

### 1. Persistence format

The spec requires durable execution state and replayability, but the repo has no storage layer yet. Start with a filesystem store and wrap it in a trait so SQLite or another backend can replace it later without rewriting the control loop.

### 2. Background execution model

The spec implies long-running orchestration workers. If the first implementation runs everything synchronously inside request handlers, pause/resume and queue fairness will become fragile. Prefer a service object with an explicit worker thread or polling loop, even if single-process only.

### 3. Artifact contract mismatch

The current void-box API retrieves a single stage output file. The plan must preserve the v0.2 constraint that `result.json` is the stage’s structured output and treat richer artifact manifests as future work.

### 4. Feature-gate sprawl

The repo currently gates HTTP/JSON-heavy code behind `serde`. Keep the new orchestration layer as independent as possible so pure logic remains easy to unit test.

### 5. UI scope creep

The spec mentions visibility, but wiring the React app before backend contracts settle will create churn. Keep UI work out of the first implementation branch unless the backend is already stable.

## Verification Checklist

Run this full matrix before calling the implementation complete:

```bash
cargo test
cargo test --features serde
cargo test --features serde --test execution_spec_validation -- --nocapture
cargo test --features serde --test execution_dry_run -- --nocapture
cargo test --features serde --test execution_scheduler -- --nocapture
cargo test --features serde --test execution_pause_resume -- --nocapture
cargo test --features serde --test execution_policy_patch -- --nocapture
cargo test --features serde --test execution_reconciliation -- --nocapture
```

If a live daemon contract gate is added for executions later, keep it separate from the pure orchestration suite so implementation work does not block on an external service.

## Recommended Commit Sequence

1. `orchestration: add module scaffold`
2. `orchestration: validate execution specs`
3. `orchestration: add execution state and events`
4. `orchestration: add filesystem execution store`
5. `orchestration: add deterministic scoring`
6. `orchestration: add candidate variation generators`
7. `orchestration: add swarm iteration strategy`
8. `runtime: extend mock for execution orchestration tests`
9. `orchestration: add execution scheduler`
10. `orchestration: add execution control loop`
11. `orchestration: collect structured candidate outputs`
12. `orchestration: enforce execution failure semantics`
13. `orchestration: add execution reconciliation`
14. `bridge: add execution dry-run endpoint`
15. `bridge: add execution lifecycle endpoints`
16. `orchestration: add policy patching and observability`
17. `build: wire orchestration modules and features`
18. `test: cover execution bridge contract`
19. `docs: describe execution orchestration`

Plan complete and saved to `docs/superpowers/plans/2026-03-20-iteration-orchestration.md`. Ready to execute?
