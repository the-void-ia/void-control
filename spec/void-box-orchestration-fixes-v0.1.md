# Void-Box Orchestration Fixes (Cancel Idempotency + Timeout Enforcement)

## Version: v0.1

## Scope
This spec defines the minimum `void-box` changes required to close the remaining
controller contract gaps observed from `void-control` live integration tests.

Target failures:
- `cancel_idempotency`
- `policy_timeout_enforced_failure`

This document is implementation-focused and intentionally narrow.

---

## 1. Problem Statements

### 1.1 Cancel idempotency is not terminal-event stable
Observed behavior: repeated `POST /v1/runs/{id}/cancel` returns different
`terminal_event_id` values.

Required behavior: once a run reaches terminal state through cancel, all
subsequent cancel calls must return the same `terminal_event_id`.

### 1.2 Policy timeout is accepted but not enforced
Observed behavior: run policy includes `stage_timeout_secs`, but a long-running
step still completes successfully instead of failing on timeout.

Required behavior: policy timeout must be applied to runtime execution, causing
terminal `failed` status on timeout.

---

## 2. Required Changes

### 2.1 Stable terminal event identity for cancel

#### Data model
In `RunState`, add:
- `terminal_event_id: Option<String>` (`#[serde(default)]`)

#### API semantics
For `POST /v1/runs/{id}/cancel`:
- If run is non-terminal:
  - append cancel terminal event once,
  - persist its id in `run.terminal_event_id`,
  - return that id in response.
- If run is already terminal:
  - do not append any new event,
  - return `run.terminal_event_id` unchanged.

#### Concurrency guard
Background run completion logic must not overwrite a run already marked
terminal by cancel. If terminal, skip completion mutation and event append.

### 2.2 Enforce `stage_timeout_secs` in runtime execution

#### Policy threading
Thread `policy: Option<RunPolicy>` from daemon start request into:
- workflow execution path,
- pipeline execution path.

#### Timeout rules
- When a step has explicit timeout, keep explicit timeout.
- When a step has no explicit timeout and policy provides
  `stage_timeout_secs`, apply policy timeout.
- Service-mode infinite timeout semantics remain explicit and must not be
  silently overridden.

#### Failure mapping
When timeout expires:
- step result must be failure,
- run terminal status must become `failed`,
- failure event must be emitted with timeout reason.

---

## 3. Acceptance Criteria

The following must pass against a live daemon:

1. `void-control/tests/void_box_contract.rs::cancel_idempotency`
2. `void-control/tests/void_box_contract.rs::policy_timeout_enforced_failure`

Expected outcomes:
- repeated cancel returns identical `terminal_event_id`;
- run started with `stage_timeout_secs=1` and long sleep step ends as `failed`.

---

## 4. Non-Goals

- No controller-side behavior changes.
- No event schema redesign.
- No change to existing start/inspect/list API shape beyond fields above.
