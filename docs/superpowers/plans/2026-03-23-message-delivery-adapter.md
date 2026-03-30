# Message Delivery Adapter Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `MessageDeliveryAdapter` support to `void-control` so it can inject inbox snapshots through `void-box` daemon sidecar endpoints and merge sidecar-drained intents with structured output intents.

**Architecture:** This plan assumes the required `void-box` work is already complete: per-run sidecar transport exists, and the daemon exposes the sidecar-facing endpoints described in the architecture spec. `void-control` keeps the existing `ProviderLaunchAdapter` path for compatibility, introduces a new sidecar-oriented delivery adapter alongside it, and switches behavior only when a delivery adapter is configured. The existing message-box pipeline remains authoritative: sidecar transport is an additional intent source, not a replacement for routing, persistence, or replay.

**Tech Stack:** Rust, serde/serde_json, existing TCP HTTP transport patterns from `VoidBoxRuntimeClient`

**Spec:** `/home/diego/github/void-control/docs/superpowers/specs/2026-03-23-message-delivery-architecture-design.md`

**Assumption:** `void-box` already implements the required daemon endpoints and sidecar lifecycle. This plan does not implement those endpoints.

---

## File Map

- Create: `src/runtime/delivery.rs`
- Create: `src/runtime/http_sidecar.rs`
- Modify: `src/runtime/mod.rs`
- Modify: `src/runtime/void_box.rs`
- Modify: `src/orchestration/message_box.rs`
- Modify: `src/orchestration/service.rs`
- Modify: `src/orchestration/mod.rs`
- Create: `tests/execution_message_delivery.rs`

## Chunk 1: Delivery Types

### Task 1: Add delivery trait and run reference types

**Files:**
- Create: `src/runtime/delivery.rs`
- Modify: `src/runtime/mod.rs`
- Test: `tests/execution_message_delivery.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/execution_message_delivery.rs` with coverage for:
- `DeliveryCapability` variants
- `VoidBoxRunRef` fields
- compile-time visibility through `void_control::runtime`

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --features serde --test execution_message_delivery`
Expected: compile failure because the delivery module does not exist yet.

- [ ] **Step 3: Add the delivery module**

Create `src/runtime/delivery.rs` with:
- `DeliveryCapability`
- `VoidBoxRunRef`
- `MessageDeliveryAdapter`

Requirements:
- gate the trait behind `serde`
- use `std::io::Result`
- model `drain_intents` as non-idempotent
- keep `push_live` defaulting to unsupported

- [ ] **Step 4: Export the new types**

Update `src/runtime/mod.rs` to:
- include `mod delivery`
- re-export `DeliveryCapability`
- re-export `VoidBoxRunRef`
- re-export `MessageDeliveryAdapter` behind `serde`

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --features serde --test execution_message_delivery`
Expected: PASS for the new type-level tests.

- [ ] **Step 6: Commit**

```bash
git add src/runtime/delivery.rs src/runtime/mod.rs tests/execution_message_delivery.rs
git commit -m "runtime: add message delivery adapter types"
```

## Chunk 2: HTTP Sidecar Adapter

### Task 2: Add `HttpSidecarAdapter`

**Files:**
- Create: `src/runtime/http_sidecar.rs`
- Modify: `src/runtime/mod.rs`
- Modify: `src/runtime/void_box.rs`
- Test: `tests/execution_message_delivery.rs`

- [ ] **Step 1: Write the failing tests**

Add tests for:
- declared capabilities (`LaunchInjection`, `LivePoll`)
- unsupported `push_live` by default
- generated messaging skill content

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --features serde --test execution_message_delivery`
Expected: compile failure because `HttpSidecarAdapter` does not exist.

- [ ] **Step 3: Reuse the existing HTTP transport pattern**

Inspect `src/runtime/void_box.rs` and extract or reuse the existing low-level HTTP helper pattern instead of introducing a brand-new client stack. Do not assume `reqwest` is already in this repo unless you explicitly add it.

- [ ] **Step 4: Implement `HttpSidecarAdapter`**

Create `src/runtime/http_sidecar.rs` with methods for:
- `inject_at_launch` -> `PUT /v1/runs/{id}/inbox`
- `drain_intents` -> `GET /v1/runs/{id}/intents`
- `messaging_skill`

Implementation requirements:
- serialize/deserialize with `serde_json`
- convert transport failures into `io::Error`
- map the sidecar intent payloads into canonical `CommunicationIntent`
- leave `push_live` unsupported in this adapter unless the daemon path is already stable and testable

- [ ] **Step 5: Export the adapter**

Update `src/runtime/mod.rs` to re-export `HttpSidecarAdapter` behind `serde`.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --features serde --test execution_message_delivery`
Expected: PASS for the adapter capability tests.

- [ ] **Step 7: Commit**

```bash
git add src/runtime/http_sidecar.rs src/runtime/mod.rs src/runtime/void_box.rs tests/execution_message_delivery.rs
git commit -m "runtime: add http sidecar delivery adapter"
```

## Chunk 3: Dual Intent Merge

### Task 3: Add dual-source merge and dedup

**Files:**
- Modify: `src/orchestration/message_box.rs`
- Modify: `src/orchestration/mod.rs`
- Test: `tests/execution_message_delivery.rs`

- [ ] **Step 1: Write the failing tests**

Add tests for:
- merging distinct sidecar and structured-output intents
- deduplicating identical content across the two sources
- keeping both intents when audience differs
- keeping both intents when source iteration differs

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --features serde --test execution_message_delivery merge`
Expected: compile failure because `merge_and_dedup` does not exist.

- [ ] **Step 3: Implement `merge_and_dedup`**

In `src/orchestration/message_box.rs`, add a helper that:
- takes `sidecar_intents` and `output_intents`
- deduplicates by the message-box content key:
  - normalized payload
  - audience
  - source iteration
- preserves source priority deterministically

Phase-1 rule:
- sidecar wins on exact duplicate because it reflects transport-collected emission

- [ ] **Step 4: Export the helper if needed for tests**

Update `src/orchestration/mod.rs` if the tests need the symbol re-exported.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --features serde --test execution_message_delivery merge`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/orchestration/message_box.rs src/orchestration/mod.rs tests/execution_message_delivery.rs
git commit -m "orchestration: merge sidecar and structured output intents"
```

## Chunk 4: Service Plumbing

### Task 4: Add optional delivery-adapter plumbing to `ExecutionService`

**Files:**
- Modify: `src/orchestration/service.rs`
- Test: `tests/execution_message_delivery.rs`

- [ ] **Step 1: Write the failing tests**

Add tests that verify:
- `ExecutionService::new(...)` still compiles and behaves the same
- a new `with_delivery_adapter(...)` constructor exists alongside `with_launch_adapter(...)`
- dispatch uses the delivery adapter path when configured
- legacy launch adapter remains the fallback when no delivery adapter is configured

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --features serde --test execution_message_delivery service`
Expected: compile or behavior failure because the new constructor and plumbing do not exist yet.

- [ ] **Step 3: Add the optional adapter field**

Update `ExecutionService<R>` in `src/orchestration/service.rs`:
- add `delivery_adapter: Option<Box<dyn crate::runtime::MessageDeliveryAdapter>>` behind `serde`
- initialize it to `None` in existing constructors

- [ ] **Step 4: Add a real constructor matching current APIs**

Add `with_delivery_adapter(...)` using the actual current constructor shape:
- `GlobalConfig`
- runtime
- `FsExecutionStore`
- `Box<dyn MessageDeliveryAdapter>`

Do not invent a string path constructor; this repo already uses `FsExecutionStore::new(...)` outside the service.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --features serde --test execution_message_delivery service`
Expected: PASS for constructor/plumbing tests.

- [ ] **Step 6: Commit**

```bash
git add src/orchestration/service.rs tests/execution_message_delivery.rs
git commit -m "orchestration: add optional delivery adapter plumbing"
```

## Chunk 5: Dispatch Integration

### Task 5: Use sidecar injection when a delivery adapter is configured

**Files:**
- Modify: `src/orchestration/service.rs`
- Test: `tests/execution_message_delivery.rs`

- [ ] **Step 1: Write the failing integration test**

Add a test with a fake delivery adapter that records:
- injected run IDs
- drained intents

Seed `MockRuntime` with a successful candidate output and verify:
- `inject_at_launch` is called before run completion
- legacy `launch_context` path is not required when the delivery adapter is active

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --features serde --test execution_message_delivery dispatch`
Expected: FAIL because dispatch still only uses `launch_adapter`.

- [ ] **Step 3: Branch dispatch on adapter presence**

In `dispatch_candidate(...)`:
- load the inbox snapshot as today
- if `delivery_adapter` exists:
  - construct `VoidBoxRunRef` from the actual runtime/daemon configuration
  - call `inject_at_launch(...)`
  - start the run without relying on `launch_context`
- else:
  - use the existing `launch_adapter.prepare_launch_request(...)` path unchanged

Refactor shared post-launch logic into a helper if needed to avoid duplication.

- [ ] **Step 4: Run focused tests**

Run: `cargo test --features serde --test execution_message_delivery dispatch`
Expected: PASS.

- [ ] **Step 5: Run regression tests**

Run: `cargo test --features serde`
Expected: existing orchestration and bridge tests remain green.

- [ ] **Step 6: Commit**

```bash
git add src/orchestration/service.rs tests/execution_message_delivery.rs
git commit -m "orchestration: use delivery adapter for sidecar inbox injection"
```

## Chunk 6: Intent Collection Integration

### Task 6: Merge sidecar-drained intents with structured output intents

**Files:**
- Modify: `src/orchestration/service.rs`
- Test: `tests/execution_message_delivery.rs`

- [ ] **Step 1: Write the failing test**

Add a test that:
- configures a fake delivery adapter to return one sidecar intent
- seeds candidate structured output with one additional intent
- verifies both reach the existing normalization/routing pipeline after dedup

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --features serde --test execution_message_delivery collection`
Expected: FAIL because collection still only uses `output.intents`.

- [ ] **Step 3: Update intent persistence flow**

Change `persist_candidate_intents(...)` so that when a delivery adapter is present it:
- drains sidecar intents
- merges them with `output.intents` via `merge_and_dedup`
- preserves the current normalize -> route -> persist flow

Collection-failure rule for phase 1:
- treat sidecar drain failure as partial loss
- continue with structured-output intents if present
- emit a control-plane diagnostic event if an event type already exists, otherwise leave a clearly marked follow-up comment rather than using `eprintln!`

- [ ] **Step 4: Run focused tests**

Run: `cargo test --features serde --test execution_message_delivery collection`
Expected: PASS.

- [ ] **Step 5: Run full serde regression**

Run: `cargo test --features serde`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/orchestration/service.rs tests/execution_message_delivery.rs
git commit -m "orchestration: collect intents from sidecar and structured output"
```

## Chunk 7: Final Verification

### Task 7: Validate the whole slice

**Files:**
- No additional edits required

- [ ] **Step 1: Run formatting**

Run: `cargo fmt --all -- --check`
Expected: PASS.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS.

- [ ] **Step 3: Run Rust tests**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 4: Run serde tests**

Run: `cargo test --features serde`
Expected: PASS.

- [ ] **Step 5: Inspect final diff**

Run: `git diff --stat`

- [ ] **Step 6: Optional squash or handoff**

If the branch history should be compressed, do that only after review. Otherwise keep the task-scoped commits.

## Deferred Follow-up

This plan intentionally does **not** include:
- `Skill` / `CandidateSpec::add_skill()` changes
- provider-bridge-specific live push implementation
- removal of `ProviderLaunchAdapter`
- `void-box` endpoint implementation

Those should be handled in separate follow-up plans after this phase lands cleanly.
