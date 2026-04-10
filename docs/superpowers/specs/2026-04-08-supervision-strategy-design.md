# Void-Control Supervision Strategy Design

Date: 2026-04-08

## Goal

Define the first `supervision` orchestration strategy for `void-control`.

`supervision` should implement a classic orchestrator-worker pattern:

- one supervisor coordinates the execution
- specialized workers perform independent subtasks
- workers do not communicate with each other
- the supervisor owns routing, review, retry, and finalization

The strategy must reuse the existing `void-control` execution substrate:

- execution persistence
- control-plane event log
- bridge APIs and CLI
- graph-first UI shell
- runtime dispatch through `void-box`

## Problem

`void-control` already supports:

- `swarm`: sibling candidate exploration with score-based reduction
- wrapped runtime executions such as pipelines, agents, and workloads

What is missing is a centralized orchestration strategy for tasks that are best
modeled as:

- decompose
- assign
- review
- revise
- finalize

This is a different coordination model from `swarm`.

`swarm` is breadth-oriented and convergence/score driven.
`supervision` should be review/decision driven.

## First-Release Principle

`supervision` is a control-plane strategy in `void-control`, not a new runtime
mode in `void-box`.

That means:

- runtime execution still belongs to `void-box`
- the supervisor and workers are modeled as control-plane execution entities
- routing, review, revision, and completion are persisted as control-plane
  records
- the same bridge, CLI, and UI surfaces should work for `supervision`

## Strategy Definition

`supervision` is the orchestrator-worker pattern:

- a single supervisor receives the top-level goal
- the supervisor creates or selects worker tasks
- workers execute independently
- workers report outputs back to the supervisor
- the supervisor decides whether each output is:
  - accepted
  - revised
  - retried
  - rejected
  - finalized into the execution result

V1 is intentionally flat:

- one supervisor
- many workers
- no worker-to-worker communication
- no multi-level delegation tree

This is not full hierarchy.
Hierarchy can be added later on top of the same substrate if needed.

## Why This Fits Void-Control

`void-control` already has the central coordination model needed by a
supervisor:

- orchestration service owns execution state
- scheduler owns dispatch order
- store persists execution/candidate/event state
- message-box already supports routed communication records
- UI already renders execution graphs with a right-side inspector

So `supervision` is a natural extension of the current architecture.

It does not require a new product surface or a new runtime contract.

## Scope

### In scope

- `mode: supervision`
- one supervisor and many workers
- worker assignment and review loops
- revision and retry decisions
- explicit control-plane events for supervision
- bridge, CLI, and UI support for the new mode

### Out of scope

- multi-level hierarchical orchestration
- mesh or peer-to-peer collaboration
- worker-to-worker direct messaging
- replacing `swarm`
- changing `void-box` pipeline/runtime primitives

## Execution Model

The execution remains the top-level control-plane object.

Within a supervision execution:

- the supervisor is the controlling role
- workers are tracked as candidate-like execution units
- every worker run can be mapped to a runtime run ID
- supervisor decisions are persisted as execution events

The execution lifecycle is:

```text
ExecutionCreated
  -> SupervisorAssigned
  -> WorkerQueued
  -> WorkerDispatched
  -> WorkerOutputCollected
  -> ReviewRequested
  -> Approved | RevisionRequested | RetryRequested | Rejected
  -> ExecutionCompleted | ExecutionFailed | ExecutionCanceled
```

## Spec Model

V1 should extend the existing execution spec rather than create a separate
document family.

Minimal shape:

```yaml
mode: supervision
goal: Resolve a complex task through a central supervisor and specialized workers
workflow:
  template: examples/runtime-templates/some_worker_template.yaml
policy:
  budget:
    max_iterations: 3
    max_child_runs: 8
  concurrency:
    max_concurrent_candidates: 4
evaluation:
  scoring_type: weighted_metrics
  ranking: highest_score
  tie_breaking: latency_p99_ms
variation:
  candidates_per_iteration: 4
  source: explicit
  explicit:
    - overrides:
        sandbox.env.WORKER_ROLE: researcher
    - overrides:
        sandbox.env.WORKER_ROLE: implementer
supervision:
  supervisor_role: coordinator
  review_policy:
    max_revision_rounds: 2
    retry_on_runtime_failure: true
    require_final_approval: true
swarm: false
```

Notes:

- `workflow.template` remains the worker runtime template reference in v1
- the `supervision` block defines control-plane review semantics
- the existing `variation` section remains the source of worker roles/tasks

This keeps the spec delta small and lets `supervision` reuse the existing
planning inputs.

## Planning Semantics

`swarm` planning asks:

- what candidate set should explore this iteration?

`supervision` planning asks:

- what workers should be assigned?
- which outputs need review?
- which workers must revise or retry?
- when is the execution done?

So the supervision planner needs to:

1. create the initial worker set
2. dispatch available workers under concurrency policy
3. collect worker outputs
4. transform outputs into supervisor review requests
5. emit review decisions
6. queue revisions or retries when needed
7. finalize the execution when approval criteria are satisfied

## Reduction Semantics

This is the biggest difference from `swarm`.

`swarm` reduces by:

- scoring outputs
- ranking candidates
- updating best candidate
- applying convergence policy

`supervision` should reduce by decision state:

- approved worker outputs
- pending review count
- revision count
- retry count
- rejected worker count
- final approval or terminal failure

V1 reduction rules:

- if required outputs are approved, complete the execution
- if outputs need revision and revision budget remains, queue revised workers
- if a worker fails at runtime and retry policy allows it, retry
- if required outputs cannot be approved within budget, fail the execution

## Event Model

V1 should add explicit supervision events to the existing control-plane event
stream.

New event types:

- `SupervisorAssigned`
- `WorkerQueued`
- `ReviewRequested`
- `WorkerApproved`
- `RevisionRequested`
- `ExecutionFinalized`

Event payloads should include:

- execution ID
- worker ID
- iteration/review round
- linked runtime run ID when present
- decision reason
- revision notes when present

These events are the primary truth source for UI/CLI supervision inspection.

Current implementation note:

- v1 approval is metric-driven
- worker output must include `metrics.approved`
- the checked-in supervision example appends that field after the measured
  benchmark run

## Message And Collaboration Model

V1 supervision should use directed control-plane communication semantics, not
mesh-style peer communication.

Primary in-guest communication surface:

- `void-mcp`

Secondary/complementary in-guest communication surface:

- `void-message`

Both feed the same persisted `void-control` communication model:

- `CommunicationIntent`
- routed message records
- inbox snapshots
- message stats

So the source of truth remains the control plane, while `void-mcp` and
`void-message` are agent-facing transport surfaces into that model.

Allowed routes:

- worker -> supervisor
- supervisor -> worker

Disallowed in v1:

- worker -> worker
- ad hoc peer topology

The control-plane message/inbox layer should persist:

- worker submissions for review
- supervisor review notes
- revision instructions

This keeps the system debuggable and matches the orchestrator-worker model.

## UI Model

`supervision` should use the same UI shell already agreed for runtime and swarm:

- left execution list
- center graph
- bottom event strip
- right inspector

Graph semantics:

- one supervisor node
- worker nodes fan out from the supervisor
- revision edges return from supervisor to worker
- finalized outputs are highlighted as approved

Inspector semantics:

- selected supervisor or worker
- review state
- revision history
- runtime jump
- guest and host metrics when relevant

The graph should remain graph-first and operational, not a chat transcript.

## CLI Model

The existing non-interactive CLI is already the correct outer surface:

```text
voidctl execution submit <spec>
voidctl execution dry-run <spec>
voidctl execution watch <execution-id>
voidctl execution inspect <execution-id>
voidctl execution events <execution-id>
voidctl execution result <execution-id>
voidctl execution runtime <execution-id> [worker-id]
```

For `supervision`:

- `inspect` should show supervisor state, review counts, and worker status
- `events` should expose supervision decisions
- `result` should show approved/finalized worker outputs
- `runtime` should resolve the selected worker runtime run

No new top-level CLI family is required.

## Skill Model

The `void-control` skill should learn one more spec choice:

- choose `supervision` when the task needs a central reviewer/coordinator and
  specialized workers rather than score-based candidate competition

Decision rule:

- use `swarm` for broad exploration and competing strategies
- use `supervision` for decomposition, review, revision, and final approval
- use wrapped runtime specs for single pipelines/agents/workloads

## Data Model Impact

The existing execution and candidate substrate should be reused.

Likely extensions:

- supervision metadata on execution accumulator
- worker review state on candidate records
- revision round counters
- final approval summary

Prefer additive fields rather than parallel supervision-only persistence
structures.

## Testing Strategy

V1 should add:

1. Spec validation tests
- `mode: supervision` accepted
- supervision block validation

2. Strategy tests
- worker planning
- review/revision reduction
- retry and terminal failure behavior

3. Service integration tests
- supervision execution runs end to end on `MockRuntime`
- review decisions persist correctly
- runtime failures trigger retry or failure under policy

4. Bridge/CLI tests
- create/dry-run/inspect/result paths for supervision

5. UI build + behavior checks
- supervision graph renders
- inspector shows review state

## Migration And Compatibility

This design is additive.

- existing `swarm` behavior should remain unchanged
- wrapped runtime executions remain unchanged
- bridge and CLI gain one additional strategy mode
- UI gets one additional graph/inspector interpretation

## Recommendation

Implement `supervision` as the next strategy after `swarm`, but keep v1 flat
and centralized.

That gives `void-control`:

- one exploration strategy: `swarm`
- one orchestrator-worker strategy: `supervision`
- one wrapped runtime path for pipelines/agents/workloads

This is the right first taxonomy for the product and matches the current system
boundary with `void-box`.
