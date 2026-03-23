# Void Control Iteration Specification

## Version: v0.1

## Scope
Define the control-plane iteration model for future `void-control`
execution modes, with `swarm` as the first motivating example.

This specification establishes:
- the control-plane object model,
- iteration and candidate lifecycle,
- event-mediated communication,
- how `void-control` consumes `void-box` completion information,
- strict boundaries between `void-control` and `void-box`.

This is a specification only. It does not require immediate
implementation of all modes described here.

---

# 1. Core Idea

`void-control` is the control plane.

`void-box` is the execution runtime.

`void-control` owns high-level execution modes that may require one or
many `void-box` child runs. Iterative modes, such as `swarm`, are
therefore a control-plane concern, not an internal `void-box` scheduler
mode.

The first-class resource is:

- `Execution`

The concrete runtime unit remains:

- `Run`

An `Execution` may create one or many child `Run`s across one or many
iterations.

---

# 2. Architectural Boundaries

## 2.1 `void-control` Responsibilities

- Accept and validate `ExecutionSpec`.
- Persist durable execution state.
- Own iteration state and candidate registry.
- Decide when to create, stop, or replace child runs.
- Consume runtime events and outputs from `void-box`.
- Derive control-plane events and execution status.
- Apply convergence, budget, and policy rules.

## 2.2 `void-box` Responsibilities

- Execute one concrete child `Run`.
- Isolate work inside microVM-backed stage execution.
- Emit runtime lifecycle and stage events.
- Persist stage output artifacts.
- Expose run completion status and stage-level output retrieval.

## 2.3 Strict Boundary Rules

`void-control` MUST NOT:
- depend on direct candidate-to-candidate transport,
- infer semantic execution state from raw logs alone,
- treat `void-box` as the owner of iteration state.

`void-box` MUST NOT:
- own swarm memory or iteration memory,
- decide convergence for an `Execution`,
- directly route messages between candidates,
- persist cross-run control-plane state.

---

# 3. Control-Plane Object Model

## 3.1 Execution

`Execution` is the top-level control-plane object.

Suggested shape:

```json
{
  "execution_id": "exec_123",
  "mode": "swarm",
  "status": "running",
  "goal": "optimize latency",
  "current_iteration": 2,
  "result": null
}
```

Required properties:
- `execution_id`
- `mode`
- `status`
- `goal`
- `policy`
- `created_at`
- `updated_at`

## 3.2 Iteration

An `Iteration` is one control-plane decision round inside an `Execution`.

An iteration owns:
- the candidate set launched in that round,
- the delivery window for messages visible to those candidates,
- evaluation/scoring results,
- iteration completion status.

## 3.3 Candidate

A `Candidate` is one evaluated alternative within an iteration.

A candidate may map to:
- exactly one child `Run` in the simple case, or
- multiple child `Run`s in future modes if needed.

For v0.1, the default mapping is:

`candidate -> one child run`

## 3.4 Child Run

A child `Run` is the concrete `void-box` execution backing a candidate.

Users should interact primarily with `Execution`.

Child runs are drill-down details for:
- logs,
- runtime events,
- stage graph,
- stage artifacts,
- failure debugging.

---

# 4. Execution Spec Model

`void-control` should accept a single `ExecutionSpec` envelope with
common fields and mode-specific sections.

Example:

```json
{
  "mode": "swarm",
  "goal": "optimize latency under load",
  "inputs": {},
  "policy": {},
  "workflow": {
    "template": {}
  },
  "swarm": {
    "max_iterations": 10
  }
}
```

Rules:
- common fields live at the top level,
- `workflow` is an execution primitive, not itself the control-plane mode,
- mode-specific sections are optional unless required by the selected mode,
- validation is mode-aware.

## 4.1 Mode Taxonomy

`Execution` modes should be treated as control-plane orchestration
strategies, not as aliases for workflow shape.

Suggested families:

- static modes
  - `single_run`
- delegated modes
  - `one_shot_agent`
  - `planner_executor`
- iterative modes
  - `swarm`
  - `search`
  - `tournament`

`swarm` is therefore not the only future mode.

For example, a one-shot delegated coding agent flow similar to Stripe's
Minions pattern is better modeled as a delegated mode than as `swarm`:

- one task is assigned,
- one execution owns the end-to-end lifecycle,
- the control plane tracks progress and artifacts,
- iteration across parallel candidates is not the primary abstraction.

This distinction keeps:
- iterative comparison logic in iterative modes,
- end-to-end autonomous task delegation in delegated modes,
- concrete workflow execution in child runs.

---

# 5. Iteration Semantics

## 5.1 Iteration Lifecycle

Each iteration proceeds through:

`Planned -> Dispatching -> Running -> Evaluating -> Completed`

Terminal iteration states:
- `Completed`
- `Failed`
- `Canceled`

## 5.2 Candidate Lifecycle

Each candidate proceeds through:

`Pending -> Scheduled -> Running -> {Succeeded | Failed | Canceled}`

Candidate completion is driven by the terminal state of its child run
plus any required structured outputs.

## 5.3 Control Loop

Iterative modes should follow this model:

```rust
loop {
    let inboxes = materialize_candidate_inboxes(execution_state);
    let candidates = plan_next_candidates(execution_state, inboxes);
    let child_runs = dispatch_candidates(candidates);
    let runtime_updates = collect_runtime_events(child_runs);
    let outputs = collect_candidate_outputs(child_runs);
    let derived = evaluate_iteration(runtime_updates, outputs);
    execution_state = reduce(execution_state, derived);

    if should_stop(execution_state) {
        break;
    }
}
```

This loop lives in `void-control`, not in `void-box`.

---

# 6. Event Model

## 6.1 Two Event Layers

The system uses two distinct event layers.

### Runtime Events

Produced by `void-box` child runs.

Examples:
- `RunStarted`
- `StageStarted`
- `StageCompleted`
- `StageFailed`
- `RunCompleted`
- `RunFailed`
- `RunCanceled`

These are low-level execution facts.

### Control-Plane Events

Produced by `void-control`.

Examples:
- `ExecutionCreated`
- `IterationPlanned`
- `IterationStarted`
- `CandidateScheduled`
- `CandidateMessageProduced`
- `CandidateMessageDelivered`
- `CandidateCompleted`
- `CandidateScored`
- `IterationCompleted`
- `ExecutionCompleted`
- `ExecutionFailed`
- `ExecutionCanceled`

These are orchestration facts.

## 6.2 Event Ownership Rule

Execution state in `void-control` MUST advance from persisted events and
reduced outputs.

Direct inspection of child runs may be used for reconciliation and repair,
but not as the primary source of orchestration truth.

## 6.3 Replayability Rule

Every orchestration decision that changes execution state MUST be
reconstructible from the control-plane event log plus referenced child-run
artifacts.

---

# 7. Candidate Communication

## 7.1 Communication Model

Candidates do not communicate directly with each other.

All candidate communication is mediated by `void-control`.

A candidate may express an intent to communicate, but delivery is always
a control-plane decision.

## 7.2 `@` Mentions

A candidate output such as `@candidate-b` or `@leader` is interpreted as
a communication intent, not direct transport.

Flow:

`child run output -> control-plane event -> routing decision -> next inbox`

## 7.3 Canonical Message Shape

Suggested control-plane message event:

```json
{
  "type": "candidate.message",
  "execution_id": "exec_123",
  "iteration": 2,
  "from_candidate_id": "cand_a",
  "mentions": ["cand_b"],
  "message": "Try the lower-concurrency variant",
  "visibility": "swarm"
}
```

## 7.4 Delivery Rule

For v0.1, messages should be delivered to future candidate inboxes, not
to already-running child runs.

This avoids mid-run coupling and keeps replay semantics simple.

## 7.5 Mailbox Rule

The canonical mailbox lives in control-plane state as persisted message
events plus derived delivery state.

`mailbox.json` is allowed only as:
- a generated inbox snapshot injected into a child run at launch time,
- a debug artifact,
- a convenience input format for agent code.

`mailbox.json` MUST NOT be the system of record.

---

# 8. Leader and Roles

## 8.1 Role Assignment

Leader semantics, when used, are assigned by `void-control`.

A leader is a role, not an autonomous authority.

The control plane may mark a candidate as:
- `leader`
- `reviewer`
- `researcher`
- other future logical roles

## 8.2 Authority Rule

The leader may produce intents such as:
- propose next candidates,
- summarize results,
- recommend a direction,
- address other candidates by logical role.

Those intents are advisory until `void-control` accepts and realizes
them.

## 8.3 Initial Support

For early versions, `void-control` should support:
- `leaderless`
- `fixed_leader`

Dynamic leader election may be added later.

---

# 9. State Ownership

## 9.1 Durable State

The following state MUST live in `void-control`:
- `ExecutionSpec`
- execution status
- iteration state
- candidate registry
- role assignments
- message history
- scoring history
- child run mapping
- convergence and stop reason
- artifact references

## 9.2 Ephemeral State

The following state may live only inside a child `void-box` run:
- local filesystem data for that run,
- task input files,
- generated mailbox snapshot,
- temporary artifacts,
- process-local execution context.

## 9.3 Restart Rule

If a restart requires the state to continue or reconstruct the execution,
that state belongs in `void-control`.

---

# 10. Completion Information from `void-box`

## 10.1 Required Runtime Completion Sources

`void-control` should use the following existing `void-box` completion
surfaces:

- child run terminal status,
- stable terminal event id,
- resumable run event stream,
- stage snapshot endpoint,
- persisted stage output artifacts,
- runtime run report when available.

## 10.2 Completion Mapping

For each child run, `void-control` should collect:

### Lifecycle Completion

From runtime events and run inspection:
- terminal state,
- terminal event id,
- failure/cancel reason,
- timestamps,
- attempt id.

### Stage Completion

From stage snapshots:
- per-stage terminal status,
- timing,
- exit code,
- dependency shape,
- stage grouping.

### Semantic Completion

From stage artifacts and/or run output:
- candidate result summary,
- candidate metrics,
- communication intents,
- referenced artifacts.

## 10.3 Structured Output Rule

Logs alone are insufficient for control-plane iteration decisions.

Iterative modes SHOULD define a structured artifact contract for child
runs, such as:
- `result.json`
- `intents.json`
- `artifacts.json`

These names are illustrative in v0.1; exact filenames may be finalized
later. The core requirement is stable structured output, not filename
choice.

## 10.4 Candidate Completion Rule

A candidate is only fully complete when:
- its child run is terminal, and
- all required structured outputs for the mode have been collected or
  explicitly marked absent.

---

# 11. Reconciliation

On `void-control` restart:
- reload non-terminal executions,
- reload candidate-to-run mapping,
- inspect known child runs,
- resume runtime event consumption from the last seen event id,
- rebuild derived inboxes and iteration status from control-plane events.

Reconciliation may use runtime inspection and runtime event replay, but
the rebuilt execution state must still be reduced into the control-plane
model.

---

# 12. UI and API Visibility

## 12.1 Primary View

Users should primarily see:
- execution status,
- current iteration,
- candidate counts,
- scores,
- current best result,
- orchestration timeline.

## 12.2 Drill-Down View

Users may drill into child runs for:
- per-run events,
- logs,
- stage graph,
- output artifacts,
- detailed failure diagnosis.

The UI and API should not force users to reason about all child runs by
default.

---

# 13. Non-Goals for v0.1

- Direct candidate-to-candidate transport.
- Shared mutable mailbox files as canonical state.
- Mid-run message injection into already-running child runs.
- Leader election semantics.
- Multi-node distributed runtime scheduling.
- A final stable schema for all mode-specific artifacts.

---

# 14. Acceptance Criteria

This specification is satisfied when a future implementation can:

1. Create one top-level `Execution` that spans multiple iterations.
2. Launch multiple child `void-box` runs in parallel for one iteration.
3. Track candidate completion using existing `void-box` terminal events
   and run state.
4. Collect semantic candidate outputs from structured artifacts rather
   than logs alone.
5. Convert candidate communication intents into persisted control-plane
   message events.
6. Materialize candidate inbox snapshots for future iterations without
   making those snapshots the system of record.
7. Reconstruct execution state after restart from persisted control-plane
   state plus replayed child-run information.
