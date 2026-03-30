# Message Delivery Architecture Design

## Date: 2026-03-23

## Problem

void-control has a message-box layer that handles intent normalization,
routing, and inbox materialization. But there is no design for how messages
actually reach agents running inside void-box VMs, or how agents emit
intents back to the control plane.

void-box candidates are not limited to Claude Code. They can be arbitrary
programs in OCI containers (Node.js, Python, custom binaries). The
delivery mechanism must be vendor-neutral while enabling rich integrations
for specific providers.

## Constraints

- void-box runs three execution models: batch (ephemeral), long-running
  (service mode), and snapshot/restore.
- Agents can be Claude Code, OpenClaw, Ollama-backed models, or arbitrary
  OCI workloads.
- The message-box spec requires determinism, replayability, and
  content-blind strategy behavior.
- void-control must not reach into VMs directly. void-box owns the
  host-guest boundary.
- The signal-reactive spec requires `MessageStats` derived from message
  metadata only.

## Research

Three real-world patterns were evaluated:

| Pattern | Example | Mechanism |
|---------|---------|-----------|
| MCP tools | claude-peers-mcp | Broker-mediated, tool-based send/receive |
| Channel push | Claude Code channels | MCP notification push into session |
| Launch injection | void-control (current) | Inbox serialized into launch_context |

Key findings:
- claude-peers-mcp uses a SQLite broker with poll + channel push hybrid.
- Claude Code channels are MCP servers with push notifications, not a
  separate protocol.
- agent-swarm (desplega-ai) uses MCP tools over a central SQLite-backed
  server with channel messaging and task delegation.
- All approaches are provider-specific. None solve the "any OCI container"
  problem.

## Compatibility Statement

This document extends the message-box v0 transport with optional
live-delivery support for service-mode runs. Launch injection remains
the canonical and required delivery mode — all adapters MUST support
it. Live delivery (`LivePush`, `LivePoll`) is an optional capability
that adapters MAY declare for long-running executions. An adapter that
supports only `LaunchInjection` is fully conformant.

## Architecture

### Layered Model

```
void-control (semantics)
    │
    │  HTTP API calls
    ▼
void-box daemon (transport)
    │
    ├── HTTP Sidecar (always present, canonical bus)
    │       │
    │       ├── Generic agent: curls sidecar directly
    │       ├── Claude Code bridge: sidecar → channels/MCP push
    │       └── Custom bridge: sidecar → whatever protocol
    │
    └── VM guest (agent runtime)
```

Three layers with clear ownership:

1. **void-control** owns semantics: intents, routing, signals, planning.
2. **void-box** owns transport: sidecar, bridges, VM networking.
3. **Agents** own interpretation: read inbox, reason, emit intents.

### Layer 1: HTTP Sidecar (Base, Non-Negotiable)

An HTTP server runs host-side, reachable from inside the VM via SLIRP
network (`http://10.0.2.2:<port>` on KVM) or vsock forwarding.

Guest-facing endpoints (versioned):

```
GET  /v1/inbox          → InboxSnapshot (JSON), supports ?since=version
POST /v1/intents        → accept intent or intent batch, 201 Created
GET  /v1/context        → execution identity and metadata (JSON)
GET  /v1/health         → sidecar liveness + protocol version
GET  /v1/signals        → reserved, returns 501 Not Implemented in v1
```

Why host-side:
- No guest-agent dependency — works with any OCI image.
- void-control can pre-load inbox before VM boots.
- void-control can collect intents without waiting for graceful shutdown.
- Works identically for batch, long-running, and snapshot/restore.

#### `GET /v1/inbox`

Reuses existing `InboxSnapshot` with an added `version` field for
incremental polling:

```json
{
  "version": 3,
  "execution_id": "exec-1",
  "candidate_id": "c-3",
  "iteration": 2,
  "entries": [
    {
      "message_id": "msg-001",
      "from_candidate_id": "c-1",
      "kind": "proposal",
      "payload": { "summary_text": "..." }
    }
  ]
}
```

Long-running agents can poll incrementally:

```
GET /v1/inbox?since=3  → only entries added after version 3
```

This enables efficient polling without re-reading the full inbox.

#### `POST /v1/intents`

Accepts a single intent or a batch (array). The sidecar auto-fills
`intent_id`, `from_candidate_id`, `iteration`, and `ttl_iterations`
from execution context.

**Iteration stamping rule (normative):** The sidecar tracks a current
iteration number, initialized from the `iteration` field of the most
recent `InboxSnapshot` received via `PUT /v1/runs/{id}/inbox`. All
intents accepted after inbox version N and before the next inbox
injection are stamped with `source_iteration = N`. This is the sole
mechanism for iteration assignment — agents never set `iteration`
themselves.

Single intent:

```json
{
  "kind": "signal",
  "audience": "broadcast",
  "payload": { "summary_text": "converging on approach A" },
  "priority": "normal"
}
```

Batch submission:

```json
[
  { "kind": "proposal", "audience": "broadcast", "payload": {...}, "priority": "normal" },
  { "kind": "evaluation", "audience": "leader", "payload": {...}, "priority": "high" }
]
```

Batching enables atomic multi-intent submission and simplifies limit
enforcement (max 3 intents per candidate per iteration, validated once).

**Idempotency:** Agents SHOULD include an `Idempotency-Key` header:

```
POST /v1/intents
Idempotency-Key: <uuid>
```

The sidecar guarantees: same key → same intent, no duplicates. This is
required for long-running agents, flaky networks, and retry safety.

**Non-blocking:** `POST /v1/intents` always returns fast (fire-and-forget
semantics). The sidecar buffers intents asynchronously. The agent is never
blocked waiting for control-plane processing.

#### `GET /v1/context`

Returns execution identity — critical for agent reasoning, debugging,
and multi-agent coordination:

```json
{
  "execution_id": "exec-1",
  "candidate_id": "c-3",
  "iteration": 2,
  "role": "candidate",
  "peers": ["c-1", "c-2", "c-4"],
  "sidecar_version": "0.1.0"
}
```

#### `GET /v1/signals` (reserved)

Returns `501 Not Implemented` in v1. Reserved for future use where
agents may inspect control-plane-derived signals (convergence state,
planning hints). Returning 501 instead of an empty array prevents
agents from building polling logic against a no-op endpoint whose
semantics will change in v2. Keeps the system symmetric: agents emit
intents, control plane emits signals.

### Layer 2: Provider Adapters (Capability Layer)

Each adapter declares what it supports and the orchestrator adapts:

```rust
pub enum DeliveryCapability {
    LaunchInjection,   // batch: pre-load inbox before start
    RestoreInjection,  // snapshot: re-load inbox at restore
    LivePush,          // long-running: push messages mid-execution
    LivePoll,          // long-running: agent polls for new messages
}

/// Reference to a void-box run. The adapter uses this to call
/// void-box daemon endpoints — it never touches the sidecar directly.
pub struct VoidBoxRunRef {
    pub daemon_base_url: String,
    pub run_id: String,
}

pub trait MessageDeliveryAdapter: Send + Sync {
    fn capabilities(&self) -> Vec<DeliveryCapability>;

    /// Required: pre-load inbox via void-box daemon
    fn inject_at_launch(
        &self,
        run: &VoidBoxRunRef,
        candidate: &CandidateSpec,
        inbox: &InboxSnapshot,
    ) -> Result<()>;

    /// Required: drain emitted intents via void-box daemon.
    /// Called after completion (batch) or periodically (long-running).
    /// NOT idempotent — drain clears the sidecar buffer. Calling twice
    /// returns an empty vec on the second call. The control-plane store
    /// is the durable copy after drain.
    fn drain_intents(
        &self,
        run: &VoidBoxRunRef,
    ) -> Result<Vec<CommunicationIntent>>;

    /// Optional: push a new message to a running candidate
    fn push_live(
        &self,
        _run: &VoidBoxRunRef,
        _message: &InboxEntry,
    ) -> Result<()> {
        Err(Error::Unsupported("live push"))
    }

    /// Generate skill content for the agent
    fn messaging_skill(&self, run: &VoidBoxRunRef) -> String;
}
```

The adapter talks to void-box daemon endpoints, never directly to the
sidecar. Bridge deployment (Claude channels, MCP) is void-box's concern,
configured via `messaging.provider_bridge` in the RunSpec.

**Orchestrator capability dispatch:**

| Capability | Orchestrator behavior | Fallback if absent |
|---|---|---|
| `LaunchInjection` | Call `inject_at_launch` before VM start | Error — required |
| `RestoreInjection` | Call `inject_at_launch` before VM restore | Error for snapshot mode |
| `LivePush` | Call `push_live` when new messages arrive mid-iteration | Skip — agent polls |
| `LivePoll` | No orchestrator action needed — agent self-serves | N/A |

Default implementation: `HttpSidecarAdapter` — calls `PUT /v1/runs/{id}/inbox`
and `GET /v1/runs/{id}/intents` on the void-box daemon. Supports
`LaunchInjection` and `LivePoll`.

Claude Code implementation: `ClaudeChannelAdapter` — delegates storage to
void-box daemon (same endpoints), but sets `provider_bridge: claude_channels`
in the RunSpec so void-box deploys the MCP channel bridge. Adds `LivePush`
via `POST /v1/runs/{id}/messages`.

### Layer 3: Skill Injection (Discoverability)

Agents need to know the messaging system exists. Each adapter generates
appropriate instructions via `messaging_skill()`.

For generic agents (HTTP sidecar):

```markdown
# Collaboration Protocol

You are part of a multi-agent execution.

## Reading messages
GET http://10.0.2.2:8090/v1/inbox

## Sending messages
POST http://10.0.2.2:8090/v1/intents
Content-Type: application/json

{"kind": "proposal", "audience": "broadcast",
 "payload": {"summary_text": "..."}, "priority": "normal"}

## Message kinds
- proposal: concrete solution or approach
- signal: observation other agents should know
- evaluation: assessment of another agent's proposal

## Audience
- broadcast: all agents
- leader: coordinator only
```

For Claude Code agents: no skill file needed. The MCP channel bridge
provides `instructions` and a `send_intent` tool. Messages arrive as
`<channel>` tags automatically.

Injection point (requires new `add_skill` method on `CandidateSpec` and
`Skill::inline` constructor — these are new types to be introduced):

```rust
let skill_content = adapter.messaging_skill(&run_ref);
candidate_spec.add_skill(Skill::inline("void-messaging", &skill_content));
```

## Ownership Boundary: void-box Owns the Sidecar

The sidecar process belongs to void-box, not void-control.

void-box owns VM lifecycle, host-guest communication, and networking.
void-control should not know about VM networking details.

void-box exposes new daemon endpoints:

```
PUT  /v1/runs/{id}/inbox       → void-control pushes InboxSnapshot
GET  /v1/runs/{id}/intents     → void-control collects emitted intents
POST /v1/runs/{id}/messages    → void-control pushes live message
```

void-box handles: starting/stopping sidecar per run, guest reachability,
intent collection, provider bridge deployment.

**Iteration advancement:** `PUT /v1/runs/{id}/inbox` implicitly advances
the sidecar's iteration. The `InboxSnapshot` carries the `iteration`
field — when the sidecar receives a new inbox, it resets per-iteration
counters (intent limits, rate-limit windows) to match the new iteration.
No separate "advance iteration" signal is needed.

The adapter in void-control is thin — it calls void-box HTTP endpoints:

```
void-control                    void-box daemon
     │                               │
     │  PUT /v1/runs/{id}/inbox      │
     ├──────────────────────────────►│──► sidecar ──► VM guest
     │                               │
     │  GET /v1/runs/{id}/intents    │
     │◄──────────────────────────────┤◄── sidecar ◄── VM guest
```

RunSpec extension:

```yaml
agent:
  prompt: "..."
  messaging:
    enabled: true
    provider_bridge: claude_channels  # optional
```

## Lifecycle Integration

### Batch (ephemeral)

```
1. void-box starts sidecar for the run
2. adapter.inject_at_launch(run_ref, candidate, inbox)
   → PUT /v1/runs/{id}/inbox on void-box daemon
3. Skill injected into candidate spec
4. VM boots → agent reads /v1/inbox → works → POSTs /v1/intents
5. VM exits
6. adapter.drain_intents(run_ref) → intents
   → GET /v1/runs/{id}/intents on void-box daemon
7. void-box tears down sidecar
```

### Long-running (service mode)

```
1. void-box starts sidecar (persistent for run lifetime)
2. adapter.inject_at_launch(run_ref, candidate, inbox)
3. void-box deploys provider bridge if configured in RunSpec
4. VM boots, agent runs continuously
5. New iteration: orchestrator pushes new messages:
   adapter.push_live(run_ref, message)
   → POST /v1/runs/{id}/messages on void-box daemon
   OR agent polls GET /v1/inbox?since=version
6. Agent POSTs /v1/intents at any time (non-blocking)
7. Orchestrator periodically drains:
   adapter.drain_intents(run_ref) → intents
```

### Snapshot/restore

```
1. Iteration N: normal batch flow
2. VM snapshot saved
3. void-box kills sidecar process for the run
4. Iteration N+1: void-box spawns a new sidecar process (clean state)
5. adapter.inject_at_launch(run_ref, candidate, new_inbox)
6. VM restored from snapshot
7. Agent resumes, reads /v1/inbox → sees iteration N+1 messages
8. Normal collection on completion
```

The sidecar process is **restarted** between iterations for
snapshot/restore — not cleared, not reused. A fresh process guarantees
no stale buffers, no leaked intent state from the previous iteration,
and no ambiguity about which iteration's data the sidecar holds. VM
state carries agent memory; sidecar state does not survive.

## Intent Collection and Signal Extraction

Intents flow from sidecar back into the existing message-box pipeline:

```
Agent POSTs /intents
  → sidecar buffers (append-only, deduped by hash of (normalized_payload, audience, source_iteration))
  → void-control calls GET /v1/runs/{id}/intents
  → adapter.drain_intents() → Vec<CommunicationIntent>
  → normalize_intents() validates (max 3 per candidate, etc.)
  → route_intents() maps audience → RoutedMessage
  → materialize_inbox_snapshots() builds next iteration inboxes
  → extract_message_stats() derives MessageStats
  → strategy.plan_candidates(execution, iteration, stats)
```

### Dual Intent Sources

Two sources of intents, merged and deduplicated:

1. Sidecar buffer (collected via adapter)
2. Structured output (legacy, still supported)

```rust
let sidecar_intents = adapter.drain_intents(&run_ref)?;
let output_intents = candidate_output.intents;
let all_intents = merge_and_dedup(sidecar_intents, output_intents);
// dedup key: (normalized_payload, audience, source_iteration)
```

Backward compatible — agents embedding intents in output still work.

### Collection Timing

| Mode | When collected |
|------|---------------|
| Batch | After VM exits, before next iteration planning |
| Long-running | Periodically drained between iterations |
| Snapshot/restore | After VM suspends, before snapshot |

## Failure Semantics

### Injection failure

If `inject_at_launch` fails (sidecar unreachable, inbox too large):
- The candidate MUST NOT launch. A failed injection means the agent would
  run without coordination context, producing meaningless intents.
- The orchestrator emits a `MessageDeliveryFailed` diagnostic event.
- The candidate is marked as failed for this iteration.

### Collection failure

If `drain_intents` fails (sidecar died, VM crashed):
- The orchestrator treats the candidate as having emitted zero intents.
- This is a **partial loss**, not a fatal error — the execution continues
  with whatever intents were collected from other candidates.
- A `MessageCollectionFailed` diagnostic event is emitted.
- No retry — the sidecar buffer is ephemeral and may be gone.

### Sidecar unavailability during long-running execution

If the sidecar becomes unreachable mid-execution:
- Agents using HTTP sidecar: `GET /inbox` returns errors, agent continues
  working without new messages. Buffered intents are lost if sidecar dies.
- Agents using provider bridges: bridge detects sidecar loss, stops
  pushing. Agent continues with last-known state.
- The orchestrator detects sidecar loss via health check and emits
  `SidecarUnreachable` event.

### Payload limits

The sidecar enforces the message-box spec limits at the transport layer:
- `max_intent_payload_bytes: 4096` per intent
- `max_inbox_snapshot_bytes: 65536` per inbox
- `max_intents_per_candidate: 3` per iteration

Oversized payloads receive `413 Payload Too Large`. Excess intents
receive `429 Too Many Requests`.

## Backpressure and Rate Limits

The sidecar enforces rate limits to prevent agents from overwhelming
the coordination bus:

- Max intents per second per candidate (suggested: 10/s)
- Max payload size per intent (from message-box spec: 4096 bytes)
- Max total intents per iteration per candidate (from message-box spec: 3)

Exceeded limits return `429 Too Many Requests`. The agent should back off.

This prevents: swarm self-DDoS, runaway intent loops, unbounded buffer
growth.

## Source of Truth Clarification

The sidecar and the control-plane store serve different purposes:

- **Sidecar** = runtime buffer (ephemeral). Holds in-flight messages and
  buffered intents during candidate execution. Lost when sidecar stops.
- **Control-plane store** = source of truth (durable). Persisted intents,
  routed messages, and inbox snapshots in `intents.log`, `messages.log`,
  and `inboxes/` directories.

The sidecar is append-only and flushable. It is NOT authoritative
long-term. After `drain_intents`, intents are persisted to the
control-plane store and the sidecar buffer can be discarded.

Replay reconstructs from the control-plane store, never from sidecar
state.

## Deduplication Key

When merging intents from dual sources (sidecar + structured output),
deduplication uses the message-box spec's content-based key:

```
(normalized_payload, audience, source_iteration)
```

NOT `intent_id`, since the two sources generate IDs independently.
The sidecar auto-generates `intent_id` on submission, and structured
output intents may have their own IDs. Content-based dedup ensures
identical intents from both paths collapse to one.

## Migration from ProviderLaunchAdapter

### Current code shape

The compatibility seam today is:

```rust
// src/runtime/mod.rs:16
pub trait ProviderLaunchAdapter {
    fn prepare_launch_request(
        &self,
        request: StartRequest,
        candidate: &CandidateSpec,
        inbox: &InboxSnapshot,
    ) -> StartRequest;
}
```

`LaunchInjectionAdapter` (the only implementation) serializes the
inbox into `StartRequest.launch_context` as a JSON string. The call
site is `src/orchestration/service.rs:695`, inside candidate dispatch:

```rust
let launch_request = self.launch_adapter.prepare_launch_request(
    StartRequest { run_id, workflow_spec, launch_context: None, policy },
    candidate,
    &launch_inbox,
);
self.runtime.start_run(launch_request)?;
```

`ExecutionService` holds `launch_adapter: Box<dyn ProviderLaunchAdapter>`
(service.rs:43), defaulting to `LaunchInjectionAdapter` (service.rs:290),
with `with_launch_adapter()` (service.rs:297) for custom providers.

### Migration phases

**Phase 1: Introduce `MessageDeliveryAdapter` alongside the old trait.**

- Add `MessageDeliveryAdapter` trait in `src/runtime/mod.rs`.
- Add `HttpSidecarAdapter` as the default implementation.
- `ExecutionService` gains a new field:
  `delivery_adapter: Option<Box<dyn MessageDeliveryAdapter>>`.
- Constructor `with_delivery_adapter()` added.
- At candidate dispatch (service.rs:695), the logic becomes:

```rust
if let Some(ref delivery) = self.delivery_adapter {
    // New path: inject via void-box daemon sidecar
    delivery.inject_at_launch(&run_ref, candidate, &launch_inbox)?;
    self.runtime.start_run(StartRequest {
        run_id, workflow_spec, launch_context: None, policy,
    })?;
} else {
    // Legacy path: serialize inbox into launch_context
    let launch_request = self.launch_adapter.prepare_launch_request(
        StartRequest { run_id, workflow_spec, launch_context: None, policy },
        candidate, &launch_inbox,
    );
    self.runtime.start_run(launch_request)?;
}
```

- Intent collection in `persist_candidate_intents` (service.rs)
  similarly branches:

```rust
let intents = if let Some(ref delivery) = self.delivery_adapter {
    let sidecar_intents = delivery.drain_intents(&run_ref)?;
    let output_intents = candidate_output.intents;
    merge_and_dedup(sidecar_intents, output_intents)
} else {
    candidate_output.intents
};
// Existing pipeline continues unchanged:
let (valid, rejected) = normalize_intents(&intents, ...);
let routed = route_intents(&valid);
```

**Phase 2: Migrate callers.**

- All `with_launch_adapter()` call sites switch to
  `with_delivery_adapter()`.
- `ClaudeChannelAdapter` replaces any Claude-specific
  `ProviderLaunchAdapter` implementations.

**Phase 3: Remove the old path.**

- Remove `ProviderLaunchAdapter` trait and `LaunchInjectionAdapter`.
- Remove `launch_adapter` field from `ExecutionService`.
- Remove `with_launch_adapter()` constructor.
- Remove `StartRequest.launch_context` field (or keep as `deprecated`
  if external consumers exist).
- `delivery_adapter` becomes non-optional:
  `delivery_adapter: Box<dyn MessageDeliveryAdapter>`.

### When it is safe to remove the old path

The old path can be removed when:

1. All void-box deployments support the sidecar endpoints
   (`PUT /v1/runs/{id}/inbox`, `GET /v1/runs/{id}/intents`).
2. No external consumers of `with_launch_adapter()` remain.
3. The `HttpSidecarAdapter` health-check fallback to `launch_context`
   has been unused in production for at least one release cycle.

## Spec Amendments Required

This design requires amendments to existing specifications:

1. **Message-box spec Section 1 and Section 9**: Recognize
   sidecar-collected intents as a valid source alongside structured
   output. The ownership boundary "void-control owns intent extraction
   from child run structured output" should be broadened to "void-control
   owns intent extraction from child run structured output and sidecar
   collection."

2. **Signal-reactive spec Section 6.1 ("Required seam")**: Add a note that long-running
   mode periodic drain does not violate the `delivery_iteration >
   source_iteration` invariant — intents collected mid-iteration are
   still routed with correct delivery iteration at the next planning
   step.

3. **`RestoreInjection`** is a new delivery capability not in existing
   specs. The snapshot/restore lifecycle is an extension of the iteration
   spec and should be documented as such.

## Sidecar Observability

The sidecar is a critical path component — every message and intent
flows through it. Silent failures here mean lost coordination. The
sidecar MUST be observable at the same standard as the rest of the
void-* stack.

### Structured Logging

The sidecar uses the `tracing` crate with the same structured logging
conventions as void-box. Every log entry carries the run's trace
context (W3C Trace Context propagated from the daemon) so sidecar logs
correlate with the broader execution trace.

Required log events (all at `INFO` or above):

| Event | Level | Key fields |
|-------|-------|------------|
| Sidecar started | INFO | `run_id`, `listen_addr`, `sidecar_version` |
| Inbox loaded | INFO | `run_id`, `iteration`, `entry_count`, `snapshot_bytes` |
| Intent accepted | INFO | `run_id`, `intent_id`, `kind`, `audience`, `payload_bytes` |
| Intent rejected (limit) | WARN | `run_id`, `reason` (`max_per_iteration`, `rate_limit`, `payload_too_large`) |
| Intent deduplicated | DEBUG | `run_id`, `intent_id`, `dedup_key` |
| Intents drained | INFO | `run_id`, `intent_count` |
| Health check served | DEBUG | `run_id`, `client_addr` |
| Sidecar stopping | INFO | `run_id`, `buffered_intents`, `uptime_ms` |

All events include `run_id` and `trace_id` as span context. Sidecar
logs are collected by the void-box daemon alongside guest and host
logs.

### Metrics

The sidecar exposes Prometheus-compatible metrics via the void-box
daemon's existing telemetry pipeline. Metrics are collected into the
per-run `TelemetryRingBuffer` and exported via the daemon's
`GET /v1/runs/{run_id}/telemetry` endpoint.

**Counters:**

| Metric | Labels | Description |
|--------|--------|-------------|
| `sidecar_inbox_loads_total` | `run_id` | Number of inbox load operations |
| `sidecar_intents_accepted_total` | `run_id`, `kind` | Intents accepted by kind |
| `sidecar_intents_rejected_total` | `run_id`, `reason` | Intents rejected by reason |
| `sidecar_intents_drained_total` | `run_id` | Intents flushed to daemon |
| `sidecar_requests_total` | `run_id`, `endpoint`, `status` | HTTP requests served |

**Gauges:**

| Metric | Labels | Description |
|--------|--------|-------------|
| `sidecar_buffer_depth` | `run_id` | Current buffered intent count |
| `sidecar_inbox_version` | `run_id` | Current inbox version number |
| `sidecar_uptime_seconds` | `run_id` | Seconds since sidecar start |

**Histograms:**

| Metric | Labels | Description |
|--------|--------|-------------|
| `sidecar_request_duration_ms` | `run_id`, `endpoint` | Request latency per endpoint |
| `sidecar_intent_payload_bytes` | `run_id` | Intent payload size distribution |

When the `opentelemetry` feature is enabled, these metrics are
additionally exported via OTLP alongside existing void-box metrics.

### Diagnostic Events

The sidecar emits diagnostic events that void-box propagates to
void-control's event stream. These extend the existing
`EventEnvelope` contract:

| Event | Severity | When |
|-------|----------|------|
| `SidecarStarted` | info | Sidecar process spawned and listening |
| `SidecarReady` | info | First health check passed |
| `SidecarInboxLoaded` | info | Inbox snapshot accepted, ready to serve |
| `SidecarIntentRejected` | warn | Intent rejected (limit, size, rate) |
| `SidecarUnreachable` | error | Health check failed (existing, now formalized) |
| `SidecarStopped` | info | Clean shutdown, includes final buffer stats |
| `SidecarCrashed` | error | Unexpected termination, includes buffered intent count lost |
| `MessageDeliveryFailed` | error | Injection failed (existing, now formalized) |
| `MessageCollectionFailed` | warn | Drain failed (existing, now formalized) |

Event payloads follow the existing `BTreeMap<String, serde_json::Value>`
pattern and always include `run_id`, `candidate_id`, and `iteration`.

### Distributed Tracing

The sidecar participates in the execution's distributed trace.
void-box passes the trace context (W3C `traceparent` header) when
spawning the sidecar process. The sidecar creates child spans for:

- `sidecar.inbox.load` — inbox deserialization and storage
- `sidecar.intent.accept` — intent validation and buffering
- `sidecar.intent.drain` — bulk flush to daemon
- `sidecar.http.request` — per-request span (parent of the above)

This means a single execution trace shows: void-control planning →
void-box VM launch → sidecar inbox load → agent HTTP calls →
sidecar intent accept → void-control drain → normalize → route.

### Health Check Contract

`GET /v1/health` on the sidecar returns:

```json
{
  "status": "ok",
  "sidecar_version": "0.1.0",
  "run_id": "run-123",
  "buffer_depth": 2,
  "inbox_version": 3,
  "uptime_ms": 45200
}
```

The void-box daemon polls this endpoint periodically (suggested: every
5s for long-running, once after start for batch). A failed health check
triggers the `SidecarUnreachable` diagnostic event. Three consecutive
failures in long-running mode trigger `SidecarCrashed`.

The daemon exposes aggregate sidecar health via the existing
`GET /v1/runs/{run_id}` response, adding a `sidecar` field:

```json
{
  "run_id": "run-123",
  "state": "running",
  "sidecar": {
    "status": "ok",
    "buffer_depth": 2,
    "last_health_check_ms": 1200
  }
}
```

## Invariants

- Sidecar is the authoritative buffer for in-flight messages during
  candidate execution (ephemeral — not the durable source of truth).
- void-control never reaches into the VM to read/write messages.
- Agents never talk to void-control directly — only to the sidecar.
- Intent collection always goes through the adapter.
- The existing message-box pipeline (normalize, route, materialize) is
  unchanged.
- Provider bridges are optional optimizations, never requirements.
- Replay can reconstruct all state from persisted intents and routed
  messages.

## Implementation Order

1. HTTP sidecar in void-box (`/v1/inbox`, `/v1/intents`, `/v1/context`, `/v1/health`)
2. void-box daemon endpoints (`PUT /inbox`, `GET /intents`, `POST /messages`)
3. Skill injection for generic agents
4. `MessageDeliveryAdapter` trait in void-control with `HttpSidecarAdapter`
5. Integration with existing message-box pipeline (dual intent sources)
6. Claude Code channel bridge (provider-specific optimization)

## Non-Goals

- Replacing the message-box spec — this is the transport layer below it.
- Agent-to-agent direct communication — all messages route through
  control plane.
- Free-text payload parsing by strategies — signals remain metadata-only.
- Real-time streaming of intent state — collection is iteration-aligned.

## Future Considerations (v2+)

- **Message ACK model**: agents acknowledge message consumption. Useful
  for debugging, reliability, and advanced coordination. Not needed in v1
  because delivery is fire-and-forget at the sidecar level.
- **`/v1/signals` endpoint**: control-plane-derived signals (convergence
  state, planning hints) pushed to agents. Keeps the system symmetric:
  agents emit intents, control plane emits signals.
- **Streaming intent collection**: WebSocket or SSE from sidecar to
  void-box daemon for real-time intent propagation in long-running mode.
- **Agent-addressed routing**: extend `CommunicationIntentAudience`
  beyond `Leader | Broadcast` to support `@candidate_id`, `@role`,
  `@group` targeting.
