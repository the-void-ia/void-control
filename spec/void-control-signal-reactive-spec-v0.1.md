# Void Control Signal-Reactive Planning Specification

## Version: v0.1

## Scope

This specification defines the first planning-semantics layer above the
message box in `void-control`.

It extends:
- `spec/void-control-message-box-spec-v0.1.md`
- `spec/void-control-iteration-spec-v0.2.md`

It introduces:
- a metadata-driven planning input called `MessageStats`,
- a dedicated signal-extraction layer between message routing and
  strategy planning,
- a new planning mode called `signal_reactive`,
- v1 planning reactions for `swarm` and `search`.

This specification does not introduce:
- free-text payload parsing by strategies,
- direct candidate-directed routing,
- full semantic `Signal` objects yet,
- direct execution mutation from messages.

---

# 1. Core Idea

Messages are transport.

Signals are planning input.

Strategies do not read raw messages. They react to normalized planning
inputs derived from message metadata.

The control-plane flow becomes:

1. candidates emit `CommunicationIntent`s,
2. the message box validates, routes, and persists them,
3. signal extraction derives `MessageStats` from intent and routing
   metadata,
4. iterative strategies consume `MessageStats` when planning the next
   candidates,
5. candidates still receive full inbox payloads for their own reasoning.

This preserves:
- determinism,
- replayability,
- provider neutrality,
- content-blind strategy behavior.

---

# 2. Ownership Boundary

## `void-control` owns

- signal extraction from persisted message-box state,
- normalization of message metadata into `MessageStats`,
- strategy-specific interpretation of `MessageStats`,
- planning bias derived from message patterns.

## candidates own

- interpretation of inbox payload content,
- free-text reasoning,
- optional emission of new intents.

## strict rule

Strategies MUST NOT:
- parse free-text payload content,
- read raw inbox payloads directly,
- infer planning meaning from message text,
- treat intents as imperative commands.

Strategies MAY:
- consume `MessageStats`,
- react to delivery counts, audience mix, priority mix, TTL expiry,
  drops, and source diversity,
- adjust candidate generation heuristics based on those normalized
  signals.

---

# 3. New Mode: `signal_reactive`

`signal_reactive` is a new planning mode for metadata-driven planning.

It is not a semantic alias for `leader_directed`.

Reason:
- `leader_directed` described leader-authored candidate override
  proposals,
- `signal_reactive` describes planner reaction to message metadata
  patterns,
- these are related collaboration mechanisms, but they are not the same
  control-plane behavior.

`signal_reactive` means:
- candidate generation may be biased by aggregated communication
  patterns,
- no raw payload content is consumed,
- no direct override extraction occurs in v1.

`leader_directed` remains the legacy name for the older
payload-authored override model described in the iteration
specification.

New executions that use metadata-only planning SHOULD use
`signal_reactive`.

---

# 4. Layered Model

The collaboration stack becomes:

| Layer | Consumes | Produces |
|-------|----------|----------|
| Message box | Raw intents | Routed messages, inbox snapshots |
| Signal extraction | Intent metadata, routed-message metadata | `MessageStats` |
| Strategy | `MessageStats` | Candidate specs |
| Candidate runtime | Inbox snapshots with full payload | New intents |

Important rule:

- message payload is for candidates,
- message metadata is for control-plane,
- `MessageStats` is the only v1 planning input produced by the signal
  extraction layer.

---

# 5. `MessageStats`

## 5.1 Purpose

`MessageStats` is the v1 normalized planning summary for one execution at
one planning step.

It is intentionally small, deterministic, content-blind, and
routed-message based.

## 5.2 Suggested shape

```json
{
  "iteration": 1,
  "total_messages": 6,
  "leader_messages": 2,
  "broadcast_messages": 4,
  "proposal_count": 3,
  "signal_count": 2,
  "evaluation_count": 1,
  "high_priority_count": 2,
  "normal_priority_count": 4,
  "low_priority_count": 0,
  "delivered_count": 6,
  "dropped_count": 1,
  "expired_count": 0,
  "unique_sources": 3,
  "unique_intent_count": 5
}
```

Illustrative Rust shape:

```rust
struct MessageStats {
    iteration: u32,
    total_messages: usize,
    leader_messages: usize,
    broadcast_messages: usize,
    proposal_count: usize,
    signal_count: usize,
    evaluation_count: usize,
    high_priority_count: usize,
    normal_priority_count: usize,
    low_priority_count: usize,
    delivered_count: usize,
    dropped_count: usize,
    expired_count: usize,
    unique_sources: usize,
    unique_intent_count: usize,
}
```

## 5.3 Required fields

- `iteration`
- `total_messages`
- `leader_messages`
- `broadcast_messages`
- `proposal_count`
- `signal_count`
- `evaluation_count`
- `high_priority_count`
- `normal_priority_count`
- `low_priority_count`
- `delivered_count`
- `dropped_count`
- `expired_count`
- `unique_sources`
- `unique_intent_count`

## 5.4 Derived ratios

Implementations MAY derive ratios from `MessageStats`, for example:

- `broadcast_ratio = broadcast_messages / max(total_messages, 1)`
- `proposal_ratio = proposal_count / max(total_messages, 1)`
- `priority_pressure = high_priority_count / max(total_messages, 1)`

Ratios are derived convenience values. They do not need to be persisted
as first-class fields in v0.1.

---

# 6. Signal Extraction Layer

## 6.1 Required seam

Signal extraction MUST exist as a dedicated control-plane step between
message routing and strategy planning.

Suggested interface:

```rust
fn extract_message_stats(
    intents: &[CommunicationIntent],
    routed_messages: &[RoutedMessage],
    delivery_iteration: u32,
) -> MessageStats
```

Illustrative implementation skeleton:

```rust
fn extract_message_stats(
    intents: &[CommunicationIntent],
    routed_messages: &[RoutedMessage],
    delivery_iteration: u32,
) -> MessageStats {
    // Join routed messages back to source intent metadata by intent_id.
    // Count only routed-message outcomes for this planning iteration.
    // Do not inspect payload text.
    todo!()
}
```

This layer:
- reads message metadata only,
- does not parse payload content,
- may join routed messages to source intent metadata by `intent_id`,
- is deterministic from persisted state,
- is replayable after restart.

## 6.2 Source material

`MessageStats` may use:
- routed-message destination
- routed-message `status`
- routed-message delivery iteration
- routed-message source intent metadata joined via `intent_id`
- TTL outcome effects such as `Expired`
- delivery/dedup/drop outcomes from control-plane

`MessageStats` MUST NOT use:
- `payload.summary_text`
- free-text content
- provider-specific delivery transport details

---

# 7. Strategy Consumption Rules

## 7.1 Shared rule

All iterative strategies MUST treat `MessageStats` as advisory evidence,
not imperative commands.

Messages do not directly mutate execution.

They shape the search space.

## 7.2 `swarm`

`swarm` SHOULD use `MessageStats` to adjust breadth and convergence
pressure.

Examples:
- higher `proposal_count` and higher `unique_sources` MAY increase
  exploration budget or preserve breadth,
- higher `broadcast_messages` MAY increase convergence bias,
- higher `dropped_count` or `expired_count` MAY reduce fan-out pressure
  or exploration aggressiveness,
- higher `leader_messages` MAY shift some budget toward refinement-like
  candidates while preserving diversity.

`swarm` MUST NOT:
- derive exact override patches from message payload,
- collapse exploration solely from raw message count.

## 7.3 `search`

`search` SHOULD use `MessageStats` to adjust refinement aggressiveness.

If a `search` execution emits no communication intents, `MessageStats`
for that iteration is simply zero-biased input and `search` falls back
to incumbent-centered planning.

Examples:
- more `evaluation_count` than `proposal_count` MAY increase exploitation
  pressure,
- higher `signal_count` MAY preserve a small exploration quota,
- higher `leader_messages` MAY allow one additional refinement iteration
  before declaring plateau,
- higher `expired_count` or `dropped_count` MAY reduce planner confidence
  and avoid over-committing to a refinement path.

`search` MUST remain incumbent-centered.

`MessageStats` may bias refinement, but MUST NOT replace incumbent-based
planning.

---

# 8. TTL, Dedup, Drops, and Planning Semantics

These are not only storage concerns. They are learning-dynamics controls.

## TTL

TTL controls memory horizon.

- short TTL means reactive behavior,
- longer TTL means more persistent planning evidence.

## Dedup

Dedup controls signal compression.

- repeated similar communication should not create unbounded planner
  noise,
- dedup MAY still contribute to stronger aggregate counts or repeated
  delivery evidence.

## Drops

Drops indicate overload or bounded suppression.

High drop counts MAY cause strategies to:
- reduce exploration pressure,
- reduce broadcast-heavy behavior,
- preserve budget for higher-priority patterns.

## Expiry

Expiry indicates stale information.

High expiry counts MAY reduce confidence in long-lived communication
patterns.

---

# 9. What V1 Does Not Do

V1 does not include:
- arbitrary override extraction from payload,
- strategy parsing of message text,
- direct spawn/cancel/suppress commands from intents,
- candidate-targeted routing,
- full semantic `Signal` objects like `ProposalCluster` yet.

Those may be added later in a future specification after the
`MessageStats` seam is stable.

---

# 10. V1.5 / V2 Direction

The future evolution path is:

1. `MessageStats` in v1,
2. controller-derived structured `Signal` objects later,
3. typed candidate proposal objects or validated override hints if
   needed.

Important future rule:

- strategies should react to patterns first,
- then to controller-derived structured meaning,
- never to raw messages.

Illustrative later-stage semantic layer:

```rust
enum Signal {
    ProposalCluster { topic: String },
    ImprovementTrend { topic: String },
    RegressionTrend { topic: String },
}

fn plan_with_signals(signals: &[Signal]) {
    for signal in signals {
        match signal {
            Signal::ProposalCluster { topic } => bias_topic(topic),
            Signal::ImprovementTrend { topic } => reinforce(topic),
            Signal::RegressionTrend { topic } => penalize(topic),
        }
    }
}
```

---

# 11. Acceptance Criteria

An implementation conforms to this specification if:

1. `signal_reactive` exists as a distinct metadata-driven planning mode,
2. a dedicated signal-extraction layer exists,
3. `MessageStats` is derived from persisted message metadata only,
4. `swarm` planning can react to `MessageStats`,
5. `search` planning can react to `MessageStats`,
6. replay can reconstruct `MessageStats` from persisted state,
7. no strategy consumes free-text payload directly,
8. message transport and candidate inbox delivery continue to work
   independently of strategy planning semantics.

---

# 12. Non-Goals

This specification is not:
- a replacement for the message box,
- a replacement for scoring/evaluation,
- a direct-command protocol,
- a free-form chat coordination system.

It is the first deterministic planning-semantics layer above the message
transport.

---

# 13. Derivation Semantics

## 13.1 Planning window

`MessageStats` is derived for exactly one planning step.

The planning window SHOULD be:
- all routed-message outcomes relevant to iteration `N` planning,
- primarily routed messages with `delivery_iteration = N`.

This keeps signal extraction aligned with actual planner input instead
of raw historical message volume.

## 13.2 Field interpretation

Suggested v0.1 interpretation:

- `iteration`: the planning iteration for which stats are derived,
- `total_messages`: count of routed messages considered in the planning
  window,
- `leader_messages`: routed messages whose destination is `leader`,
- `broadcast_messages`: routed messages whose destination is
  `broadcast`,
- `proposal_count`: routed messages whose source intent kind is
  `proposal`,
- `signal_count`: routed messages whose source intent kind is `signal`,
- `evaluation_count`: routed messages whose source intent kind is
  `evaluation`,
- `high_priority_count`: routed messages whose source intent priority is
  `high`,
- `normal_priority_count`: routed messages whose source intent priority is
  `normal`,
- `low_priority_count`: routed messages whose source intent priority is
  `low`,
- `delivered_count`: routed messages with status `Delivered`,
- `dropped_count`: routed messages with status `Dropped`,
- `expired_count`: routed messages with status `Expired`,
- `unique_sources`: distinct `from_candidate_id` values represented in
  the window,
- `unique_intent_count`: distinct `intent_id` values represented in the
  window.

The canonical counting unit in v0.1 is the routed message, not the raw
intent. Intent-oriented distinctness is captured only via
`unique_intent_count`.

## 13.3 Invariants

Implementations SHOULD maintain the following invariants:

- `proposal_count + signal_count + evaluation_count = total_messages`
- `high_priority_count + normal_priority_count + low_priority_count =
  total_messages`
- `leader_messages + broadcast_messages = total_messages`
- `delivered_count + dropped_count + expired_count <= total_messages`
- `unique_sources <= total_messages`
- `unique_intent_count <= total_messages`

These are sanity rules for deterministic extraction, not additional
persisted state requirements.

## 13.4 Dedup accounting

When dedup suppresses repeated routed messages, implementations MUST
count only the persisted post-dedup routed outcomes.

Dedup pressure may appear indirectly through lower delivered counts and
higher dropped counts when those outcomes are persisted.

---

# 14. Persistence and Replay

## 14.1 Source of truth

The source of truth remains:
- persisted `CommunicationIntent` records,
- persisted `RoutedMessage` records,
- persisted inbox snapshots when materialized.

`MessageStats` is a controller-derived view over that persisted state.

## 14.2 Persistence options

V0.1 permits either:
- recomputing `MessageStats` on demand from persisted message-box state,
  or
- persisting a cached `MessageStats` snapshot per planning iteration.

If persisted, the snapshot MUST be treated as derived data and SHOULD
include:
- `execution_id`,
- `iteration`,
- extractor version or schema version,
- the `MessageStats` payload.

## 14.3 Replay rule

After restart, the controller MUST be able to derive the same
`MessageStats` for the same execution state without consulting candidate
payload text.

If a cached `MessageStats` snapshot disagrees with recomputation, the
implementation SHOULD:
- prefer recomputation from canonical persisted message-box state,
- emit a diagnostic event,
- avoid silently feeding inconsistent planning input into strategies.

---

# 15. Planner Integration Contract

## 15.1 Execution-spec shape

For iterative planning modes that use this behavior, the variation
source SHOULD be expressed as:

- `signal_reactive`

Suggested `ExecutionSpec` fragment:

```json
{
  "variation": {
    "source": "signal_reactive",
    "candidates_per_iteration": 3
  }
}
```

This source means:
- candidate variation is planner-generated,
- planner bias may depend on `MessageStats`,
- raw message payload remains inaccessible to the planner.

This specification amends the variation-source set from the iteration
specification for signal-reactive planning:
- `signal_reactive` is the valid planning mode name for new executions,
- `leader_directed` remains the legacy mode for payload-authored
  candidate proposals.

## 15.2 Planner seam

Suggested strategy-facing interface:

```rust
fn plan_candidates(
    execution: &Execution,
    iteration: &Iteration,
    stats: &MessageStats,
) -> Vec<CandidateSpec>
```

Illustrative planner hook:

```rust
fn plan_with_message_stats(stats: &MessageStats) {
    if stats.proposal_count > 3 && stats.unique_sources > 2 {
        increase_exploration();
    }

    if stats.broadcast_messages > stats.leader_messages {
        bias_convergence();
    }

    if stats.dropped_count > 0 || stats.expired_count > 0 {
        reduce_fanout_pressure();
    }

    if stats.evaluation_count > stats.proposal_count {
        bias_refinement();
    }
}
```

`MessageStats` joins existing planning inputs such as:
- execution policy,
- prior scores and rankings,
- iteration history,
- candidate provenance.

It does not replace them.

## 15.3 Candidate input boundary

The planner and the candidate runtime consume different views:

- planner: `MessageStats`,
- candidate: inbox snapshot with full structured payload.

This split MUST remain explicit in code structure and persisted data
flow.

---

# 16. Compatibility and Migration

## 16.1 Config migration

Existing configurations that reference `leader_directed` SHOULD migrate
to `signal_reactive`.

V0.1-compatible implementations MAY support a temporary compatibility
mapping:

- rewrite new submission-time configuration from `leader_directed` to
  `signal_reactive`,
- emit a deprecation warning,
- require that such rewritten executions use metadata-only planning and
  do not parse payload-authored candidate override directives.

This mapping applies only to new execution submission or config parsing.
It MUST NOT rewrite the persisted `variation.source` of an already
created execution.

## 16.2 Iteration-spec alignment

Any section of the iteration specification that describes
`leader_directed` as payload-authored variation remains applicable to
legacy executions whose persisted `variation.source` is
`leader_directed`.

The key semantic shift is:
- before: leader output proposed concrete candidate overrides,
- now: controller derives metadata signals and the strategy plans
  candidates from those signals plus normal execution history.

This specification does not redefine `leader_directed`.
It introduces `signal_reactive` alongside it.

The older iteration-spec description of `candidate.message`, `@`
mentions, and free-text message bodies should be read as legacy
pre-message-box transport language. The canonical communication model
for signal-reactive planning is the structured `CommunicationIntent`
defined by the message-box specification.

## 16.3 Backward-compatibility constraint

Migration to `signal_reactive` MUST NOT break:
- message-box persistence,
- inbox delivery semantics,
- candidate ability to emit structured intents,
- replay of pre-existing executions under their originally persisted
  `variation.source` and routing state.

## 16.4 Future explicit override mechanism

If explicit candidate proposal behavior is still needed, it SHOULD be
specified as a separate future mechanism rather than folded back into
`signal_reactive`.

That future mechanism SHOULD:
- use typed candidate proposal objects,
- remain controller-validated,
- make override authority explicit,
- stay distinct from metadata-driven signaling.

---

# 17. Observability

Implementations SHOULD emit control-plane diagnostics that make signal
reactivity inspectable.

Suggested event/data points:
- `MessageStatsDerived`,
- extractor version,
- iteration number,
- key counts and ratios,
- whether stats were recomputed or loaded from cache,
- any replay mismatch between cached and recomputed stats.

This is important because strategy behavior will otherwise appear
opaque: the planner changes, but the underlying reason remains hidden.
