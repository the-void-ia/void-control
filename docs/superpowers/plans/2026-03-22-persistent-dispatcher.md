# Persistent Dispatcher Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current “scan pending executions and process them to completion” worker loop with a persistent dispatcher that stores queued and running candidate state, enforces execution-local and global concurrency rules, survives restart, and exposes queue/dispatch state through persisted events and bridge inspection.

**Architecture:** Keep execution planning, scheduling, and candidate dispatch separate. `ExecutionService` should plan and persist candidate work, `scheduler.rs` should decide which persisted candidates are runnable under global and per-execution limits, `store/fs.rs` should become the source of truth for queued/running candidate records, and `bridge.rs` should drive a dispatcher tick that advances work incrementally instead of executing an entire orchestration loop in one pass.

**Tech Stack:** Rust 2021, filesystem-backed execution store, existing `serde`-gated bridge and orchestration modules, current runtime trait abstraction, persisted control-plane events, and the existing `GlobalScheduler` primitives as the starting point.

---

## Scope Check

This plan includes:
1. persisted candidate queue/running state,
2. dispatcher ticks with global and per-execution slot enforcement,
3. restart reconciliation of queued/running candidates,
4. bridge/inspection updates for queue and dispatch observability,
5. focused restart and fairness tests.

This plan intentionally excludes:
- additional orchestration strategies beyond `swarm`,
- UI work,
- richer candidate artifact history beyond what dispatching needs,
- database-backed persistence,
- distributed locking beyond the current filesystem claim model.

## File Map

### Primary files

- Modify: `src/orchestration/types.rs`
  Responsibility: add persisted candidate record types and candidate lifecycle status.
- Modify: `src/orchestration/store/fs.rs`
  Responsibility: persist candidate queue/running/completed state and reload it on restart.
- Modify: `src/orchestration/store.rs`
  Responsibility: expose candidate-oriented store operations through the store abstraction.
- Modify: `src/orchestration/service.rs`
  Responsibility: split planning from dispatch and process one persisted candidate at a time.
- Modify: `src/orchestration/scheduler.rs`
  Responsibility: decide runnable candidates from persisted state under spec rules.
- Modify: `src/orchestration/events.rs`
  Responsibility: add queue/dispatched/released candidate lifecycle events required by the dispatcher.
- Modify: `src/orchestration/reconcile.rs`
  Responsibility: rebuild dispatcher state from persisted executions and candidates after restart.
- Modify: `src/bridge.rs`
  Responsibility: replace the simple pending-execution scan with a dispatcher tick and expose queue state via inspection.

### Supporting tests

- Modify: `tests/execution_scheduler.rs`
  Responsibility: fairness, ordering, slot release, and candidate lifecycle coverage.
- Modify: `tests/execution_reconciliation.rs`
  Responsibility: restart rebuild of queued/running candidate state.
- Modify: `tests/execution_worker.rs`
  Responsibility: worker dispatch of persisted candidates and restart-safe progression.
- Modify: `tests/execution_bridge.rs`
  Responsibility: bridge inspection surface for queued/running candidate summaries.

## Delivery Strategy

Implement in this order:

1. define persisted candidate records and store support,
2. split orchestration into planning and candidate dispatch,
3. wire a dispatcher tick around the persisted queue,
4. add restart reconciliation and queue observability,
5. run the full verification sweep.

This order keeps the store model authoritative and avoids building more behavior on top of the current monolithic `process_execution()` loop.

## Chunk 1: Persisted Candidate Records

### Task 1: Add candidate lifecycle types

**Files:**
- Modify: `src/orchestration/types.rs`

- [ ] **Step 1: Add a candidate record model**

Add types such as:

```rust
pub enum CandidateStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Canceled,
}

pub struct ExecutionCandidate {
    pub execution_id: String,
    pub candidate_id: String,
    pub created_seq: u64,
    pub iteration: u32,
    pub status: CandidateStatus,
    pub runtime_run_id: Option<String>,
}
```

Do not add candidate metric history yet. Keep the type limited to what dispatching and restart recovery need.

- [ ] **Step 2: Add store support in `src/orchestration/store/fs.rs`**

Persist candidate records separately from `execution.txt`, for example under:
- `candidates/<candidate_id>.txt`, or
- a single `candidates.log` plus a reload parser.

Prefer a simple per-candidate file because it makes reload and patch updates easier.

- [ ] **Step 3: Extend `src/orchestration/store.rs`**

Expose store methods for:
- save candidate,
- load candidates for one execution,
- list runnable/active candidates across executions.

- [ ] **Step 4: Add focused store tests**

Add tests that round-trip queued and running candidate records through the filesystem store.

- [ ] **Step 5: Verify**

Run:

```bash
cargo test --features serde --test execution_worker -- --nocapture
cargo test --features serde --test execution_scheduler -- --nocapture
```

Expected: compilation and any new store tests pass.

## Chunk 2: Split Planning From Dispatch

### Task 2: Persist candidate queue state instead of dispatching immediately

**Files:**
- Modify: `src/orchestration/service.rs`
- Modify: `src/orchestration/events.rs`

- [ ] **Step 1: Introduce a planning-only phase**

Refactor `process_execution()` so the iteration loop can:
- materialize inboxes,
- plan candidates,
- persist them as `Queued`,
- emit `CandidateQueued`,
- stop before calling `runtime.start_run(...)` inline.

- [ ] **Step 2: Add a one-candidate dispatch path**

Introduce a method with a shape like:

```rust
fn dispatch_candidate(
    &mut self,
    execution: &mut Execution,
    candidate: &ExecutionCandidate,
    spec: &ExecutionSpec,
    worker_id: &str,
) -> io::Result<DispatchOutcome>
```

This method should:
- mark the candidate `Running`,
- start the runtime run,
- wait/poll to terminal,
- collect structured output,
- mark the candidate terminal,
- update accumulator/execution state incrementally.

- [ ] **Step 3: Add/extend events**

Add or use events for:
- `CandidateQueued`
- `CandidateDispatched`
- `CandidateOutputCollected`
- candidate terminal release/failure if needed for observability

Do not remove the existing execution lifecycle events.

- [ ] **Step 4: Keep the current single-process semantics temporarily**

Within this chunk, it is acceptable if one dispatcher tick still drains all runnable candidates. The main requirement is that candidates are persisted and lifecycle transitions are explicit.

- [ ] **Step 5: Verify**

Run:

```bash
cargo test --features serde --test execution_worker -- --nocapture
cargo test --features serde --test execution_event_replay -- --nocapture
```

Expected: worker tests still pass and event replay remains consistent.

## Chunk 3: Dispatcher Tick and Slot Enforcement

### Task 3: Enforce scheduler rules from persisted state

**Files:**
- Modify: `src/orchestration/scheduler.rs`
- Modify: `src/bridge.rs`
- Modify: `src/orchestration/service.rs`

- [ ] **Step 1: Build a scheduler view from persisted candidates**

The dispatcher tick should reconstruct runnable work from:
- queued candidates,
- running candidates,
- execution pause/cancel state,
- global child-run limit,
- per-execution max concurrent candidates.

- [ ] **Step 2: Enforce spec ordering**

Ensure:
- within one execution, dispatch order matches persisted candidate creation sequence,
- across executions, dispatch is FIFO by candidate creation time.

- [ ] **Step 3: Release slots on candidate completion**

Candidate completion should immediately make capacity available on the next tick.

- [ ] **Step 4: Replace the simple bridge worker scan**

Update `process_pending_executions_once()` in `src/bridge.rs` to:
- queue work for executions that need planning,
- dispatch runnable persisted candidates,
- avoid treating one execution as a monolithic unit.

- [ ] **Step 5: Add scheduler tests**

Extend `tests/execution_scheduler.rs` to cover:
- persisted FIFO across executions,
- execution-local order preservation after restart,
- slot release on completion,
- paused execution not dispatching queued candidates.

- [ ] **Step 6: Verify**

Run:

```bash
cargo test --features serde --test execution_scheduler -- --nocapture
cargo test --features serde --test execution_worker -- --nocapture
```

Expected: PASS.

## Chunk 4: Restart Reconciliation

### Task 4: Rebuild dispatcher state after restart

**Files:**
- Modify: `src/orchestration/reconcile.rs`
- Modify: `src/orchestration/store/fs.rs`
- Modify: `tests/execution_reconciliation.rs`

- [ ] **Step 1: Reload queued and running candidates from disk**

Reconciliation should restore:
- active executions,
- queued candidates,
- running candidates,
- execution-level pause/cancel state,
- accumulator state.

- [ ] **Step 2: Define restart behavior for running candidates**

At first cut, choose one explicit behavior and encode it in tests:
- either mark previously running candidates back to `Queued`,
- or mark them failed/stalled and allow replan.

Recommendation:
- move previously `Running` candidates back to `Queued` on restart unless the runtime can be proven terminal from reconciliation data.

- [ ] **Step 3: Add restart tests**

Cover:
- queued candidates remain queued after restart,
- running candidates are recovered into a safe resumable state,
- completed candidates are not re-dispatched.

- [ ] **Step 4: Verify**

Run:

```bash
cargo test --features serde --test execution_reconciliation -- --nocapture
```

Expected: PASS.

## Chunk 5: Bridge Observability

### Task 5: Expose queue and dispatch state through execution inspection

**Files:**
- Modify: `src/bridge.rs`
- Modify: `tests/execution_bridge.rs`

- [ ] **Step 1: Extend execution detail response**

Add queue-oriented fields such as:
- queued candidate count,
- running candidate count,
- completed candidate count,
- maybe the next queued candidate id.

- [ ] **Step 2: Keep event history as the source of truth**

Do not invent a second in-memory scheduler status model in the bridge. Derive summaries from persisted candidate and event state.

- [ ] **Step 3: Add bridge tests**

Add tests that validate:
- execution detail includes queued/running/completed counts,
- event stream stays stable,
- paused executions show queued candidates without dispatch progress.

- [ ] **Step 4: Verify**

Run:

```bash
cargo test --features serde --test execution_bridge -- --nocapture
```

Expected: PASS.

## Final Verification

- [ ] Run the full suite:

```bash
cargo test --features serde
```

- [ ] If a live daemon is available, rerun live bridge tests serially:

```bash
TMPDIR=/tmp CARGO_TARGET_DIR=/home/diego/github/void-control/target \
VOID_BOX_BASE_URL=http://127.0.0.1:43100 \
cargo test --features serde --test execution_bridge_live -- --ignored --nocapture --test-threads=1
```

- [ ] Review bridge inspection output for one completed and one paused execution.

## Risks and Notes

- The existing `process_execution()` logic currently couples planning and evaluation tightly. The refactor must preserve current scoring behavior while changing when dispatch happens.
- Persisting candidate records will expose backward-compatibility questions for existing temp stores. Treat old stores as best-effort and prefer forward correctness.
- Restart behavior for previously running candidates must be explicit and test-backed. Hidden assumptions here will cause duplicate work or dropped work.
- Keep the first cut filesystem format simple. A later migration to SQLite or a more structured store is easier if the behavior is already correct and well-tested.

## Definition of Done

The plan is complete when:
- executions persist queued/running candidate records,
- dispatcher ticks enforce global and per-execution slot rules from persisted state,
- restart reconciliation rebuilds runnable work safely,
- bridge inspection exposes queue/dispatch progress,
- all `cargo test --features serde` tests pass,
- live bridge tests still pass when run serially against the real daemon.
