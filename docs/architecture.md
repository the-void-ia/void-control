# Architecture

## Overview

`void-control` is the control-plane layer for `void-box` execution. It consumes
runtime data from `void-box`, normalizes it into stable contract types, plans
and tracks orchestration iterations, persists execution state, and exposes both
operator-facing and programmatic interfaces.

At a high level:

```text
void-control = contract normalization + orchestration strategies + persistence + bridge/UI
```

## System boundary

`void-control` does not launch or isolate workloads itself. That responsibility
belongs to `void-box`. `void-control` assumes a runtime provider that can:

- start a run
- inspect a run
- return structured output or a typed failure

The default live provider is `VoidBoxRuntimeClient`. Tests use `MockRuntime`.

`void-control` does not interpret itself as a runtime. It is the orchestration
control plane that sits above runtime execution.

That distinction matters for first release:

- runtime execution belongs to `void-box`
- orchestration strategy belongs to `void-control`
- graph/UI inspection belongs to `void-control`
- collaboration transport and event persistence belong to `void-control`

## Component diagram

```text
┌─────────────────────────────────────────────────────────────────┐
│ Operator / CLI / UI                                            │
│                                                                 │
│  ┌──────────────────────────────┐   ┌─────────────────────────┐ │
│  │ web/void-control-ux          │   │ voidctl / bridge        │ │
│  │ graph + inspector + launch   │   │ launch, dry-run, query  │ │
│  └───────────────┬──────────────┘   └────────────┬────────────┘ │
│                  │                               │              │
└──────────────────┼───────────────────────────────┼──────────────┘
                   │                               │
                   ▼                               ▼
         ┌───────────────────────────────────────────────────┐
         │ Orchestration Service                             │
         │                                                   │
         │  - validate execution spec                        │
         │  - plan candidates                                │
         │  - route communication intents                    │
         │  - dispatch runtime work                          │
         │  - collect artifacts and reduce iterations        │
         │  - persist execution/event/candidate state        │
         └───────────────┬───────────────────────┬───────────┘
                         │                       │
                         ▼                       ▼
              ┌───────────────────┐   ┌──────────────────────┐
              │ Store / Replay    │   │ Planning Strategies  │
              │ fs-backed data    │   │ swarm / search       │
              │ events / inboxes  │   │ variation sources    │
              └─────────┬─────────┘   └──────────┬───────────┘
                        │                        │
                        └────────────┬───────────┘
                                     ▼
                           ┌───────────────────┐
                           │ Runtime Adapter   │
                           │ mock / void-box   │
                           └─────────┬─────────┘
                                     ▼
                               `void-box`
```

## Main components

### Contract layer

`src/contract/` defines the stable types and normalization logic used to map
raw runtime payloads into `void-control`'s contract model.

Responsibilities:

- map daemon status/event values into stable enums
- reject malformed or incompatible payloads
- preserve diagnostics for compatibility analysis

Key files:

- `src/contract/api.rs`
- `src/contract/compat.rs`
- `src/contract/compat_json.rs`
- `src/contract/event.rs`
- `src/contract/state.rs`

### Runtime adapter layer

`src/runtime/` abstracts over the execution provider.

Responsibilities:

- define the runtime interface used by orchestration
- provide the mock runtime used by tests
- provide the serde-gated `void-box` client used for live integrations
- inject launch context such as inbox snapshots into provider requests

Key files:

- `src/runtime/mod.rs`
- `src/runtime/mock.rs`
- `src/runtime/void_box.rs`

### Orchestration core

`src/orchestration/service.rs` coordinates the execution lifecycle.

Responsibilities:

- create and validate execution records
- plan iteration candidates from a chosen strategy and variation source
- persist queued candidates
- start candidate runs through the runtime adapter
- collect structured output and failure outcomes
- reduce iteration results into accumulator and execution state
- emit control-plane events for replay and UX

Supporting modules:

- `src/orchestration/spec.rs`: execution spec schema/validation
- `src/orchestration/variation.rs`: candidate source generation
- `src/orchestration/strategy.rs`: swarm/search planning and stopping
- `src/orchestration/scoring.rs`: weighted scoring and ranking
- `src/orchestration/policy.rs`: budgets, concurrency, convergence policies
- `src/orchestration/events.rs`: persisted control-plane event model
- `src/orchestration/scheduler.rs`: global dispatch fairness and queueing
- `src/orchestration/reconcile.rs`: restart/reload handling

## Orchestration strategy model

`void-control` should be read as a strategy host.

Implemented or active:

- `swarm`: breadth-oriented sibling candidate exploration with reduction
- `search`: incumbent-centered refinement using the same execution primitives

Planned next:

- `supervision`: a higher-level orchestration strategy that uses the same
  execution, event, message-box, and UI primitives while changing the planner
  semantics

Strategy responsibilities:

- decide how candidates are proposed
- decide how iteration outcomes are reduced
- interpret advisory collaboration state
- stop, continue, or converge under policy

Shared strategy substrate:

- `ExecutionSpec`
- persisted execution and candidate records
- control-plane event log
- message-box / MCP-backed collaboration transport
- bridge APIs and graph-first inspection UI

### Message box, MCP, and signal-reactive planning

The message-box model gives candidates a structured communication channel across
iterations.

Responsibilities:

- persist `CommunicationIntent` records
- route intents into `RoutedMessage` records
- build per-candidate inbox snapshots
- derive `MessageStats` for planning iteration `N`

In practice this is the collaboration/event layer used by orchestration-aware
agents and tools:

- runtime workloads can emit collaboration intents
- MCP-backed tools can expose or consume the same execution context primitives
- the control plane persists those intents as first-class orchestration records
- strategies consume the routed/persisted view, not arbitrary terminal text

Current signal-reactive behavior is metadata-driven:

- planner reacts to routed-message counts and delivery outcomes
- planner does not inspect free-form payload text for candidate construction
- legacy `leader_directed` remains distinct from `signal_reactive`

Key files:

- `src/orchestration/message_box.rs`
- `src/orchestration/types.rs`
- `src/orchestration/variation.rs`
- `src/orchestration/strategy.rs`

## Event architecture

The system has two event planes that must stay distinct.

### 1. Runtime event plane

Produced by `void-box` and normalized by `void-control`.

Examples:

- run lifecycle events
- stage lifecycle events
- telemetry and artifact readiness
- terminal runtime failures

Properties:

- runtime truth source is `void-box`
- `void-control` normalizes these into stable contract types
- runtime events describe what happened inside a specific run

### 2. Control-plane orchestration event plane

Produced by `void-control`.

Examples:

- `ExecutionCreated`
- `ExecutionSubmitted`
- `CandidateQueued`
- `CandidateDispatched`
- `CandidateOutputCollected`
- `CandidateScored`
- `IterationCompleted`
- `ExecutionCompleted`
- communication/message-box persistence and routing outcomes

Properties:

- truth source is the orchestration service and store
- these events describe how the control plane planned, routed, reduced, and
  persisted the execution
- they remain stable even if runtime transport/provider details change

### MCP and collaboration events

MCP is not a replacement event plane. It is a collaboration/tooling surface
that can participate in orchestration through the same persisted control-plane
primitives.

Flow:

```text
┌──────────────────────────┐
│ candidate run / MCP tool │
└─────────────┬────────────┘
              v
┌──────────────────────────┐
│ CommunicationIntent      │
│ audience / kind / body   │
└─────────────┬────────────┘
              v
┌──────────────────────────┐
│ message-box validation   │
│ schema / limits / dedup  │
└─────────────┬────────────┘
              v
┌──────────────────────────┐
│ RoutedMessage            │
└───────┬───────────┬──────┘
        │           │
        v           v
┌──────────────┐  ┌──────────────────┐
│ leader inbox │  │ broadcast fanout │
│ snapshot     │  │ future inboxes   │
└──────┬───────┘  └────────┬─────────┘
       └──────────┬────────┘
                  v
        ┌──────────────────────┐
        │ MessageStats         │
        └──────────┬───────────┘
                   v
        ┌──────────────────────┐
        │ strategy planner     │
        │ swarm now            │
        │ supervision later    │
        └──────────────────────┘
```

The architectural rule is:

- free-form tool traffic is not the canonical contract
- persisted `CommunicationIntent`, `RoutedMessage`, and inbox snapshots are
  the canonical collaboration records
- audiences such as `leader` and `broadcast` are orchestration semantics owned
  by `void-control`

This keeps collaboration inspectable in the UI and reusable across future
strategies such as `supervision`.

### Persistence and replay

The filesystem-backed store persists enough state to reconstruct active
executions and replay control-plane history.

Responsibilities:

- execution metadata and snapshots
- queued and terminal candidate records
- control-plane events
- communication intents and routed messages
- inbox snapshots for provider launch injection

Key files:

- `src/orchestration/store.rs`
- `src/orchestration/store/fs.rs`

## Core flows

### 1. Execution submission

```text
ExecutionSpec
  -> validation
  -> execution record + accumulator persisted
  -> initial planning request
  -> queued candidate records
```

Entry points:

- CLI / bridge route
- test harness helpers

### 2. Iteration planning

```text
execution + accumulator + prior results + message stats
  -> strategy.plan_candidates(...)
  -> variation source selection
  -> candidate specs persisted as queued
```

Planning inputs depend on strategy:

- swarm: breadth-oriented candidate planning
- search: incumbent-centered neighborhood refinement
- supervision: planned next strategy on the same execution substrate

### 3. Candidate dispatch

```text
queued candidate
  -> scheduler grant
  -> inbox snapshot resolution
  -> runtime.start_run(...)
  -> candidate marked running
```

For serde-enabled live flows, launch injection can embed the inbox snapshot into
the runtime request.

### 4. Artifact collection and reduction

```text
runtime inspection / terminal result
  -> structured output collection
  -> candidate terminal record
  -> iteration evaluation set
  -> strategy.reduce(...)
  -> accumulator + execution status update
```

Reduction decides whether to:

- continue with another iteration
- stop due to threshold/plateau/exhaustion
- mark execution failed when policy requires it

### 5. Signal-reactive planning path

```text
CommunicationIntent[]
  -> RoutedMessage[]
  -> inbox snapshots for delivery iteration N
  -> extract_message_stats(...)
  -> advisory strategy bias for iteration N
```

The planner uses the stats as advisory metadata. It does not treat message
payloads as direct candidate-authoring commands.

## UI and API implications

The operator UI and bridge APIs should expose one execution model regardless of
strategy.

That means:

- one launcher for runtime specs and orchestration specs
- one graph-first execution surface
- one persisted execution/event model
- strategy-specific semantics rendered through the same shell

For first release, `swarm` is the concrete orchestration strategy that proves
this model. `supervision` should extend the same primitives, not introduce a
parallel stack.

## Persistence and replay model

The event log and persisted execution state must support restart and partial
reconstruction:

- active executions are reloaded by reconciliation
- queued candidates are restored without duplication
- control events remain the replay spine for execution history
- message-box artifacts remain separate persisted data, not ad hoc in-memory state

This separation matters because planning, dispatch, and operator views all
depend on deterministic persisted state rather than transient worker memory.

## Source file map

### Operator and bridge

- `src/bin/voidctl.rs`
- `src/bridge.rs`
- `web/void-control-ux/`

### Contract and runtime

- `src/contract/`
- `src/runtime/`

### Orchestration

- `src/orchestration/service.rs`
- `src/orchestration/spec.rs`
- `src/orchestration/strategy.rs`
- `src/orchestration/variation.rs`
- `src/orchestration/message_box.rs`
- `src/orchestration/store/fs.rs`
- `src/orchestration/scheduler.rs`
- `src/orchestration/reconcile.rs`

## Related documents

- `README.md`
- `AGENTS.md`
- `spec/`
- `docs/release-process.md`
