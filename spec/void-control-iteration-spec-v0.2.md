# Void Control Iteration Specification

## Version: v0.2

## Changelog

- v0.2: Added policy model, evaluation contract, candidate variation,
  failure semantics, artifact retrieval, iteration state threading,
  iteration strategy trait, backpressure/concurrency, observability
  events, execution checkpointing, mid-execution policy adjustment,
  dry-run mode, and result provenance. Updated acceptance criteria.
- v0.1: Initial specification.

## Scope

Define the control-plane iteration model for future `void-control`
execution modes, with `swarm` as the first motivating example.

This specification establishes:
- the control-plane object model,
- iteration and candidate lifecycle,
- event-mediated communication,
- how `void-control` consumes `void-box` completion information,
- strict boundaries between `void-control` and `void-box`,
- the policy, evaluation, and variation models,
- failure semantics and operational controls.

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

# 2. Layered Architecture

## 2.1 Two Layers Within `void-control`

`void-control` is internally organized into two layers:

### Runtime Integration Layer (existing)

The contract and runtime modules (`src/contract/`, `src/runtime/`)
provide the integration surface with `void-box`. This layer:

- defines canonical types for individual runs: `RunState`,
  `EventEnvelope`, `EventType`, `EventSequenceTracker`,
  `ExecutionPolicy`.
- defines the runtime interaction API: `StartRequest`, `StartResult`,
  `StopRequest`, `StopResult`, `RuntimeInspection`,
  `SubscribeEventsRequest`.
- provides concrete clients: `VoidBoxRuntimeClient` (HTTP transport)
  and `MockRuntime` (testing).
- handles compatibility mapping from void-box wire format to canonical
  types (compat layer).

This layer knows about one run at a time. It has no concept of
executions, iterations, candidates, or scoring.

### Orchestration Layer (this spec)

The iteration model defined in this specification sits above the
runtime integration layer. This layer:

- defines the multi-run object model: `Execution`, `Iteration`,
  `Candidate`, `ExecutionAccumulator`.
- owns the control loop that creates, monitors, and evaluates multiple
  child runs across iterations.
- manages cross-run concerns: scoring, variation, convergence,
  communication, concurrency, and budget.
- produces control-plane events that are distinct from runtime events.

### How the Layers Connect

The orchestration layer consumes the runtime layer — it never bypasses
it to talk to void-box directly.

| Orchestration action | Runtime layer call |
|----------------------|-------------------|
| Dispatch a candidate | `VoidBoxRuntimeClient::start(StartRequest)` with the resolved candidate spec |
| Cancel a child run | `VoidBoxRuntimeClient::stop(StopRequest)` |
| Check child run status | `VoidBoxRuntimeClient::inspect()` → `RuntimeInspection` |
| Consume child run events | `VoidBoxRuntimeClient::subscribe_events(SubscribeEventsRequest)` → stream of `EventEnvelope` |
| Retrieve stage artifacts | `GET /v1/runs/{id}/stages/{stage}/output-file` (via HTTP transport) |

The shared infrastructure functions in the control loop map to these
calls:

- `dispatch_candidates()` → iterates candidate specs, calls `start()`
  for each, records the `child_run_id` from `StartResult`, emits
  `CandidateDispatched` control-plane event.
- `collect_outputs()` → subscribes to events for each child run via
  `subscribe_events()`, waits for terminal `EventEnvelope`, then
  fetches artifacts. Maps `RuntimeInspection` fields (terminal state,
  exit code, timestamps) into `CandidateOutput`.
- Failure handling → calls `stop()` for timeout or cancellation, reads
  `RuntimeInspection.terminal_reason` for diagnostics.

### Policy Mapping

The existing `ExecutionPolicy` in the contract layer
(`max_parallel_microvms_per_run`, `max_stage_retries`,
`stage_timeout_secs`, `cancel_grace_period_secs`) controls per-run
behavior inside void-box.

The orchestration-layer `policy` defined in this spec (Section 14)
controls cross-run behavior: budget, concurrency across candidates,
convergence, and failure escalation.

Both policies coexist. When dispatching a candidate, the orchestration
layer passes the contract-level `ExecutionPolicy` through to
`StartRequest` for the child run, while applying its own policy to
decide whether to dispatch at all.

## 2.2 Responsibility Boundaries

### `void-control` Responsibilities

- Accept and validate `ExecutionSpec`.
- Persist durable execution state.
- Own iteration state and candidate registry.
- Decide when to create, stop, or replace child runs.
- Consume runtime events and outputs from `void-box`.
- Derive control-plane events and execution status.
- Apply convergence, budget, and policy rules.
- Score candidates and track evaluation history.
- Manage concurrency across executions.

### `void-box` Responsibilities

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
  "policy": {},
  "result": null,
  "created_at": "2026-03-18T10:00:00Z",
  "updated_at": "2026-03-18T10:05:00Z"
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

Execution status enum:

`Pending | Running | Paused | Completed | Failed | Canceled`

Valid transitions:

```
Pending -> Running
Running -> Paused
Running -> Completed
Running -> Failed
Running -> Canceled
Paused  -> Running
Paused  -> Canceled
```

Note: `evaluation` and `variation` are part of the `ExecutionSpec`
(submission-time configuration) but are not runtime fields on the
`Execution` object. They are referenced from the persisted
`ExecutionSpec`.

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

For v0.2, the default mapping is:

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
  "policy": {
    "budget": {
      "max_iterations": 10,
      "max_child_runs": 50,
      "max_wall_clock_secs": 3600,
      "max_cost_usd": 25.00
    },
    "concurrency": {
      "max_concurrent_candidates": 4
    },
    "convergence": {
      "strategy": "threshold",
      "min_score": 0.85,
      "max_iterations_without_improvement": 3
    },
    "failure": {
      "max_candidate_failures_per_iteration": 2,
      "iteration_failure_policy": "fail_execution",
      "missing_output_policy": "mark_failed"
    }
  },
  "evaluation": {
    "scoring": {
      "type": "weighted_metrics",
      "weights": {
        "latency_p99_ms": { "weight": 0.6, "direction": "minimize" },
        "cost_usd": { "weight": 0.4, "direction": "minimize" }
      },
      "pass_threshold": 0.7
    },
    "ranking": "highest_score",
    "tie_breaking": "lowest_cost"
  },
  "variation": {
    "source": "parameter_space",
    "parameter_space": {
      "sandbox.memory_mb": [512, 1024, 2048],
      "sandbox.env.CONCURRENCY": ["4", "8", "16"]
    },
    "candidates_per_iteration": 3,
    "selection": "random"
  },
  "workflow": {
    "template": {}
  },
  "swarm": {}
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

`Pending -> Queued -> Dispatching -> Running -> {Succeeded | Failed | Canceled}`

`Queued` indicates the candidate is waiting for a concurrency slot (see
Section 21).

Candidate completion is driven by the terminal state of its child run
plus any required structured outputs.

## 5.3 Control Loop

Iterative modes should follow this model:

```rust
let strategy = strategy_for_mode(execution.mode);

loop {
    let inboxes = strategy.materialize_inboxes(&accumulator);
    let candidates = strategy.plan_candidates(&accumulator, &inboxes);
    let child_runs = dispatch_candidates(candidates);
    let outputs = collect_outputs(child_runs);
    let evaluation = strategy.evaluate(&accumulator, &outputs);

    if let Some(reason) = strategy.should_stop(&accumulator, &evaluation) {
        finalize_execution(accumulator, reason);
        break;
    }

    accumulator = strategy.reduce(accumulator, evaluation);
}
```

This loop lives in `void-control`, not in `void-box`.

`dispatch_candidates()` and `collect_outputs()` are shared
infrastructure that handle void-box interaction, concurrency,
artifact retrieval, and failure handling. They are not mode-specific.

---

# 6. Event Model

## 6.1 Two Event Layers

The system uses two distinct event layers.

### Runtime Events

Produced by `void-box` child runs.

Examples:
- `RunStarted`
- `StageStarted`
- `StageSucceeded`
- `StageFailed`
- `RunCompleted`
- `RunFailed`
- `RunCancelled`

These are low-level execution facts.

Note: `void-box` uses British spelling (`RunCancelled`, `StageSucceeded`)
for some event types. `void-control` normalizes these via the compatibility
layer (e.g., `RunCancelled` → `RunCanceled`). This spec uses the
`void-control` canonical names throughout. The compat layer handles the
mapping.

### Control-Plane Events

Produced by `void-control`.

Lifecycle events:
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

Operational events (see Section 22):
- `CandidateQueued`
- `CandidateDispatched`
- `CandidateOutputCollected`
- `CandidateOutputError`
- `CandidateTimeout`
- `IterationBudgetWarning`
- `ExecutionBudgetExhausted`
- `ExecutionStalled`
- `ExecutionPaused`
- `ExecutionResumed`
- `PolicyUpdated`

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

For v0.2, messages should be delivered to future candidate inboxes, not
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
- execution accumulator

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

These names are illustrative in v0.2; exact filenames may be finalized
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
- rebuild derived inboxes and iteration status from control-plane events,
- reconstruct execution accumulator from event log.

Reconciliation may use runtime inspection and runtime event replay, but
the rebuilt execution state must still be reduced into the control-plane
model.

Paused executions remain paused after reconciliation.

---

# 12. UI and API Visibility

## 12.1 Primary View

Users should primarily see:
- execution status,
- current iteration,
- candidate counts,
- scores,
- current best result,
- orchestration timeline,
- budget consumption.

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

# 13. Non-Goals for v0.2

- Direct candidate-to-candidate transport.
- Shared mutable mailbox files as canonical state.
- Mid-run message injection into already-running child runs.
- Leader election semantics.
- Multi-node distributed runtime scheduling.
- A final stable schema for all mode-specific artifacts.
- LLM-in-the-loop evaluation (leader-as-scorer).
- Execution priority across competing executions.
- Artifact push via event payload (see Section 18.6 future notes).

---

# 14. Policy Model

## 14.1 Policy Shape

The `policy` field in `ExecutionSpec` controls budget, concurrency,
convergence, and failure behavior.

```json
{
  "policy": {
    "budget": {
      "max_iterations": 10,
      "max_child_runs": 50,
      "max_wall_clock_secs": 3600,
      "max_cost_usd": 25.00
    },
    "concurrency": {
      "max_concurrent_candidates": 4
    },
    "convergence": {
      "strategy": "threshold",
      "min_score": 0.85,
      "max_iterations_without_improvement": 3
    },
    "failure": {
      "max_candidate_failures_per_iteration": 2,
      "iteration_failure_policy": "fail_execution",
      "missing_output_policy": "mark_failed"
    }
  }
}
```

## 14.2 Budget

Budget fields are hard limits. Exceeding any one stops the execution.

All budget fields are individually optional, but at least one of
`max_iterations` or `max_wall_clock_secs` MUST be set. No unbounded
executions are allowed.

| Field | Type | Description |
|-------|------|-------------|
| `max_iterations` | integer | Maximum number of iterations |
| `max_child_runs` | integer | Maximum total child runs across all iterations |
| `max_wall_clock_secs` | integer | Maximum wall-clock time (paused time excluded) |
| `max_cost_usd` | float | Maximum total cost across all child runs |

## 14.3 Concurrency

| Field | Type | Description |
|-------|------|-------------|
| `max_concurrent_candidates` | integer | Maximum in-flight candidates for this execution |

Cannot exceed the global pool size. Validated at submission time.

## 14.4 Convergence

| Field | Type | Description |
|-------|------|-------------|
| `strategy` | enum | `threshold`, `plateau`, or `exhaustive` |
| `min_score` | float | Stop when best score >= this value (`threshold` strategy) |
| `max_iterations_without_improvement` | integer | Stop after N iterations with no score improvement (`plateau` strategy) |

- `threshold`: stop when a candidate scores >= `min_score`. Requires
  `min_score`.
- `plateau`: stop after `max_iterations_without_improvement` consecutive
  iterations where `best_result` does not improve. Requires
  `max_iterations_without_improvement`.
- `exhaustive`: run all `max_iterations` iterations regardless of scores.
  Requires `policy.budget.max_iterations` to be set (otherwise there is
  no bound).

Providing fields not relevant to the selected strategy (e.g., `min_score`
with `exhaustive`) is ignored — not an error. This allows changing the
strategy without removing unrelated fields.

## 14.5 Failure

| Field | Type | Description |
|-------|------|-------------|
| `max_candidate_failures_per_iteration` | integer | Short-circuit iteration after this many candidate failures |
| `iteration_failure_policy` | enum | `fail_execution`, `retry_iteration`, or `continue` |
| `missing_output_policy` | enum | `mark_failed` or `mark_incomplete` |
| `candidate_timeout_secs` | integer | Cancel a child run if it exceeds this duration. Default: inherited from the workflow template's `timeout_secs`. |

## 14.6 Validation

Policy validation happens at `ExecutionSpec` submission time:
- At least one of `max_iterations` or `max_wall_clock_secs` must be set.
- All numeric fields must be positive.
- `max_concurrent_candidates` must not exceed the global pool size.
- Convergence strategy must be consistent with provided fields (e.g.,
  `threshold` requires `min_score`).

---

# 15. Evaluation Contract

## 15.1 Scoring Model

The control plane runs a deterministic scoring function against
structured candidate outputs. No LLM participates in evaluation.

## 15.2 Scoring Input

For each candidate, void-control collects:
- the structured output artifact (`result.json`),
- child run terminal status,
- child run metrics (duration, cost, token usage).

## 15.3 Scoring Function

```rust
trait ScoringFunction {
    fn score(&self, candidate_output: &CandidateOutput) -> ScoringResult;
}
```

`ScoringResult` shape:

```json
{
  "candidate_id": "cand_a",
  "score": 0.82,
  "metrics": {
    "latency_p99_ms": 142,
    "cost_usd": 0.03,
    "duration_ms": 45000
  },
  "pass": true,
  "reason": "meets latency target, under cost cap"
}
```

## 15.4 Scoring Configuration

```json
{
  "evaluation": {
    "scoring": {
      "type": "weighted_metrics",
      "weights": {
        "latency_p99_ms": { "weight": 0.6, "direction": "minimize" },
        "cost_usd": { "weight": 0.4, "direction": "minimize" }
      },
      "pass_threshold": 0.7
    },
    "ranking": "highest_score",
    "tie_breaking": "lowest_cost"
  }
}
```

## 15.5 Scoring Types

For v0.2:

- `weighted_metrics`: weighted combination of numeric fields from
  `result.json` metrics. Each weight specifies a direction (`minimize`
  or `maximize`). Values are normalized using min-max normalization
  across all candidates within the current iteration (0.0 = worst,
  1.0 = best, direction-aware). For `minimize` metrics, lower raw
  values produce higher normalized scores. When only one candidate
  exists, normalized values default to 1.0. Note: because
  normalization is per-iteration, raw scores are not directly
  comparable across iterations. The `best_result` comparison in the
  accumulator uses raw metric values, not normalized scores.
- `pass_fail`: binary scoring based on the presence and validity of
  required fields in the candidate output. Score is `1.0` for pass,
  `0.0` for fail.

Future:
- `custom`: user-provided function reference.

## 15.6 Scoring Rules

- Every candidate gets a score. Failed candidates get score `0.0` with
  `pass: false`.
- Iteration best is determined by the `ranking` strategy.
- Execution best is the best score across all iterations.
- Scoring results are persisted as `CandidateScored` control-plane
  events.

---

# 16. Candidate Variation Model

## 16.1 Two-Layer Design

Candidate variation separates mechanism from strategy:
- **Mechanism:** template + overrides (how candidates are expressed).
- **Strategy:** variation source (how differences are decided).

## 16.2 Mechanism: Template + Overrides

Every candidate is expressed as a base workflow template plus a set of
overrides.

```json
{
  "candidate_id": "cand_a",
  "iteration": 2,
  "base_template": "workflow.template",
  "overrides": {
    "agent.prompt": "Try a streaming approach with chunked responses",
    "sandbox.memory_mb": 1024,
    "sandbox.env": {
      "CONCURRENCY": "8"
    }
  }
}
```

Rules:
- The base template comes from `ExecutionSpec.workflow.template`.
- Overrides use dot-path notation to target specific fields.
- Overrides are shallow-merged — they replace the target value, not
  deep-merge.
- The resolved candidate spec is a pure function of
  `template + overrides` (reproducible).
- The resolved spec is persisted with the `CandidateScheduled` event
  for replay.

## 16.3 Strategy: Variation Source

The `variation` section in the `ExecutionSpec` defines how overrides are
generated.

```json
{
  "variation": {
    "source": "parameter_space",
    "parameter_space": {
      "sandbox.memory_mb": [512, 1024, 2048],
      "sandbox.env.CONCURRENCY": ["4", "8", "16"]
    },
    "candidates_per_iteration": 3,
    "selection": "random"
  }
}
```

## 16.4 Variation Sources

For v0.2:

- `parameter_space`: enumerate or sample from a defined space of
  override values.
  - `selection`: `random` (sample randomly), `sequential` (enumerate in
    order). Future: `latin_hypercube` (space-filling sample).
- `explicit`: user provides a fixed list of override sets. Each
  iteration cycles through the list.
- `leader_directed`: overrides come from the leader candidate's
  structured output (`intents.json`).

## 16.5 Leader-Directed Variation

When `variation.source` is `leader_directed`, the leader candidate's
`intents.json` includes:

```json
{
  "proposed_candidates": [
    {
      "rationale": "lower concurrency showed promise, try even lower",
      "overrides": {
        "sandbox.env.CONCURRENCY": "2"
      }
    }
  ]
}
```

These proposals are advisory. `void-control` validates and may reject
or modify them before scheduling.

---

# 17. Failure Decision Tree

## 17.1 Candidate-Level Failures

| Scenario | Action |
|----------|--------|
| Child run fails (non-zero exit, RunFailed) | Candidate marked `Failed`, score `0.0`, `pass: false`. Counts toward `max_candidate_failures_per_iteration`. |
| Child run succeeds but structured output missing | Governed by `policy.failure.missing_output_policy`: `mark_failed` (default) treats as candidate failure; `mark_incomplete` scores `0.0` but does not count as failure. |
| Child run succeeds but structured output malformed | Same as missing — policy decides. Control plane emits `CandidateOutputError` event with diagnostic details. |
| Child run times out | void-control cancels the child run via void-box cancel API. Candidate marked `Failed` with `terminal_reason: "timeout"`. |

## 17.2 Iteration-Level Failures

| Scenario | Action |
|----------|--------|
| Some candidates fail, others succeed | Iteration completes normally. Failed candidates are scored but excluded from ranking. |
| All candidates fail | Governed by `policy.failure.iteration_failure_policy`: `fail_execution` (default) terminates the execution; `retry_iteration` re-runs the iteration (hardcoded limit of 1 retry in v0.2 — a configurable retry count may be added later); `continue` advances to next iteration with empty results. |
| Candidate failure count exceeds `max_candidate_failures_per_iteration` | Iteration is short-circuited. Remaining in-flight candidates are allowed to finish but no new candidates are dispatched. Iteration status is `Failed`. |

## 17.3 Execution-Level Failures

| Scenario | Action |
|----------|--------|
| Budget exhausted (any limit) | Current iteration completes (in-flight candidates finish). Execution terminates with `stop_reason: "budget_exhausted"` and the specific limit that was hit. Best result so far is the execution result. |
| void-box unreachable | void-control retries with exponential backoff (3 attempts, 1s/2s/4s). If still unreachable, in-flight candidates for that run are marked `Failed` with `terminal_reason: "runtime_unavailable"`. Normal failure cascading applies. |
| Unrecoverable control-plane error | Execution marked `Failed` with `error` field. All in-flight child runs are cancelled. |

## 17.4 Failure Visibility

Failures are always explicit — no silent drops. Every failure path
produces a control-plane event with enough context to diagnose.

---

# 18. Artifact Retrieval Protocol

## 18.1 v0.2: Pull After Terminal Event

When void-control observes a child run reach terminal state (via
`RunCompleted`, `RunFailed`, or `RunCanceled` event), it fetches
artifacts:

```
1. Observe terminal event for child run.
2. GET /v1/runs/{run_id}/stages/{stage_name}/output-file
   for each required artifact (result.json, intents.json).
3. Parse and validate artifact against mode's schema.
4. Emit CandidateOutputCollected or CandidateOutputError event.
5. Candidate is now evaluable.
```

## 18.2 Required Artifacts Per Mode

| Mode | Required | Optional |
|------|----------|----------|
| `swarm` | `result.json` | `intents.json` |
| `search` | `result.json` | — |
| `tournament` | `result.json` | — |

## 18.3 `result.json` Minimal Schema

```json
{
  "status": "success",
  "summary": "human-readable result description",
  "metrics": {},
  "artifacts": []
}
```

- `metrics` is a flat key-value map of numeric values. These are the
  inputs to the scoring function.
- `artifacts` is a list of references to additional output files (paths
  within the stage output).

## 18.4 void-box API Note

The current void-box endpoint `GET /v1/runs/{run_id}/stages/{stage}/output-file`
returns a single file per stage. For v0.2, the structured output contract
requires that the child run's output stage produces a single JSON file
containing all required fields (`status`, `summary`, `metrics`,
`artifacts`). This is the `result.json` content, retrieved as the stage's
sole output file.

If `intents.json` is required (e.g., for `leader_directed` variation), it
should be embedded as a field within the same output file, or a separate
stage should produce it. A future void-box enhancement may add named
artifact retrieval (e.g., `?name=intents.json`), but v0.2 works within
the existing single-file-per-stage constraint.

## 18.5 Retrieval Rules

- void-control MUST wait for the terminal event before fetching — no
  speculative reads.
- Retrieval has a timeout (default 30s). If the artifact is not
  available within the timeout, it is treated as missing.
- Artifacts are fetched once and cached in control-plane state by
  reference (not copied in full).
- Artifact content is NOT stored in control-plane events — only
  references and parsed metrics.

## 18.6 Future: Pull With Manifest

In a future version, the terminal event may include an artifact
manifest:

```json
{
  "event_type": "RunCompleted",
  "payload": {
    "artifact_manifest": [
      { "name": "result.json", "size_bytes": 1240, "stage": "main" },
      { "name": "intents.json", "size_bytes": 580, "stage": "main" }
    ]
  }
}
```

This lets void-control decide what to fetch without blind requests.
Requires a void-box enhancement — deferred.

---

# 19. Iteration State Threading

## 19.1 Execution Accumulator

The control loop's `reduce()` produces an `ExecutionAccumulator` that
carries forward between iterations.

```json
{
  "best_result": {
    "candidate_id": "cand_b",
    "iteration": 2,
    "score": 0.91,
    "metrics": { "latency_p99_ms": 98, "cost_usd": 0.02 },
    "artifact_ref": "exec_123/iter_2/cand_b/result.json"
  },
  "scoring_history": [
    {
      "iteration": 1,
      "candidates": [
        { "candidate_id": "cand_a", "score": 0.72, "pass": true },
        { "candidate_id": "cand_b", "score": 0.65, "pass": false }
      ],
      "best_candidate_id": "cand_a"
    }
  ],
  "message_backlog": [],
  "budget_consumed": {
    "iterations": 2,
    "child_runs": 6,
    "wall_clock_secs": 340,
    "cost_usd": 0.15
  },
  "iterations_without_improvement": 0,
  "failure_counts": {
    "total_candidate_failures": 3,
    "iteration_retries_used": 0
  }
}
```

## 19.2 Accumulator Fields

| Field | Purpose | Consumers |
|-------|---------|-----------|
| `best_result` | Global best across all iterations | `should_stop()` (convergence check), final execution result |
| `scoring_history` | Per-iteration scores for all candidates | `plan_candidates()` (inform variation strategy), UI |
| `message_backlog` | Undelivered candidate communication intents | `materialize_inboxes()` |
| `budget_consumed` | Running totals against policy limits | `should_stop()` (budget check) |
| `iterations_without_improvement` | Counter reset when `best_result` improves | `should_stop()` (plateau convergence) |
| `failure_counts` | Running totals of candidate failures and iteration retries | Failure decision tree, UI |

## 19.3 Accumulator Rules

- The accumulator is the only cross-iteration state — no side channels.
- The accumulator is persisted after each iteration completes (crash
  recovery).
- `best_result` updates when a new candidate is ranked higher than the
  current best using the full ranking function (score comparison first,
  then tie-breaking). A candidate that ties on score but wins on the
  tie-breaking metric (e.g., `lowest_cost`) does update `best_result`.
- `scoring_history` is append-only.
- `budget_consumed` is derived from events but persisted for fast
  access.
- The accumulator is reconstructible from the control-plane event log
  (it is a projection, not the source of truth).

---

# 20. Iteration Strategy Trait

## 20.1 Trait Definition

Each execution mode implements the `IterationStrategy` trait.

```rust
trait IterationStrategy {
    /// Produce inbox snapshots for candidates in the next iteration.
    fn materialize_inboxes(
        &self,
        accumulator: &ExecutionAccumulator,
    ) -> Vec<CandidateInbox>;

    /// Decide which candidates to launch in the next iteration.
    fn plan_candidates(
        &self,
        accumulator: &ExecutionAccumulator,
        inboxes: &[CandidateInbox],
    ) -> Vec<CandidateSpec>;

    /// Score completed candidates and rank them.
    fn evaluate(
        &self,
        accumulator: &ExecutionAccumulator,
        outputs: &[CandidateOutput],
    ) -> IterationEvaluation;

    /// Decide whether to stop iterating.
    fn should_stop(
        &self,
        accumulator: &ExecutionAccumulator,
        evaluation: &IterationEvaluation,
    ) -> Option<StopReason>;

    /// Produce the next accumulator state.
    fn reduce(
        &self,
        accumulator: ExecutionAccumulator,
        evaluation: IterationEvaluation,
    ) -> ExecutionAccumulator;
}
```

## 20.2 Design Rules

- Trait methods are pure functions of their inputs. No side effects, no
  I/O. This keeps them testable and replayable.
- `dispatch_candidates()` and `collect_outputs()` are shared
  infrastructure — not part of the trait. They handle void-box
  interaction, concurrency, artifact retrieval, and failure handling.
- Each mode registers a strategy at startup. Unknown modes are rejected
  at `ExecutionSpec` validation time.

## 20.3 Mode Implementations

v0.2 ships with `SwarmStrategy`. `SearchStrategy` and
`TournamentStrategy` are named but not implemented.

Mode-specific behavior lives in the trait, not in the loop:

- `SwarmStrategy.plan_candidates()` uses `leader_directed` or
  `parameter_space` variation.
- A future `TournamentStrategy.plan_candidates()` would pair candidates
  for head-to-head comparison.
- `SearchStrategy.plan_candidates()` would use scoring history to narrow
  a parameter space.

---

# 21. Backpressure and Concurrency

## 21.1 Two-Level Concurrency Model

### Global Pool

Configured at void-control startup (config file or CLI flag).

```json
{
  "global": {
    "max_concurrent_child_runs": 20
  }
}
```

Shared across all active executions. Acts as an admission gate —
candidates queue until a slot opens.

### Per-Execution Limit

Lives in `policy.concurrency`:

```json
{
  "concurrency": {
    "max_concurrent_candidates": 4
  }
}
```

Cannot exceed the global pool size. Validated at submission time.

## 21.2 Scheduling Model

```
CandidateSpec created by plan_candidates()
    -> enters execution-local queue
    -> waits for execution concurrency slot (max_concurrent_candidates)
    -> waits for global concurrency slot (max_concurrent_child_runs)
    -> dispatched to void-box
```

## 21.3 Scheduling Rules

- Within an execution, candidates are dispatched in the order
  `plan_candidates()` returns them.
- Across executions, scheduling is FIFO by candidate creation time (no
  execution priority in v0.2).
- When a child run completes, the slot is released immediately. The
  next queued candidate is dispatched.
- If an execution is paused (checkpointed), its queued candidates remain
  queued but are not dispatched. Its slots are released back to the
  global pool.
- Budget checks happen before queuing, not at dispatch time. A candidate
  is never queued if the budget is already exhausted.

## 21.4 Queue Observability

- `CandidateQueued` event emitted when a candidate enters the queue.
- `CandidateDispatched` event emitted when it gets a slot.
- Queue depth and wait time are available via execution inspection.

---

# 22. Observability Events

## 22.1 Operational Events

These extend the control-plane event model from Section 6.

| Event | Trigger | Payload |
|-------|---------|---------|
| `CandidateQueued` | Candidate enters concurrency queue | `candidate_id`, `queue_position`, `execution_id` |
| `CandidateDispatched` | Candidate gets a concurrency slot | `candidate_id`, `child_run_id`, `queue_wait_ms` |
| `CandidateOutputCollected` | Structured artifact successfully retrieved | `candidate_id`, `artifact_name`, `size_bytes` |
| `CandidateOutputError` | Artifact missing, malformed, or retrieval timeout | `candidate_id`, `artifact_name`, `error`, `policy_action` |
| `IterationBudgetWarning` | Consumed budget crosses 80% of any limit | `limit_name`, `consumed`, `max`, `percent` |
| `ExecutionBudgetExhausted` | Any budget limit hit | `limit_name`, `consumed`, `max`, `stop_reason` |
| `CandidateTimeout` | Child run exceeded expected duration | `candidate_id`, `child_run_id`, `elapsed_secs`, `timeout_secs` |
| `ExecutionStalled` | No progress for configurable duration | `last_progress_at`, `stall_duration_secs` |
| `ExecutionPaused` | User-initiated checkpoint | `iteration`, `reason` |
| `ExecutionResumed` | Execution resumed from checkpoint | `iteration`, `paused_duration_secs` |
| `PolicyUpdated` | Mid-execution policy adjustment | `changed_fields`, `old_values`, `new_values` |

## 22.2 Stall Detection

void-control tracks `last_progress_at`, updated when any of:
- a candidate completes (success or failure),
- an iteration completes,
- a new candidate is dispatched.

If `now - last_progress_at` exceeds `stall_detection_secs`, emit
`ExecutionStalled`. This is informational — it does not stop the
execution.

`stall_detection_secs` is configured in void-control's global config
(not per-execution). Default: 300.

## 22.3 Event Rules

- All new events follow the existing `EventEnvelope` schema
  (`event_id`, `event_type`, `timestamp`, `seq`, `payload`).
- Warning and operational events do not advance execution state — they
  are side-channel observability.
- All events are persisted in the control-plane event log and
  participate in replay.

---

# 23. Execution Checkpointing

## 23.1 Pause Flow

```
User sends: POST /v1/executions/{id}/pause
  -> void-control sets execution.pending_pause = true
  -> no new candidates are dispatched from the queue
     (including candidates for the current iteration awaiting
     concurrency slots)
  -> already in-flight candidates (child run started) run to completion
  -> after all in-flight candidates finish, execution transitions
     to status: "paused"
  -> queued but not-yet-dispatched candidates remain queued
  -> global concurrency slots released
  -> ExecutionPaused event emitted
  -> accumulator persisted (already happens at iteration boundary)
```

Note: if some candidates from the current iteration were queued but not
dispatched when pause was requested, the iteration is considered
incomplete. On resume, those candidates will be dispatched and the
iteration will complete before advancing to the next one.

## 23.2 Resume Flow

```
User sends: POST /v1/executions/{id}/resume
  -> execution transitions from "paused" to "running"
  -> ExecutionResumed event emitted
  -> control loop picks up from next iteration
  -> queued candidates become dispatchable again
  -> global concurrency slots re-acquired
```

## 23.3 Execution Status

The execution status set includes `Paused`:

`Pending -> Running -> {Paused -> Running} -> {Completed | Failed | Canceled}`

## 23.4 Checkpointing Rules

- Pause is only valid when status is `Running`.
- Resume is only valid when status is `Paused`.
- Cancel is valid in both `Running` and `Paused` states. Cancel from
  paused skips to terminal immediately.
- Budget wall-clock timer is paused while the execution is paused
  (paused time does not count toward `max_wall_clock_secs`).
- Paused executions still hold their candidate queue — they do not
  lose their place.
- If void-control restarts while an execution is paused, it remains
  paused after reconciliation.

---

# 24. Mid-Execution Policy Adjustment

## 24.1 Mutable Fields

| Field | Effect |
|-------|--------|
| `policy.budget.max_iterations` | New limit checked at next `should_stop()` |
| `policy.budget.max_child_runs` | New limit checked before next candidate queuing |
| `policy.budget.max_wall_clock_secs` | New limit checked at next `should_stop()` |
| `policy.budget.max_cost_usd` | New limit checked at next `should_stop()` |
| `policy.concurrency.max_concurrent_candidates` | Takes effect immediately. May release queued candidates or stop dispatching. |
| `policy.failure.max_candidate_failures_per_iteration` | New limit applied to future iterations. Does not affect the current in-progress iteration. |

## 24.2 Immutable Fields

| Field | Reason |
|-------|--------|
| `policy.convergence.*` | Changing scoring criteria mid-execution makes iteration history inconsistent. |
| `policy.failure.iteration_failure_policy` | Changing what happens when all candidates fail mid-execution makes failure handling inconsistent across iterations. |
| `policy.failure.missing_output_policy` | Changing how missing outputs are classified mid-execution makes scoring history inconsistent. |
| `evaluation.*` | Scoring function must be stable across iterations. |
| `variation.source` | Changing variation strategy breaks the relationship between iteration history and candidate planning. |

## 24.3 API

```
PATCH /v1/executions/{id}/policy
Body: {
  "budget": { "max_iterations": 15 },
  "concurrency": { "max_concurrent_candidates": 6 }
}
```

## 24.4 Adjustment Rules

- Only valid when execution status is `Running` or `Paused`.
- Attempting to mutate an immutable field returns a validation error
  (no partial apply).
- New budget limits must be >= current consumed values. Cannot set
  `max_iterations` to 3 if 5 iterations have already completed.
- Changes emit a `PolicyUpdated` event with old and new values.
- Changes are reflected in the persisted execution state immediately.
- Concurrency increases may cause queued candidates to dispatch
  immediately.

---

# 25. Execution Dry-Run Mode

## 25.1 API

```
POST /v1/executions/dry-run
Body: { <full ExecutionSpec> }
```

## 25.2 Response

```json
{
  "valid": true,
  "mode": "swarm",
  "plan": {
    "candidates_per_iteration": 3,
    "max_iterations": 10,
    "max_child_runs": 30,
    "estimated_concurrent_peak": 3,
    "variation_source": "parameter_space",
    "parameter_space_size": 27
  },
  "warnings": [
    "max_cost_usd not set — execution has no cost cap"
  ],
  "errors": []
}
```

## 25.3 Validation

Dry-run validates:
- `ExecutionSpec` schema and required fields.
- Policy validation (non-zero limits, consistent constraints).
- Mode-specific section present and valid (e.g., `swarm` section for
  mode `swarm`).
- Evaluation config is well-formed (valid scoring type, weights sum
  correctly, tie-breaking field exists).
- Workflow template parses as a valid void-box `RunSpec`.
- Concurrency fits within global pool (warning if
  `max_concurrent_candidates` > 50% of global pool).

## 25.4 Computed Plan

Dry-run computes:
- Maximum possible child runs
  (`candidates_per_iteration * max_iterations`).
- Peak concurrency.
- Parameter space cardinality (if `parameter_space` variation).

## 25.5 Dry-Run Rules

- Dry-run does not create an `Execution`, emit events, or contact
  void-box.
- Dry-run is idempotent and side-effect free.
- Warnings are informational — they do not make `valid: false`.
- Errors make `valid: false` — the spec would be rejected by
  `POST /v1/executions`.

---

# 26. Result Provenance

## 26.1 Provenance in Execution Result

When an execution completes, its `result` field includes full
provenance:

```json
{
  "execution_id": "exec_123",
  "status": "completed",
  "stop_reason": "convergence_threshold",
  "result": {
    "candidate_id": "cand_f",
    "iteration": 4,
    "child_run_id": "run-1700004000",
    "score": 0.93,
    "metrics": {
      "latency_p99_ms": 87,
      "cost_usd": 0.018
    },
    "artifact_refs": [
      {
        "name": "result.json",
        "stage": "main",
        "retrieval": "GET /v1/runs/run-1700004000/stages/main/output-file"
      }
    ],
    "variation": {
      "overrides": {
        "sandbox.env.CONCURRENCY": "2",
        "sandbox.memory_mb": 1024
      }
    }
  }
}
```

## 26.2 What Provenance Answers

| Question | Field |
|----------|-------|
| Which candidate produced the best result? | `candidate_id` |
| Which iteration? | `iteration` |
| What void-box run backs it? | `child_run_id` (drill down to logs, events, stages) |
| How did it score? | `score` + `metrics` |
| Where are the artifacts? | `artifact_refs` with retrieval paths |
| What made this candidate different? | `variation.overrides` |

## 26.3 Provenance Rules

- Provenance is set when `best_result` is finalized at execution
  completion.
- If the execution fails with no successful candidates, `result` is
  `null` and `stop_reason` explains why.
- Provenance is a snapshot — it references but does not duplicate
  artifacts.
- The `child_run_id` is the authoritative link for deep inspection
  (logs, stage graph, telemetry).

---

# 27. Acceptance Criteria

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
8. Validate an `ExecutionSpec` via dry-run without creating an execution.
9. Score candidates using a deterministic scoring function configured in
   the `ExecutionSpec`.
10. Generate candidate variation from a parameter space, explicit list,
    or leader-directed proposals.
11. Enforce budget limits and stop execution when any limit is exhausted.
12. Manage concurrency across executions via a global pool and
    per-execution limits.
13. Pause and resume an execution, with in-flight candidates completing
    before the pause takes effect.
14. Adjust budget and concurrency policy on a running execution.
15. Emit operational observability events for queue depth, stalls,
    budget warnings, and policy changes.
16. Trace the final execution result back to its originating candidate,
    iteration, child run, and variation overrides.
