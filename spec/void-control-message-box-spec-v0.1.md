# Void Control Message Box Specification

## Version: v0.1

## Scope

This spec defines the first real control-plane message box for
collaboration between iterative candidates in `void-control`.

It extends:
- `spec/void-control-iteration-spec-v0.2.md`
- `spec/void-control-runtime-spec-v0.2.md`

It does not move collaboration ownership into `void-box`.

This specification is intentionally control-plane first:
- candidate runs may emit structured communication intents,
- `void-control` persists, routes, and delivers those intents,
- future inboxes are reconstructed from persisted routing state,
- delivery is deterministic and replayable.

This is not a free-form chat protocol. It is decision propagation
infrastructure for iterative execution modes such as `swarm` and
`search`.

---

# 1. Ownership Boundary

`void-control` owns:
- communication intent extraction from child run structured output,
- message validation and routing,
- persistence of intents, routed messages, and inbox snapshots,
- next-iteration inbox delivery,
- replay and restart reconstruction of collaboration state,
- safety limits such as TTL, deduplication, and fan-out caps.

`void-box` owns:
- execution of a single child run,
- durable publication of structured stage output artifacts,
- retrieval of `result.json` and any additional artifacts,
- runtime lifecycle and artifact publication status.

`void-box` MUST NOT:
- route candidate-to-candidate messages,
- persist execution-level collaboration state,
- infer control-plane audiences such as `leader` or `broadcast`,
- decide inbox delivery timing.

---

# 2. Core Idea

The system does not pass chat messages. It propagates decisions.

The control-plane collaboration flow is:

1. a candidate run emits structured communication intents,
2. `void-control` validates and persists those intents,
3. `void-control` routes intents into delivery messages,
4. `void-control` materializes inbox snapshots for a later iteration,
5. future candidates receive those inbox snapshots as launch input.

V0 delivery is strictly delayed:

`run -> reduce -> route -> next inbox`

Messages emitted in iteration `N` MUST NOT be delivered in iteration `N`.
They MAY be delivered in iteration `N + 1` or later if still valid.

---

# 3. Object Model

## 3.1 CommunicationIntent

`CommunicationIntent` is the canonical record of what a candidate tried
to communicate.

Suggested shape:

```json
{
  "intent_id": "intent_17",
  "from_candidate_id": "candidate-2",
  "iteration": 0,
  "kind": "proposal",
  "audience": "leader",
  "payload": {
    "summary_text": "Rate limit plus cache fallback reduced latency",
    "strategy_hint": "rate_limit_cache",
    "metric_deltas": {
      "latency_p99_ms": -30.0,
      "error_rate": -0.02
    }
  },
  "priority": "normal",
  "ttl_iterations": 1,
  "caused_by": null,
  "context": {
    "family_hint": "incident-mitigation"
  }
}
```

Required fields:
- `intent_id`
- `from_candidate_id`
- `iteration`
- `kind`
- `audience`
- `payload`
- `priority`
- `ttl_iterations`

Optional fields:
- `caused_by`
- `context`

Rules:
- `intent_id` MUST be unique within an `Execution`.
- `from_candidate_id` references the emitting candidate.
- `iteration` is the iteration in which the source run completed.
- `caused_by` references another `intent_id` when an intent is a direct
  refinement or response.
- `context` is advisory and strategy-defined. It is optional in v0.

## 3.2 RoutedMessage

`RoutedMessage` is the canonical record of a validated intent after
control-plane routing.

Suggested shape:

```json
{
  "message_id": "msg_44",
  "intent_id": "intent_17",
  "to": "leader",
  "delivery_iteration": 1,
  "routing_reason": "leader_feedback_channel",
  "status": "Routed"
}
```

Required fields:
- `message_id`
- `intent_id`
- `to`
- `delivery_iteration`
- `routing_reason`
- `status`

Rules:
- one intent MAY produce zero, one, or many routed messages,
- `delivery_iteration` MUST be greater than the source `iteration`,
- `status` lifecycle is defined in Section 5.

## 3.3 InboxEntry

`InboxEntry` is the unit delivered to a future candidate.

Suggested shape:

```json
{
  "message_id": "msg_44",
  "intent_id": "intent_17",
  "from_candidate_id": "candidate-2",
  "kind": "proposal",
  "payload": {
    "summary_text": "Rate limit plus cache fallback reduced latency",
    "strategy_hint": "rate_limit_cache"
  }
}
```

Required fields:
- `message_id`
- `intent_id`
- `from_candidate_id`
- `kind`
- `payload`

## 3.4 InboxSnapshot

`InboxSnapshot` is the persisted record of what a candidate actually
received at launch time.

Suggested shape:

```json
{
  "execution_id": "exec_1",
  "candidate_id": "candidate-3",
  "iteration": 1,
  "entries": [
    {
      "message_id": "msg_44",
      "intent_id": "intent_17",
      "from_candidate_id": "candidate-2",
      "kind": "proposal",
      "payload": {
        "summary_text": "Rate limit plus cache fallback reduced latency"
      }
    }
  ]
}
```

Rules:
- this is the source of truth for delivered inbox content,
- replay MUST prefer persisted inbox snapshots over re-deriving launch
  input from current routing rules,
- a candidate with no messages MAY still have an empty inbox snapshot.

---

# 4. Intent Semantics

## 4.1 Intent kinds

V0 supports exactly three intent kinds:
- `proposal`
- `signal`
- `evaluation`

### `proposal`

Use when a candidate recommends a change, mitigation, or next step.

Examples:
- try `rate_limit_cache`
- lower prompt verbosity
- switch transform to streaming mode

### `signal`

Use when a candidate reports a condition that should influence routing or
planning.

Examples:
- anomaly detected
- retry strategy caused instability
- current family appears saturated

### `evaluation`

Use when a candidate reports structured feedback about a prior proposal or
family of proposals.

Examples:
- rate limiting improved latency but hurt success rate
- candidate family `friendly_structured` scored highest

## 4.2 Payload rules

Payloads MUST remain structured.

V0 payload shape:

```json
{
  "summary_text": "short advisory summary",
  "strategy_hint": "optional short stable hint",
  "metric_deltas": {
    "latency_p99_ms": -30.0
  },
  "recommendation": "optional concise action string"
}
```

Rules:
- `summary_text` is required,
- `strategy_hint` is optional but recommended,
- `metric_deltas` is optional,
- unknown fields MAY be allowed for forward compatibility,
- large free-form prose blobs SHOULD be rejected or truncated.

## 4.3 Audience rules

V0 supported audiences:
- `leader`
- `broadcast`

`candidate:<id>` addressing is deferred.

Rules:
- `leader` means the intent should be routed only to the leader inbox or
  equivalent strategy-specific supervisory role,
- `broadcast` means the router MAY fan the message out to multiple future
  inboxes subject to safety limits.

---

# 5. Message Lifecycle

`RoutedMessage.status` enum:

`Routed | Delivered | Expired | Dropped`

Definitions:
- `Routed`: message has been created by the router but not yet included in
  a persisted inbox snapshot.
- `Delivered`: message was included in at least one inbox snapshot.
- `Expired`: message was valid when routed but exceeded its TTL before
  delivery.
- `Dropped`: message was rejected by routing or delivery rules such as
  deduplication, policy cap, or invalid audience.

`Consumed` is intentionally deferred in v0 because the control plane cannot
reliably prove semantic use by a child run.

---

# 6. Delivery Rules

## 6.1 Timing

V0 delivery MUST be next-iteration or later only.

For an intent emitted in iteration `N`:
- `delivery_iteration` MUST be `>= N + 1`
- same-iteration delivery is forbidden

## 6.2 TTL

`ttl_iterations` is measured in control-plane iterations.

Rules:
- default TTL SHOULD be `1`,
- a message expires when `current_iteration > source_iteration + ttl`,
- expired messages MUST transition to `Expired`,
- expired messages MUST NOT appear in new inbox snapshots.

## 6.3 Deduplication

The router MUST support deduplication.

Recommended dedup key:
- normalized payload
- audience
- source iteration

Rules:
- identical messages SHOULD NOT fan out repeatedly within the same
  iteration,
- deduped messages SHOULD produce `Dropped` routed message records or
  equivalent traceable diagnostics.

## 6.4 Fan-out limits

The router MUST bound amplification.

V0 recommended defaults:
- max `3` intents per candidate per iteration,
- max `1` broadcast intent per candidate per iteration,
- bounded recipient count for a single broadcast,
- bounded payload size.

## 6.5 Default retention and disk limits

V0 SHOULD define conservative defaults to prevent unbounded disk growth.

Recommended default policy:

```json
{
  "message_box": {
    "retention": {
      "completed_days": 7,
      "failed_days": 14,
      "canceled_days": 3
    },
    "limits": {
      "max_intent_payload_bytes": 4096,
      "max_inbox_snapshot_bytes": 65536,
      "max_intents_per_candidate_per_iteration": 3,
      "max_broadcast_intents_per_candidate_per_iteration": 1,
      "max_execution_message_box_bytes": 10485760
    }
  }
}
```

Rules:
- active executions MUST NOT be cleaned up,
- terminal executions MAY be cleaned up only after their retention window,
- if message-box artifacts for a single execution exceed
  `max_execution_message_box_bytes`, the control plane SHOULD reject or
  drop further intents for that execution and emit a warning event,
- failed executions retain logs longer than completed executions because
  they have higher debugging value.

---

# 7. Persistence Model

The control plane MUST persist three distinct records:

1. `intents.log`
- append-only raw emitted intents

2. `messages.log`
- append-only routed messages and status changes

3. `inboxes/<iteration>/<candidate>.json`
- exact delivered inbox snapshots

This separation is required so the system can answer:
- what was emitted,
- what was routed,
- what was actually delivered.

---

# 8. Event Model

The current control-plane event enum is not sufficient to represent
message-box semantics on its own.

V0 SHOULD introduce additional collaboration events:
- `CommunicationIntentEmitted`
- `CommunicationIntentRejected`
- `MessageRouted`
- `MessageDelivered`
- `MessageExpired`
- `MessageDropped`

Rules:
- event logs MAY stay lightweight and refer to IDs rather than carrying
  full payload bodies,
- full payload data MUST remain available in the persisted intent/message
  records,
- replay MAY combine event logs with persisted message-box records.

---

# 9. Execution Output Contract

Candidate runs emit intents through structured output consumed by
`void-control`.

Suggested extension to orchestration-facing `result.json`:

```json
{
  "status": "ok",
  "summary": "candidate finished successfully",
  "metrics": {
    "latency_p99_ms": 72
  },
  "intents": [
    {
      "kind": "proposal",
      "audience": "leader",
      "payload": {
        "summary_text": "Rate limit plus cache fallback reduced latency",
        "strategy_hint": "rate_limit_cache"
      },
      "priority": "normal",
      "ttl_iterations": 1
    }
  ]
}
```

Rules:
- `intents` is optional,
- invalid intents MUST NOT make a successful candidate output
  unrecoverable if metrics are otherwise valid,
- invalid intents SHOULD produce rejection diagnostics and
  `CommunicationIntentRejected`.

---

# 10. Strategy Interaction

The message box is generic transport. Strategies still own collaboration
semantics.

## 10.1 `swarm`

Expected v0 usage:
- candidates emit `proposal` and `signal` intents,
- `broadcast` and `leader` audiences are common,
- router fans out a bounded set of messages into later inboxes,
- inboxes influence future broad exploration.

## 10.2 `search`

Expected v0 usage:
- candidates emit `evaluation` and `proposal` intents,
- most messages route to `leader` or the next refiner role,
- `caused_by` is especially useful to express refinement lineage.

---

# 11. Provider Adapter Boundary

The collaboration protocol is owned by `void-control`, but message
delivery into a concrete model runtime is provider-specific.

V0 therefore introduces a provider adapter abstraction.

## 11.1 Responsibilities

`void-control` owns:
- canonical `CommunicationIntent`, `RoutedMessage`, and `InboxSnapshot`
  semantics,
- routing and delivery timing,
- persistence and replay,
- selection of which inbox snapshot belongs to a launched candidate.

The provider adapter owns:
- translation of an `InboxSnapshot` into provider-specific launch input,
- any provider-specific formatting or prompt shaping,
- optional future live-delivery optimization when supported by a
  provider runtime.

## 11.2 Adapter contract

Suggested conceptual interface:

```rust
trait CollaborationAdapter {
    fn prepare_launch_input(
        &self,
        candidate: &CandidateSpec,
        inbox: &InboxSnapshot,
    ) -> ProviderLaunchInput;
}
```

Optional future capability:

```rust
trait LiveCollaborationAdapter {
    fn supports_live_delivery(&self) -> bool;
    fn deliver_live_message(
        &self,
        runtime_handle: &str,
        message: &RoutedMessage,
    ) -> Result<(), AdapterError>;
}
```

V0 MUST NOT require live delivery.

## 11.3 Delivery modes

V0 supported conceptual delivery modes:

### LaunchInjection

The inbox snapshot is rendered into the initial candidate prompt/input.

Properties:
- universal fallback,
- deterministic,
- replay-friendly,
- provider-neutral.

This is the required v0 mode.

### StructuredContext

The inbox snapshot is translated into provider-native structured context
or resource attachments when supported.

Properties:
- cleaner than raw prompt injection,
- still launch-time only,
- optional in v0.

### LiveChannel

The provider may support live side-channel delivery after launch.

Examples:
- broker/channel injection,
- MCP-backed session message delivery,
- provider-local streaming collaboration APIs.

Properties:
- optional optimization only,
- MUST preserve the same canonical `InboxSnapshot` and routing semantics,
- MUST NOT replace the control-plane source of truth.

## 11.4 Fallback rule

All provider adapters MUST support deterministic launch-time delivery.

If a provider-specific live or structured delivery mode is unavailable,
`void-control` MUST fall back to `LaunchInjection` without changing the
collaboration semantics.

## 11.5 Canonical truth

The canonical collaboration truth is:
- routed messages in the control plane,
- persisted inbox snapshots,
- the launch-time candidate input derived from those snapshots.

Provider-specific delivery channels are implementation details. They are
not the primary record of collaboration state.

---

# 12. Replay and Restart Semantics

The message box MUST be replay-safe.

Required guarantees:
- persisted inbox snapshots are immutable historical facts,
- routed message status is reconstructible after restart,
- delivery decisions are deterministic given persisted state,
- replay does not require re-reading raw runtime logs once intents have
  already been extracted and persisted.

---

# 13. Out of Scope for v0

Deferred items:
- same-iteration delivery,
- direct candidate-to-candidate addressing,
- thread-like conversations,
- arbitrary free-form chat,
- semantic `Consumed` tracking,
- UI conversation views,
- runtime-side message routing inside `void-box`,
- provider-required live delivery.

---

# 14. Acceptance Criteria

V0 is acceptable when all of the following are true:

1. a candidate can emit one or more valid structured intents in
   `result.json`
2. `void-control` persists those intents in `intents.log`
3. `void-control` routes valid intents into `messages.log`
4. `void-control` materializes persisted inbox snapshots for a later
   iteration
5. delivered inbox snapshots are reconstructible after restart
6. duplicate or oversized intents are rejected or dropped deterministically
7. expired intents do not appear in new inbox snapshots
8. a `swarm` acceptance test proves backlog/inbox delivery semantics using
   real routed message records
9. a `search` acceptance test proves refinement lineage using
   `caused_by`-linked intents
10. at least one provider adapter delivers inbox snapshots through
    deterministic launch-time injection

---

# 15. Recommended Implementation Order

1. extend the structured output contract to allow `intents`
2. add `CommunicationIntent` parsing and validation
3. persist `intents.log`
4. add routing for `leader` and `broadcast`
5. persist `messages.log`
6. add provider adapter abstraction with required `LaunchInjection`
   support
7. persist `InboxSnapshot`
8. add control-plane collaboration events
9. add integration tests for:
   - valid intent emission and delivery
   - TTL expiry
   - deduplication
   - restart replay
   - launch-time provider delivery

This order keeps the boundary stable:
- `void-box` only returns structured output,
- `void-control` owns the message bus.
