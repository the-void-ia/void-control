# Void-Box Orchestration Runtime Readiness

## Version: v0.1

## Scope
This spec defines the `void-box` runtime changes required to support
forward-looking orchestration by `void-control`.

It extends the existing controller/runtime boundary described in:
- `spec/void-control-runtime-spec-v0.2.md`
- `spec/void-control-iteration-spec-v0.2.md`
- `spec/void-box-orchestration-integration-changes-v0.1.md`
- `spec/void-box-orchestration-fixes-v0.1.md`

This document is intentionally contract-first:
- it defines the runtime guarantees `void-control` depends on,
- it adds internal `void-box` guidance only where needed to make those
  guarantees implementable and testable,
- it does not move orchestration ownership from `void-control` into
  `void-box`.

---

# 1. Ownership Boundary

`void-control` owns:
- execution-level orchestration,
- iterative strategies such as `swarm`,
- cross-run scheduling and admission control,
- execution persistence and control-plane events,
- execution pause/resume/cancel policy,
- scoring, convergence, and reduction.

`void-box` owns:
- single-run workflow/stage execution,
- microVM isolation and runtime policy enforcement,
- durable publication of stage output artifacts,
- run inspection data,
- typed runtime failure reporting.

`void-box` MUST NOT:
- make cross-run scheduling decisions,
- infer execution-level strategy semantics,
- compute orchestration scores,
- replace control-plane execution state with runtime-local guesses.

---

# 2. Problem Summary

`void-control` can already launch and inspect child runs, but orchestration
readiness still depends on stronger runtime guarantees.

Current gaps:
- structured outputs are retrievable, but not yet modeled as a stable
  first-class artifact contract,
- additional artifacts are not exposed through a durable manifest contract,
- inspection data is not yet specified as the normalized source for
  reconciliation support,
- artifact publication and retrieval failure modes are not fully typed,
- internal publication/storage guidance is missing, which risks daemon
  implementations that technically expose files but do not make them
  durable, discoverable, or testable.

---

# 3. Required External Contract Changes

## 3.1 Structured stage output is a first-class runtime contract

For orchestration-facing stages, `void-box` MUST treat `result.json` as the
canonical structured output artifact.

`result.json` MUST be machine-readable JSON and MUST support this shape:

```json
{
  "status": "ok",
  "summary": "short human-readable summary",
  "metrics": {
    "latency_p99_ms": 98,
    "cost_usd": 0.02
  },
  "artifacts": [
    {
      "name": "report.md",
      "path": "report.md",
      "media_type": "text/markdown"
    }
  ]
}
```

Rules:
- `status` is required.
- `summary` is optional but recommended.
- `metrics` is a flat string-to-number map.
- `artifacts` is a list of references to additional outputs produced by the
  same stage.
- unknown fields are allowed for forward compatibility.

## 3.2 Artifact retrieval contract

The current endpoint remains valid:
- `GET /v1/runs/{run_id}/stages/{stage}/output-file`

For orchestration readiness, `void-box` MUST guarantee that this endpoint
returns the canonical structured output for the stage when `result.json`
exists.

Forward-looking support MUST also be added for named artifact retrieval:
- `GET /v1/runs/{run_id}/stages/{stage}/artifacts/{name}`

Response behavior:
- `200` with artifact content when found,
- `404` with typed error when the named artifact does not exist,
- `409` or `424` style typed error when artifact publication is incomplete,
- `5xx` only for true internal failures.

## 3.3 Artifact manifest contract

Each completed stage that publishes structured output MUST have a stable
artifact manifest available through run inspection or stage inspection.

Minimum manifest entry shape:

```json
{
  "name": "report.md",
  "stage": "main",
  "media_type": "text/markdown",
  "size_bytes": 1824,
  "retrieval_path": "/v1/runs/run_123/stages/main/artifacts/report.md"
}
```

Rules:
- `name` is stable within a stage.
- `retrieval_path` is the canonical retrieval URI suffix exposed by the
  daemon.
- `size_bytes` MAY be omitted if not known cheaply, but SHOULD be present.
- the manifest MUST include the canonical structured output artifact even if
  it is also retrievable via `output-file`.

## 3.4 Run inspection and reconciliation support

`GET /v1/runs/{id}` MUST expose normalized fields sufficient for
`void-control` reconciliation after restart.

Required fields:
- `run_id`
- `attempt_id`
- `state`
- `started_at`
- `updated_at`
- `finished_at` when terminal
- `terminal_reason` when terminal
- `active_stage_count`
- `active_microvm_count`
- `stage_states`
- `artifact_publication`

Recommended `artifact_publication` shape:

```json
{
  "status": "published",
  "published_at": "2026-03-20T18:20:00Z",
  "manifest": [
    {
      "name": "result.json",
      "stage": "main",
      "media_type": "application/json",
      "retrieval_path": "/v1/runs/run_123/stages/main/output-file"
    }
  ]
}
```

`artifact_publication.status` MUST distinguish at least:
- `not_started`
- `publishing`
- `published`
- `failed`

## 3.5 Active-run listing for reconciliation

`void-box` MUST expose one of:
- `GET /v1/runs?state=active`
- `GET /v1/runs/active`

This endpoint MUST be safe after daemon restart and MUST return enough
inspection data for `void-control` to resume runtime tracking of non-terminal
runs.

---

# 4. Failure and Error Semantics

`void-box` MUST surface typed conditions for output and publication failures.

Minimum conditions:
- `STRUCTURED_OUTPUT_MISSING`
- `STRUCTURED_OUTPUT_MALFORMED`
- `ARTIFACT_NOT_FOUND`
- `ARTIFACT_PUBLICATION_INCOMPLETE`
- `ARTIFACT_STORE_UNAVAILABLE`
- `RETRIEVAL_TIMEOUT`

These conditions MAY appear:
- in non-2xx HTTP error payloads,
- in run inspection terminal metadata,
- in event payloads when such events exist.

Minimum error envelope:

```json
{
  "code": "STRUCTURED_OUTPUT_MISSING",
  "message": "main stage completed without result.json",
  "retryable": false
}
```

Rules:
- missing structured output is distinct from malformed structured output,
- artifact lookup failure is distinct from publication-not-yet-complete,
- retrieval timeout is distinct from daemon internal failure.

---

# 5. Artifact and Output Semantics

## 5.1 Publication durability

Published artifacts MUST remain retrievable for at least the configured
retention window after run completion.

`void-box` MUST NOT report artifacts as published before their retrieval path
is actually readable.

## 5.2 Publication atomicity

Artifact publication SHOULD behave atomically from the perspective of
inspection:
- before publication completes, manifest state is `publishing`,
- after publication completes, manifest state becomes `published` and listed
  artifacts are retrievable,
- partial publication MUST surface as `failed` or `publishing`, never as a
  silently incomplete `published` manifest.

## 5.3 Backward compatibility

During rollout:
- `output-file` remains supported,
- `result.json` remains the orchestration default,
- manifest support is additive,
- existing non-orchestration uses of `void-box` MUST continue working.

---

# 6. Internal Implementation Guidance

This section is guidance, not a required source-level design, but daemon
implementations SHOULD follow it closely.

## 6.1 Separate metadata from raw artifact bytes

`void-box` SHOULD persist artifact metadata separately from the artifact
contents so inspection can answer quickly without directory scans.

Suggested persisted metadata:
- run id
- attempt id
- stage
- artifact name
- media type
- size
- publication status
- retrieval path
- publication timestamp

## 6.2 Treat artifact publication as an explicit runtime step

Artifact publication SHOULD be modeled as a distinct post-stage step:
- stage execution produces local outputs,
- publication validates and registers structured outputs,
- inspection reads the persisted publication result.

This avoids mixing "stage exited successfully" with "artifact contract is
durably published".

## 6.3 Normalize per-run inspection state

`void-box` SHOULD maintain a normalized per-run summary record containing:
- lifecycle state,
- terminal reason,
- stage terminal states,
- active stage count,
- active microVM count,
- artifact publication status,
- artifact manifest,
- timestamps.

Inspection endpoints SHOULD read from this normalized record rather than
recomputing state from logs on demand.

## 6.4 Keep execution and publication responsibilities separate

Recommended split:
- execution worker: runs stages and records runtime facts,
- publication step: validates `result.json`, registers artifacts, updates
  manifest status,
- inspection layer: serves normalized state and retrieval metadata.

This split is guidance, not a requirement for separate processes.

## 6.5 Retention and cleanup coordination

Retention logic SHOULD ensure:
- manifests do not outlive the referenced artifact bytes,
- artifact bytes do not remain indefinitely without manifest metadata,
- terminal inspection remains useful until retention expiry.

Cleanup SHOULD update publication metadata consistently rather than leaving
stale retrieval paths behind.

---

# 7. Compatibility and Migration

Recommended rollout order:

1. Add additive inspection fields and typed error payloads.
2. Make `result.json` publication rules explicit and contract-tested.
3. Add manifest support and named artifact retrieval.
4. Add reconciliation-ready active-run listing.
5. Deprecate any ad hoc artifact discovery assumptions in controller code.

Non-goals for this version:
- redesigning stage orchestration inside `void-box`,
- moving execution-level pause/resume logic into `void-box`,
- defining strategy-specific artifact schemas beyond the base `result.json`
  contract,
- specifying a particular storage backend.

---

# 8. Acceptance Criteria

`void-box` is orchestration-ready for this spec when all of the following are
true against a live daemon:

1. `void-control` can submit a child run, inspect it, and reconcile its
   lifecycle after restart using only runtime APIs and published artifacts.
2. A successful orchestration-facing stage publishes a valid `result.json`
   retrievable through `GET /v1/runs/{run_id}/stages/{stage}/output-file`.
3. Additional artifacts referenced from `result.json` are discoverable via a
   stable manifest and retrievable via named artifact endpoints.
4. A run that completes without `result.json` returns a typed
   `STRUCTURED_OUTPUT_MISSING` condition.
5. A run with malformed `result.json` returns a typed
   `STRUCTURED_OUTPUT_MALFORMED` condition.
6. Run inspection exposes artifact publication status without requiring log
   scraping.
7. Active-run listing after daemon restart is sufficient for controller
   reconciliation.
8. Contract tests cover:
   - structured output retrieval,
   - missing output classification,
   - malformed output classification,
   - manifest publication,
   - named artifact retrieval,
   - active-run reconciliation support.
