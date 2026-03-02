# Void Control Plane <-> Void-Box Runtime Specification

## Version: v0.2 (Draft)

## Scope: Single-host first, distributed-ready

This version preserves v0.1 boundaries and adds concrete contracts for
state, events, and error behavior.

---

# 1. Architectural Boundaries

## 1.1 Layer Separation

### Void-Box (Runtime)

- Executes one `Run` (workflow DAG) per accepted request.
- Schedules internal `Stage` execution and `fan_out`/`join`.
- Spawns one microVM per stage execution unit.
- Emits ordered runtime events for each run.

### Void-Controller (Control Plane)

- Owns desired/observed run lifecycle state.
- Enforces global host policies and admission control.
- Persists run metadata and durable event history.
- Reconciles after restarts and streams logs/events to clients.

Controller MUST NOT orchestrate stages.
Runtime MUST NOT persist cluster-wide lifecycle state.

---

# 2. Core Model

## 2.1 Identifiers

- `run_id`: globally unique ID for one workflow execution.
- `attempt_id`: monotonic integer starting at `1` per run.
- `stage_id`: stable ID from workflow spec.
- `event_id`: unique ID per emitted event.

## 2.2 Run States

`Pending -> Starting -> Running -> {Succeeded | Failed | Canceled}`

- Terminal states are immutable.
- `Failed` means runtime error or stage failure.
- `Canceled` means user/system-initiated stop.

## 2.3 Attempt Semantics

- Every restart creates a new `attempt_id`.
- Only one active attempt per run at a time.
- Events and logs MUST include `attempt_id`.

---

# 3. Runtime Contract (Run-Level API)

The controller interacts with runtime using run-level calls only.

## 3.1 `start(run_id, workflow_spec, policy) -> StartResult`

Idempotency:
- If run is already active, return existing `handle` and current state.
- If run is terminal, return `ALREADY_TERMINAL`.

`StartResult`:
- `handle: string`
- `attempt_id: integer`
- `state: "Starting" | "Running"`

## 3.2 `stop(handle, reason) -> StopResult`

- Must be idempotent.
- If already terminal, return success with terminal state.

`StopResult`:
- `state: "Canceled" | "Succeeded" | "Failed"`
- `terminal_event_id: string`

## 3.3 `inspect(handle) -> RuntimeInspection`

`RuntimeInspection`:
- `run_id`, `attempt_id`, `state`
- `active_stage_count`
- `active_microvm_count`
- `started_at`, `updated_at`
- `terminal_reason?`, `exit_code?`

## 3.4 `subscribe_events(handle, from_event_id?) -> EventStream`

- Delivers ordered events for a single run.
- Supports resume from `from_event_id`.
- At-least-once delivery; duplicates are allowed and must preserve
  `event_id`.

---

# 4. Event Contract

## 4.1 Event Envelope (Required)

```json
{
  "event_id": "evt_123",
  "event_type": "RunStarted",
  "run_id": "run_123",
  "attempt_id": 1,
  "timestamp": "2026-02-28T19:00:00Z",
  "seq": 42,
  "payload": {}
}
```

Required fields:
- `event_id`: unique, stable.
- `seq`: strictly increasing per `run_id` + `attempt_id`.
- `timestamp`: RFC3339 UTC.

## 4.2 Standard Event Types

- `RunStarted`
- `StageStarted`
- `StageCompleted`
- `StageFailed`
- `MicroVmSpawned`
- `MicroVmExited`
- `RunCompleted`
- `RunFailed`
- `RunCanceled`

`RunCompleted`, `RunFailed`, and `RunCanceled` are terminal events.

## 4.3 Ordering Rules

- Runtime MUST emit events in causal order per run attempt.
- Controller MUST treat `seq` as source of truth for ordering.
- Missing sequence numbers during streaming MUST trigger re-sync via
  `inspect` + resumed `subscribe_events`.

---

# 5. Policy Contract

Controller passes policy hints with `start`:

```json
{
  "max_parallel_microvms_per_run": 8,
  "max_stage_retries": 1,
  "stage_timeout_secs": 900,
  "cancel_grace_period_secs": 20
}
```

Rules:
- Runtime MUST enforce provided limits.
- Runtime MUST reject invalid or unsupported policy fields.
- No silent degradation and no policy bypass.

---

# 6. Error Model

## 6.1 Error Codes

- `INVALID_SPEC`
- `INVALID_POLICY`
- `NOT_FOUND`
- `ALREADY_TERMINAL`
- `RESOURCE_LIMIT_EXCEEDED`
- `INTERNAL_ERROR`

## 6.2 Error Response Shape

```json
{
  "code": "INVALID_POLICY",
  "message": "max_parallel_microvms_per_run must be > 0",
  "retryable": false
}
```

---

# 7. Cancellation & Reconciliation

## 7.1 Cancellation Flow

Controller:
- Sets desired state to stopped.
- Calls `stop(handle, reason)`.

Runtime:
- Terminates active microVMs (graceful, then forced after timeout).
- Emits one terminal event (`RunCanceled` unless already terminal).

## 7.2 Reconciliation After Controller Restart

Controller must:
- Reload non-terminal runs.
- Call `inspect` for each known handle.
- Resume stream via `subscribe_events(from_event_id=last_seen)`.
- Mark unknown/missing handles as orphaned and emit a controller-side
  reconciliation event.

Runtime `inspect` and `stop` must be idempotent.

---

# 8. Strict Boundary Rules (Normative)

Controller MUST NOT:
- Spawn stage-level microVMs.
- Interpret workflow DAG for execution.
- Reimplement runtime `fan_out` scheduling.

Runtime MUST NOT:
- Persist cluster-wide desired state.
- Perform cross-run global scheduling decisions.
- Manage distributed coordination between nodes.

---

# 9. Mental Model

`Run` = atomic orchestration unit (control plane scope)

`Stage` = atomic isolation unit (runtime scope)

`microVM` = execution isolation boundary

Controller orchestrates runs.
Runtime orchestrates stages.
