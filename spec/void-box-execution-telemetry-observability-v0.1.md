# Void-Box Execution + Telemetry Observability Specification

## Version: v0.1

## Scope
Define required `void-box` runtime/daemon changes so `void-control` can visualize:
- real execution flow (steps/boxes, fan-out, fan-in),
- live step state transitions,
- host/guest runtime metrics (CPU, memory, I/O, network),
- resumable event and telemetry streams.

This spec extends:
- `spec/void-control-runtime-spec-v0.2.md`
- `spec/void-box-orchestration-integration-changes-v0.1.md`

---

## 1. Problem Summary

Current UI can only display run-level events (`RunStarted`, `WorkflowPlanned`, `RunCompleted`, etc.).  
It cannot show true step-level execution, parallel groups, or resource behavior during run execution.

Required outcome:
1. step/box lifecycle is externally observable,
2. fan-out/fan-in topology is reconstructible from runtime events,
3. telemetry is available as resumable time-series data.

---

## 2. Event Contract Additions (`GET /v1/runs/{id}/events`)

### 2.1 New Event Types

- `StepQueued`
- `StepStarted`
- `StepStdoutChunk`
- `StepStderrChunk`
- `StepSucceeded`
- `StepFailed`
- `StepSkipped`
- `TelemetrySample`

### 2.2 Required Payload Fields for Step Events

```json
{
  "step_name": "aggregate",
  "box_name": "writer-box",
  "depends_on": ["parallel_a", "parallel_b"],
  "group_id": "g2",
  "attempt": 1,
  "started_at": "2026-03-03T18:00:00Z",
  "finished_at": "2026-03-03T18:00:01Z",
  "duration_ms": 1000
}
```

Notes:
- `group_id` identifies steps that may execute in parallel.
- `depends_on` is required on all `Step*` events.
- `finished_at`/`duration_ms` are required on terminal step events (`StepSucceeded`, `StepFailed`, `StepSkipped`).

### 2.3 Telemetry Event Payload

```json
{
  "scope": "run",
  "step_name": "optional",
  "box_name": "optional",
  "sample_seq": 42,
  "host": {
    "cpu_percent": 61.2,
    "rss_mb": 512,
    "io_read_bytes": 12345,
    "io_write_bytes": 9876,
    "net_rx_bytes": 1000,
    "net_tx_bytes": 800
  },
  "guest": {
    "cpu_percent": 48.1,
    "mem_used_mb": 384,
    "load_1m": 0.72
  }
}
```

---

## 3. New Runtime APIs

### 3.1 `GET /v1/runs/{id}/stages`

Returns current step snapshot for graph/state reconstruction.

```json
{
  "run_id": "ux-fanout-fanin-demo",
  "attempt_id": 1,
  "stages": [
    {
      "step_name": "parallel_a",
      "box_name": "parallel_a",
      "group_id": "g1",
      "state": "queued",
      "depends_on": ["ingest"],
      "started_at": null,
      "finished_at": null,
      "duration_ms": null,
      "retries": 0,
      "last_error": null
    }
  ],
  "updated_at": "2026-03-03T18:00:02Z"
}
```

### 3.2 `GET /v1/runs/{id}/telemetry?from_seq=...&scope=run|step&step_name=...`

```json
{
  "run_id": "ux-fanout-fanin-demo",
  "attempt_id": 1,
  "samples": [],
  "next_seq": 43
}
```

`from_seq` is resumable and idempotent, equivalent to `from_event_id` semantics on events.

---

## 4. Semantics (Normative)

1. Event `seq` remains strictly increasing per `run_id + attempt_id`.
2. Telemetry `sample_seq` is strictly increasing per `run_id + attempt_id`.
3. Step transitions must obey:
   - `queued -> started -> terminal`
   - terminal = `succeeded | failed | skipped`
4. For fan-out/fan-in:
   - parallel siblings share `group_id`,
   - fan-in step must expose all upstream dependencies in `depends_on`.
5. Runtime updates `active_stage_count` and `active_microvm_count` consistently with stage snapshot.

---

## 5. Collection and Cadence

Default telemetry sampling:
- interval: `1000ms`
- enable flag: `VOIDBOX_TELEMETRY_ENABLED=true`
- cadence override: `VOIDBOX_TELEMETRY_INTERVAL_MS=<n>`
- per-run retention: latest 5000 samples (default)

Guest metrics are best-effort:
- if unavailable, omit `guest` object (do not fail run).

---

## 6. Compatibility and Rollout

1. Additive change only; existing run APIs remain valid.
2. Use `#[serde(default)]` for all newly added persisted fields.
3. Keep legacy event types available in compatibility mode.
4. New APIs return empty collections when telemetry/stage data is unavailable, not 500.

---

## 7. Validation / Acceptance Criteria

1. Diamond DAG (`A -> (B,C) -> D`) produces observable parallel group:
   - `B.group_id == C.group_id`,
   - `D.depends_on == [B,C]`.
2. `GET /stages` reflects live transitions and final terminal state per step.
3. `GET /telemetry?from_seq` returns only newer samples and stable `next_seq`.
4. Controller restart + reconciliation can resume events and telemetry without duplication.
5. UI can render:
   - live node states per step,
   - fan-out/fan-in graph edges,
   - CPU/memory charts per run (and optionally per step).

---

## 8. Non-Goals (v0.1)

- No WebSocket requirement (HTTP polling + resumable cursors is sufficient).
- No distributed multi-node telemetry aggregation.
- No change to scheduler ownership boundaries (controller remains run-level orchestrator).
