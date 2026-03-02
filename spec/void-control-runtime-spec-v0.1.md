# Void Control Plane ↔ Void-Box Runtime Specification

## Version: v0.1

## Scope: Single-host first, distributed-ready

------------------------------------------------------------------------

# 1. Architectural Principles

## 1.1 Clear Layer Separation

### Void-Box (Runtime)

-   Executes a Run.
-   Manages workflow graph.
-   Spawns microVM per stage.
-   Handles fan_out / join.
-   Enforces skill isolation & policies.
-   Produces structured internal events.

### Void-Controller (Control Plane)

-   Orchestrates Runs.
-   Persists desired/observed state.
-   Reconciles lifecycle.
-   Streams logs/events.
-   Enforces global concurrency limits.
-   Handles restart/remove semantics.

⚠️ Controller does NOT orchestrate stages.\
⚠️ Runtime does NOT persist cluster-wide lifecycle state.

------------------------------------------------------------------------

# 2. Execution Model

## 2.1 Run

A Run represents one full workflow execution.

Run may internally produce: - N sequential microVMs - M parallel
microVMs via fan_out

This is internal to Void-Box.

## 2.2 Attempt

Each restart creates a new attempt linked to the Run.

------------------------------------------------------------------------

# 3. Runtime Contract (Void-Box as Executor)

The Controller interacts only with Run-level operations.

Required interface:

-   start(run_id, spec, policy) → RunHandle
-   stop(handle)
-   inspect(handle) → RuntimeInspection
-   subscribe_events(handle) → EventStream

------------------------------------------------------------------------

# 4. Void-Box Responsibilities

For a given Run:

-   Parse workflow
-   Execute DAG
-   Spawn microVM per stage
-   Manage fan_out parallelism internally
-   Propagate stage failures
-   Aggregate final exit result
-   Emit structured events

Example internal flow:

Stage A → microVM #1\
fan_out:\
• Stage B1 → microVM #2\
• Stage B2 → microVM #3\
• Stage B3 → microVM #4\
join\
Stage C → microVM #5

Controller sees only Run state transitions.

------------------------------------------------------------------------

# 5. Event Mapping

Runtime emits structured events such as:

-   RunStarted
-   StageStarted
-   StageCompleted
-   StageFailed
-   MicroVmSpawned
-   MicroVmExited
-   RunCompleted
-   RunFailed

Controller maps these into durable RunEvents.

------------------------------------------------------------------------

# 6. Cancellation Semantics

On cancel:

Controller: - sets desired_state = Stopped - calls runtime.stop()

Runtime: - terminates active microVM(s) - aborts workflow - cleans
resources - emits terminal event

------------------------------------------------------------------------

# 7. Concurrency & Host Protection

Controller owns host-level limits:

-   max_active_runs
-   max_total_microvms
-   max_parallel_microvms_per_run

Runtime receives execution policy hints and must respect them.

No silent degradation. No policy bypass.

------------------------------------------------------------------------

# 8. Reconciliation Contract

On Controller restart:

-   Reload active runs
-   Inspect runtime handles
-   Mark orphaned runs explicitly
-   Resume tracking active executions

Runtime must support idempotent inspection.

------------------------------------------------------------------------

# 9. Log & Telemetry Ownership

Runtime: - Produces structured logs and stage events.

Controller: - Streams logs to API clients. - Persists metadata and
references. - Exposes metrics.

------------------------------------------------------------------------

# 10. Single-Host vs Distributed

Single-host:

Controller → Local Runtime

Future Distributed:

Control Plane\
→ Node A (Void-Box runtime)\
→ Node B (Void-Box runtime)\
→ Node C (Void-Box runtime)

No changes required to workflow model.

------------------------------------------------------------------------

# 11. Strict Boundary Rules

Controller MUST NOT: - Spawn stage-level microVMs - Interpret workflow
DAG - Reimplement fan_out logic

Runtime MUST NOT: - Persist cluster-wide desired state - Schedule across
runs - Manage distributed coordination

------------------------------------------------------------------------

# 12. Core Mental Model

Run = Atomic orchestration unit\
Stage = Atomic isolation unit\
microVM = Isolation boundary

Controller orchestrates Runs.\
Runtime orchestrates Stages.
