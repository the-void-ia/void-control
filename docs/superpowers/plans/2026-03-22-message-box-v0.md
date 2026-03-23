# Message Box V0 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:executing-plans or
> superpowers:subagent-driven-development to implement this plan. Steps
> use checkbox syntax for tracking.

**Goal:** Implement the first real `void-control` message box:
- structured communication intents in orchestration-facing results,
- persisted `intents.log`, `messages.log`, and inbox snapshots,
- deterministic next-iteration delivery,
- provider-adapter launch injection as the required v0 delivery mode.

**Architecture:** Keep collaboration semantics owned by `void-control`.
Extend the orchestration model with message-box records and a provider
adapter abstraction. Do not move routing into `void-box`. Do not require
live vendor channels in v0.

**Tech Stack:** Rust 2021, existing orchestration/runtime/store modules,
filesystem-backed execution persistence, current `MockRuntime`,
integration tests under `tests/`.

---

## Scope Check

This plan includes:
1. message-box domain types and persistence,
2. structured intent extraction from candidate output,
3. routing for `leader` and `broadcast`,
4. inbox snapshot materialization,
5. provider adapter abstraction with launch injection,
6. integration acceptance tests for emission, routing, delivery, replay.

This plan intentionally excludes:
- direct `candidate:<id>` addressing,
- same-iteration delivery,
- semantic consumed-tracking,
- provider-required live delivery,
- UI message-thread rendering.

## File Map

### Primary files

- Modify: `src/orchestration/types.rs`
  Responsibility: add message-box domain types and persisted records.
- Modify: `src/orchestration/events.rs`
  Responsibility: add collaboration event types.
- Modify: `src/orchestration/service.rs`
  Responsibility: extract intents, route messages, persist inboxes, and
  invoke provider adapter launch injection.
- Modify: `src/orchestration/store/fs.rs`
  Responsibility: persist `intents.log`, `messages.log`, and inbox
  snapshots.
- Modify: `src/orchestration/mod.rs`
  Responsibility: export new message-box types.
- Create or modify: `src/orchestration/message_box.rs`
  Responsibility: routing, TTL, dedup, and inbox materialization logic.
- Create or modify: `src/runtime/mod.rs`
  Responsibility: provider adapter abstraction boundary.

### Tests

- Create: `tests/execution_message_box.rs`
  Responsibility: focused integration coverage for emission, routing,
  delivery, TTL expiry, dedup, and replay.
- Modify: `tests/strategy_scenarios.rs`
  Responsibility: upgrade swarm/search scenarios to use real routed
  message records instead of only backlog shaping.
- Modify: `tests/execution_strategy_acceptance.rs`
  Responsibility: require provider launch-injection delivery path.

## Delivery Strategy

Implement in this order:

1. define domain types and persistence,
2. add provider adapter abstraction with launch injection,
3. extract intents from structured output,
4. route and persist messages,
5. persist inbox snapshots,
6. add replay/integration tests,
7. then upgrade scenario coverage.

This keeps the control-plane truth stable while allowing provider
delivery to remain a thin adapter.

## Chunk 1: Domain Model and Persistence

### Task 1: Add message-box records

**Files:**
- Modify: `src/orchestration/types.rs`
- Modify: `src/orchestration/store/fs.rs`
- Modify: `src/orchestration/mod.rs`
- Test: `tests/execution_message_box.rs`

- [ ] Step 1: add failing persistence tests for intents/messages/inboxes
- [ ] Step 2: add `CommunicationIntent`, `RoutedMessage`,
  `InboxEntry`, `InboxSnapshot`
- [ ] Step 3: persist `intents.log` and `messages.log` as NDJSON
- [ ] Step 4: persist inbox snapshots as JSON files
- [ ] Step 5: run focused persistence tests

## Chunk 2: Provider Adapter V0

### Task 2: Add launch-injection adapter boundary

**Files:**
- Modify: `src/runtime/mod.rs`
- Modify: `src/orchestration/service.rs`
- Test: `tests/execution_message_box.rs`

- [ ] Step 1: add failing test for provider launch injection
- [ ] Step 2: add provider adapter trait with required launch injection
- [ ] Step 3: add default adapter that renders inbox snapshot into launch input
- [ ] Step 4: wire service launch path through the adapter
- [ ] Step 5: run focused adapter test

## Chunk 3: Intent Extraction and Routing

### Task 3: Extract intents from candidate output and route them

**Files:**
- Modify: `src/orchestration/service.rs`
- Create or modify: `src/orchestration/message_box.rs`
- Modify: `src/orchestration/events.rs`
- Test: `tests/execution_message_box.rs`

- [ ] Step 1: add failing test for valid intent emission
- [ ] Step 2: parse `intents` from structured candidate output
- [ ] Step 3: validate kind/audience/limits
- [ ] Step 4: persist valid intents and rejection diagnostics
- [ ] Step 5: route `leader` and `broadcast` messages
- [ ] Step 6: append collaboration events
- [ ] Step 7: run focused routing tests

## Chunk 4: Inbox Materialization and Replay

### Task 4: Deliver inbox snapshots deterministically

**Files:**
- Modify: `src/orchestration/message_box.rs`
- Modify: `src/orchestration/service.rs`
- Modify: `src/orchestration/store/fs.rs`
- Test: `tests/execution_message_box.rs`

- [ ] Step 1: add failing test for next-iteration inbox delivery
- [ ] Step 2: materialize inbox snapshots from routed messages
- [ ] Step 3: enforce TTL, dedup, and fan-out limits
- [ ] Step 4: persist immutable inbox snapshots
- [ ] Step 5: replay from persisted logs plus snapshots after restart
- [ ] Step 6: run delivery/replay tests

## Chunk 5: Scenario Upgrade and Acceptance

### Task 5: Use real message-box flow in strategy scenarios

**Files:**
- Modify: `tests/strategy_scenarios.rs`
- Modify: `tests/execution_strategy_acceptance.rs`

- [ ] Step 1: upgrade swarm scenario to assert real routed message records
- [ ] Step 2: upgrade search scenario to assert `caused_by` lineage
- [ ] Step 3: ensure supported-strategy acceptance uses provider launch injection
- [ ] Step 4: run:

```bash
cargo test --features serde --test execution_message_box -- --nocapture
cargo test --features serde --test strategy_scenarios -- --nocapture
cargo test --features serde --test execution_strategy_acceptance -- --nocapture
cargo test --features serde
```

Expected: PASS.
