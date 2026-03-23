# Void-Box Runtime Alignment Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align `void-control` with the forward-looking `void-box` runtime contract by adding live execution API tests, manifest-aware runtime client behavior, and only the minimum bridge/service adjustments required by the richer runtime surface.

**Architecture:** Keep the work focused on the `void-control` to `void-box` boundary. `tests/void_box_contract.rs` becomes the live ignored contract gate for execution and artifact behavior, `src/runtime/void_box.rs` becomes the single place that understands manifests, named artifact retrieval, and typed runtime output failures, and `src/bridge.rs` plus `src/orchestration/service.rs` should change only where the runtime client’s richer semantics force controller-side behavior changes.

**Tech Stack:** Rust 2021, existing `serde` feature-gated bridge/runtime code, live ignored Cargo tests against a running `void-box` daemon, filesystem-backed execution store, and the current TCP/HTTP runtime client.

---

## Scope Check

This plan intentionally excludes broader hardening work:
- no worker locking redesign,
- no execution lifecycle event expansion,
- no UI changes,
- no new orchestration strategies.

The focus is only:
1. live daemon contract coverage,
2. runtime client alignment to the new `void-box` artifact contract,
3. minimal bridge/service changes needed to consume that contract cleanly.

## File Map

### Primary files

- Modify: `tests/void_box_contract.rs`
  Responsibility: ignored live daemon contract tests for runtime behavior, artifact publication, and reconciliation-facing endpoints.
- Modify: `src/runtime/void_box.rs`
  Responsibility: runtime HTTP client, manifest-aware structured output retrieval, named artifact retrieval, and typed runtime error mapping.
- Modify: `src/runtime/mod.rs`
  Responsibility: `ExecutionRuntime` glue for the richer `VoidBoxRuntimeClient` behavior.
- Modify: `src/orchestration/service.rs`
  Responsibility: consume runtime output/error semantics without guessing from `None`.
- Modify: `src/bridge.rs`
  Responsibility: only the minimum response and processing adjustments needed once runtime errors and artifact metadata become explicit.

### Possible supporting files

- Modify: `src/contract/mod.rs`
  Responsibility: add or extend contract error mapping if the new runtime error codes need first-class representation.
- Modify: `tests/execution_bridge.rs`
  Responsibility: bridge-level regression tests if runtime error mapping changes user-facing execution responses.
- Modify: `tests/execution_worker.rs`
  Responsibility: worker-side regression tests if artifact collection semantics change.

## Delivery Strategy

Implement in this order:

1. write ignored live tests that describe the new daemon contract,
2. adapt `VoidBoxRuntimeClient` until those tests can pass against the new daemon,
3. tighten orchestration handling of explicit runtime output failures,
4. run the verification matrix locally against both unit tests and live daemon gates.

This keeps the contract authoritative and prevents the client from baking in assumptions that the daemon does not actually satisfy.

## Chunk 1: Live Daemon Contract Tests

### Task 1: Add execution-and-artifact live tests

**Files:**
- Modify: `tests/void_box_contract.rs`

- [ ] **Step 1: Add ignored failing tests for the new runtime contract**

Add live ignored tests covering:

```rust
#[test]
#[ignore]
fn structured_output_result_json_is_retrievable() {}

#[test]
#[ignore]
fn missing_result_json_is_typed_failure() {}

#[test]
#[ignore]
fn malformed_result_json_is_typed_failure() {}

#[test]
#[ignore]
fn manifest_lists_named_artifacts() {}

#[test]
#[ignore]
fn named_artifact_endpoint_serves_manifested_file() {}

#[test]
#[ignore]
fn active_run_listing_supports_reconciliation() {}
```

- [ ] **Step 2: Add fixture generation helpers only if needed**

Extend the fallback spec generation in `tests/void_box_contract.rs` with the minimum new cases:
- success case that emits valid `result.json`,
- success case that emits `result.json` plus one named artifact,
- terminal case with missing `result.json`,
- terminal case with malformed `result.json`.

Do not add broad fixture abstractions. Keep them in the existing test file.

- [ ] **Step 3: Run targeted compile-only validation**

Run: `cargo test --features serde --test void_box_contract -- --ignored --list`
Expected: the new ignored tests appear in the list and compile.

- [ ] **Step 4: Run live tests against the daemon once Claude’s `void-box` branch exposes the new endpoints**

Run:

```bash
TMPDIR=/tmp CARGO_TARGET_DIR=/home/diego/github/void-control/target \
VOID_BOX_BASE_URL=http://127.0.0.1:43100 \
cargo test --features serde --test void_box_contract -- --ignored --nocapture
```

Expected:
- current failures identify real contract gaps,
- no failures come from malformed test harness assumptions.

- [ ] **Step 5: Commit**

```bash
git add tests/void_box_contract.rs
git commit -m "tests: add live void-box artifact contract coverage"
```

## Chunk 2: Runtime Client Contract Alignment

### Task 2: Make `VoidBoxRuntimeClient` manifest-aware

**Files:**
- Modify: `src/runtime/void_box.rs`
- Modify: `src/runtime/mod.rs`

- [ ] **Step 1: Write focused unit tests in `src/runtime/void_box.rs` for the new retrieval paths**

Cover at least:

```rust
#[test]
fn fetch_structured_output_prefers_manifested_result_json() {}

#[test]
fn fetch_structured_output_maps_missing_output_error() {}

#[test]
fn fetch_structured_output_maps_malformed_output_error() {}

#[test]
fn fetch_named_artifact_uses_manifest_retrieval_path() {}

#[test]
fn inspect_reads_artifact_publication_metadata_when_present() {}
```

- [ ] **Step 2: Run targeted unit tests and verify they fail**

Run:

```bash
cargo test --features serde runtime::void_box:: -- --nocapture
```

Expected: failures for missing manifest parsing, missing typed error handling, or missing named artifact helpers.

- [ ] **Step 3: Implement minimal client additions**

In `src/runtime/void_box.rs`, add:
- manifest parsing from inspect payloads when present,
- a helper to retrieve named artifacts from manifest entries,
- typed handling for runtime error codes such as:
  - `STRUCTURED_OUTPUT_MISSING`
  - `STRUCTURED_OUTPUT_MALFORMED`
  - `ARTIFACT_NOT_FOUND`
  - `ARTIFACT_PUBLICATION_INCOMPLETE`
  - `ARTIFACT_STORE_UNAVAILABLE`
  - `RETRIEVAL_TIMEOUT`
- structured output retrieval that prefers normalized manifest/inspection metadata and only falls back to the current `output-file` path when needed for compatibility.

Do not redesign the transport layer.

- [ ] **Step 4: Update `src/runtime/mod.rs` glue only if the runtime trait needs richer return semantics**

If `Option<CandidateOutput>` is no longer expressive enough, add the smallest trait/API change needed to distinguish:
- missing structured output,
- malformed structured output,
- retrieval temporary failure,
- successful output.

Keep the change local and update only the call sites this plan covers.

- [ ] **Step 5: Re-run unit coverage**

Run:

```bash
cargo test --features serde runtime::void_box:: -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/runtime/void_box.rs src/runtime/mod.rs
git commit -m "runtime: align void-box client with artifact contract"
```

## Chunk 3: Minimal Orchestration and Bridge Adjustments

### Task 3: Consume explicit runtime output failures

**Files:**
- Modify: `src/orchestration/service.rs`
- Modify: `src/bridge.rs`
- Test: `tests/execution_worker.rs`
- Test: `tests/execution_bridge.rs`

- [ ] **Step 1: Add failing regression tests for runtime output failure mapping**

Cover at least:

```rust
#[test]
fn worker_marks_candidate_failed_on_structured_output_missing() {}

#[test]
fn worker_marks_candidate_failed_on_structured_output_malformed() {}

#[test]
fn bridge_preserves_retryable_runtime_error_information_when_execution_fails() {}
```

- [ ] **Step 2: Run targeted tests and verify they fail**

Run:

```bash
cargo test --features serde --test execution_worker --test execution_bridge -- --nocapture
```

Expected: current code collapses too many runtime cases into `None` or generic failure.

- [ ] **Step 3: Implement the smallest controller-side change**

Update `src/orchestration/service.rs` so artifact collection distinguishes:
- no structured output because the candidate genuinely did not produce one,
- malformed output,
- temporary retrieval/publishing failure.

Update `src/bridge.rs` only if the richer failure information should surface in execution inspection or HTTP error bodies. Avoid changing route shapes unless required.

- [ ] **Step 4: Re-run targeted regression tests**

Run:

```bash
cargo test --features serde --test execution_worker --test execution_bridge -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/orchestration/service.rs src/bridge.rs tests/execution_worker.rs tests/execution_bridge.rs
git commit -m "orchestration: preserve runtime output failure semantics"
```

## Chunk 4: Verification and Rollout

### Task 4: Run the full relevant verification matrix

**Files:**
- No source changes expected unless failures reveal drift

- [ ] **Step 1: Run fast local regression coverage**

Run:

```bash
cargo test --features serde --test execution_bridge --test execution_worker --test void_box_contract -- --nocapture
```

Expected:
- non-ignored tests pass,
- ignored live tests compile.

- [ ] **Step 2: Run runtime unit coverage**

Run:

```bash
cargo test --features serde runtime::void_box:: -- --nocapture
```

Expected: PASS.

- [ ] **Step 3: Run live daemon gate**

Run:

```bash
TMPDIR=/tmp CARGO_TARGET_DIR=/home/diego/github/void-control/target \
VOID_BOX_BASE_URL=http://127.0.0.1:43100 \
cargo test --features serde --test void_box_contract -- --ignored --nocapture
```

Expected: PASS once the paired `void-box` changes are live.

- [ ] **Step 4: If the live gate exposes daemon/client drift, fix only the boundary mismatch**

Do not expand scope into worker locking, lifecycle events, or UI work during this pass.

- [ ] **Step 5: Commit any final boundary fixes**

```bash
git add src/runtime/void_box.rs src/runtime/mod.rs src/orchestration/service.rs src/bridge.rs tests/void_box_contract.rs tests/execution_worker.rs tests/execution_bridge.rs
git commit -m "runtime: finalize void-box boundary alignment"
```
