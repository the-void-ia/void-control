# Void Control UX Visualization Specification

## Version: v0.1

## Scope
Define a user-facing visualization UX for orchestration runs using:
- graph view (run/attempt/stage/microVM relationships)
- timeline view (ordered events/logs)
- operational actions (cancel/retry/resume stream)

This spec reuses the existing runtime/control contract. It does not add new
execution semantics.

---

## 1. UX Goals

1. Let users understand run state at a glance.
2. Make cause/effect visible (dependency edges, retries, failures, timeouts).
3. Support fast operator actions from the same screen.
4. Keep reconciliation and resume behavior explicit.

---

## 2. Core Information Model (UI)

- **Run**: top-level orchestration unit.
- **Attempt**: restart/retry boundary for a run.
- **Stage**: execution step in workflow DAG.
- **microVM**: isolated execution boundary per stage unit.
- **Event**: ordered envelope (`event_id`, `event_type`, `seq`, `timestamp`).

Status colors:
- `running`: blue
- `succeeded`: green
- `failed`: red
- `canceled`: gray

---

## 3. Primary Screens

## 3.1 Runs List
- Columns: run_id, state, started_at, updated_at, active_stage_count, active_microvm_count.
- Filters: `state=active|terminal`, search by run_id.
- Row action: open run detail.

## 3.2 Run Detail (Graph + Timeline)
- Left: DAG/graph panel (`Run -> Attempt -> Stage -> microVM`).
- Right: timeline panel sorted by `seq` (source of truth ordering).
- Bottom: log stream panel (stdout/stderr chunks).
- Top actions: `Cancel`, `Retry Attempt`, `Resume Stream`.

## 3.3 Reconciliation View
- Lists non-terminal and orphan-marked runs.
- Shows last seen event id and resume status.
- Action: `Reconcile now`.

---

## 4. Interaction Rules

1. Selecting graph node filters timeline/logs to node scope.
2. Timeline hover highlights related graph nodes.
3. Resume uses `from_event_id=last_seen_event_id`.
4. Duplicate events are tolerated by `event_id` dedupe in UI state.
5. Terminal event closes live stream indicator.

---

## 5. API Requirements for Frontend

Required existing endpoints:
- `GET /v1/runs?state=active|terminal`
- `GET /v1/runs/{id}`
- `GET /v1/runs/{id}/events?from_event_id=...`
- `POST /v1/runs/{id}/cancel`

Frontend expects:
- stable `event_id`
- monotonic `seq` per run+attempt
- `attempt_id` in run and event payloads
- structured error shape `{code,message,retryable}`

---

## 6. Frontend State Shape (Recommended)

```json
{
  "runsById": {},
  "attemptsByRun": {},
  "eventsByRunAttempt": {},
  "lastSeenEventIdByRun": {},
  "orphanRuns": []
}
```

Use `seq` for ordering and `event_id` for dedupe.

---

## 7. Acceptance Criteria

1. User can open a run and see graph + timeline synchronized.
2. Cancel action updates UI terminal state without page reload.
3. Stream resume after disconnect continues from last seen event id.
4. Failed stage is visually traceable to related terminal run event.
5. Reconciliation screen clearly marks orphaned/unknown handles.

---

## 8. Non-Goals (v0.1)

- No distributed scheduler UX.
- No multi-node topology map.
- No new runtime APIs beyond current contract.

