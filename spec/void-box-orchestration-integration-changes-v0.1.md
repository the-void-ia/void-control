# Void-Box Changes Required for Controller Orchestration Integration

## Version: v0.1

## Scope
This document defines the required void-box daemon/runtime changes to
support first-class orchestration by `void-control`.

This is a specification only. No void-box repository code changes are
made in this repo.

---

# 1. Problem Summary

Current void-box daemon endpoints are sufficient for ad hoc run control,
but not for strict controller-runtime contract compliance in
`void-control-runtime-spec-v0.2.md`.

Main gaps:
- No runtime `attempt_id` model.
- No stable terminal event id contract.
- No run-level execution policy input.
- No resumable event stream API (snapshot-only `/events`).
- Event typing not aligned to canonical contract names.

---

# 2. Required API Changes

## 2.1 Start API (`POST /v1/runs`)

Current:
```json
{"file":"path","input":"optional"}
```

Required:
```json
{
  "run_id":"optional-controller-id",
  "file":"path",
  "input":"optional",
  "policy":{
    "max_parallel_microvms_per_run":8,
    "max_stage_retries":1,
    "stage_timeout_secs":900,
    "cancel_grace_period_secs":20
  }
}
```

Response must include:
```json
{"run_id":"...","attempt_id":1,"state":"running"}
```

## 2.2 Inspect API (`GET /v1/runs/{id}`)

Response must include:
- `attempt_id`
- `active_stage_count`
- `active_microvm_count`
- `started_at` and `updated_at` in RFC3339 UTC
- `terminal_reason` and `exit_code` when terminal

## 2.3 Stop API (`POST /v1/runs/{id}/cancel`)

Request should accept:
```json
{"reason":"user requested"}
```

Response should include:
```json
{"run_id":"...","state":"canceled","terminal_event_id":"evt_..."}
```

## 2.4 Events API

Add resumable API:
- `GET /v1/runs/{id}/events?from_event_id=evt_123`

Event envelope must include:
- `event_id` (stable unique)
- `event_type` (contract-aligned canonical names)
- `run_id`
- `attempt_id`
- `timestamp` (RFC3339 UTC)
- `seq` (strictly increasing per run+attempt)
- `payload`

Canonical event names:
- `RunStarted`
- `StageStarted`
- `StageCompleted`
- `StageFailed`
- `MicroVmSpawned`
- `MicroVmExited`
- `RunCompleted`
- `RunFailed`
- `RunCanceled`

---

# 3. Runtime Semantics Changes

## 3.1 Attempt Model

- Introduce `attempt_id` per run.
- Increment on restart/retry.
- Emit `attempt_id` in all run/event responses.

## 3.2 Idempotency

- `start` idempotent for active run.
- `cancel` idempotent for terminal run.
- `inspect` idempotent and safe after daemon restart.

## 3.3 Reconciliation Support

Add one of:
- `GET /v1/runs?state=active`
or
- `GET /v1/runs/active`

Controller needs this to reload non-terminal runs and resume tracking.

---

# 4. Error Contract

All non-2xx responses should use:
```json
{
  "code":"INVALID_POLICY",
  "message":"...",
  "retryable":false
}
```

Minimum codes:
- `INVALID_SPEC`
- `INVALID_POLICY`
- `NOT_FOUND`
- `ALREADY_TERMINAL`
- `RESOURCE_LIMIT_EXCEEDED`
- `INTERNAL_ERROR`

---

# 5. Compatibility / Migration

## 5.1 Transitional Mode

For backward compatibility, daemon may keep legacy event names
(`run.started`, `run.finished`, etc.) behind a compatibility flag.

## 5.2 Recommended Rollout

1. Add new fields/endpoints without removing legacy behavior.
2. Enable controller integration tests against new endpoints.
3. Deprecate legacy event naming after controller cutover.

---

# 6. Acceptance Criteria

Void-box is considered orchestration-ready when:
- Controller can `start`, `inspect`, `subscribe_events(from_event_id)`,
  and `stop` using only run-level operations.
- All events include stable `event_id`, strict `seq`, and `attempt_id`.
- Reconciliation after controller restart requires no manual recovery.
- Policy input is accepted and enforced by runtime.
