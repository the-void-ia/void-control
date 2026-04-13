# Supervision Strategy Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a first `mode: supervision` orchestration strategy to `void-control` using a flat orchestrator-worker model with centralized review, revision, retry, and finalization.

**Architecture:** Extend the existing execution spec, strategy host, orchestration service, and execution event model to support `supervision` without changing the `void-box` runtime boundary. Reuse the current execution, candidate, message-box, bridge, CLI, and graph-shell UI primitives while giving `supervision` its own planner, reducer, event semantics, and UI rendering.

**Tech Stack:** Rust (`void-control` library, bridge, CLI), serde-gated bridge APIs, React/Vite UI, existing execution store and event replay model.

---

## File Map

### Existing files to modify

- `src/orchestration/spec.rs`
  - add `mode: supervision` validation and supervision-specific config
- `src/orchestration/strategy.rs`
  - add `SupervisionStrategy`
- `src/orchestration/types.rs`
  - extend accumulator/candidate metadata for review state
- `src/orchestration/events.rs`
  - add supervision event types and payload helpers
- `src/orchestration/service.rs`
  - branch planning/reduction flow for supervision
- `src/bridge.rs`
  - expose supervision details through execution APIs
- `src/bin/voidctl.rs`
  - inspect/result/runtime output for supervision executions
- `tests/execution_spec_validation.rs`
  - spec validation coverage
- `tests/execution_strategy_acceptance.rs`
  - end-to-end supervision acceptance with `MockRuntime`
- `tests/execution_bridge.rs`
  - bridge create/inspect/result coverage
- `web/void-control-ux/src/App.tsx`
  - mode-aware supervision detail shell routing
- `web/void-control-ux/src/lib/orchestration.ts`
  - derive supervision view model from execution detail/events
- `README.md`
  - mention supervision as the next implemented strategy once code lands
- `docs/architecture.md`
  - update strategy list after implementation

### New files to create

- `tests/execution_supervision_strategy.rs`
  - focused planner/reducer tests for `SupervisionStrategy`
- `web/void-control-ux/src/components/SupervisionGraph.tsx`
  - graph rendering for supervisor + workers + revision edges
- `web/void-control-ux/src/components/SupervisionInspector.tsx`
  - right-side inspector for review state and runtime jump

Keep new files focused. Do not split persistence or runtime into supervision-only parallel subsystems.

## Chunk 1: Spec And Event Model

### Task 1: Add failing spec validation tests for supervision

**Files:**
- Modify: `tests/execution_spec_validation.rs`
- Modify: `src/orchestration/spec.rs`

- [ ] **Step 1: Write failing tests for valid supervision specs**

Add tests covering:
- `mode: supervision` is accepted
- supervision block is required when mode is supervision
- `variation.source` is still validated

- [ ] **Step 2: Run only the new validation tests and verify they fail**

Run:

```bash
cargo test --features serde --test execution_spec_validation supervision -- --nocapture
```

Expected: failing assertions or validation errors because `supervision` is not yet supported.

- [ ] **Step 3: Extend `ExecutionSpec` for supervision config**

Modify `src/orchestration/spec.rs` to add:
- a serde-compatible supervision config block
- supervisor review policy fields:
  - `max_revision_rounds`
  - `retry_on_runtime_failure`
  - `require_final_approval`

Keep the shape additive to the existing spec.

- [ ] **Step 4: Implement minimal validation**

Validation rules:
- allow `mode` values `swarm`, `supervision`
- require the supervision block when `mode == "supervision"`
- preserve current workflow/policy/variation validation

- [ ] **Step 5: Re-run the targeted test**

Run:

```bash
cargo test --features serde --test execution_spec_validation supervision -- --nocapture
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/orchestration/spec.rs tests/execution_spec_validation.rs
git commit -m "spec: add supervision validation"
```

### Task 2: Add failing supervision control-plane event tests

**Files:**
- Modify: `src/orchestration/events.rs`
- Modify: `tests/execution_event_replay.rs`

- [ ] **Step 1: Write failing event replay tests for supervision event types**

Add tests that expect support for:
- `SupervisorAssigned`
- `WorkerQueued`
- `ReviewRequested`
- `WorkerApproved`
- `RevisionRequested`
- `ExecutionFinalized`

- [ ] **Step 2: Run the targeted event replay tests and verify failure**

Run:

```bash
cargo test --features serde --test execution_event_replay supervision -- --nocapture
```

Expected: fail because event types are unknown or replay handling is incomplete.

- [ ] **Step 3: Add supervision event types and payload helpers**

Modify `src/orchestration/events.rs` to define the new event kinds and keep payloads additive.

- [ ] **Step 4: Re-run the targeted event replay tests**

Run:

```bash
cargo test --features serde --test execution_event_replay supervision -- --nocapture
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/orchestration/events.rs tests/execution_event_replay.rs
git commit -m "events: add supervision event types"
```

## Chunk 2: Strategy And Service Core

### Task 3: Add failing planner/reducer tests for `SupervisionStrategy`

**Files:**
- Create: `tests/execution_supervision_strategy.rs`
- Modify: `src/orchestration/strategy.rs`
- Modify: `src/orchestration/types.rs`

- [ ] **Step 1: Write failing planner/reducer tests**

Cover these cases:
- initial worker set comes from `variation`
- worker outputs create review-needed state
- approved outputs complete when final approval is satisfied
- revision requests requeue a worker when revision budget remains
- runtime failure retries when policy allows it

- [ ] **Step 2: Run the new strategy test file and verify failure**

Run:

```bash
cargo test --features serde --test execution_supervision_strategy -- --nocapture
```

Expected: fail because `SupervisionStrategy` and review state do not exist.

- [ ] **Step 3: Add minimal review-state fields**

Modify `src/orchestration/types.rs` to add only the additive state needed for:
- worker review status
- revision rounds
- final approval summary

- [ ] **Step 4: Implement `SupervisionStrategy`**

Modify `src/orchestration/strategy.rs` to add:
- `SupervisionStrategy`
- worker planning from existing variation proposals
- reduction by review state instead of score ranking

Do not refactor existing swarm behavior during this task.

- [ ] **Step 5: Re-run the new strategy test file**

Run:

```bash
cargo test --features serde --test execution_supervision_strategy -- --nocapture
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/orchestration/strategy.rs src/orchestration/types.rs tests/execution_supervision_strategy.rs
git commit -m "strategy: add supervision reducer"
```

### Task 4: Add failing service acceptance tests for supervision

**Files:**
- Modify: `tests/execution_strategy_acceptance.rs`
- Modify: `src/orchestration/service.rs`

- [ ] **Step 1: Write failing end-to-end supervision acceptance tests**

Cover:
- supervision execution runs end to end on `MockRuntime`
- worker outputs persist and trigger review events
- revision path requeues work
- runtime failure uses retry policy

- [ ] **Step 2: Run only the supervision acceptance tests and verify failure**

Run:

```bash
cargo test --features serde --test execution_strategy_acceptance supervision -- --nocapture
```

Expected: failure because service flow does not yet route supervision.

- [ ] **Step 3: Add supervision routing to the orchestration service**

Modify `src/orchestration/service.rs` to:
- construct `SupervisionStrategy` when `mode == "supervision"`
- persist supervision events
- convert worker output collection into review requests
- apply approve/revise/retry/finalize reduction rules

Keep runtime execution through the existing runtime adapter.

- [ ] **Step 4: Re-run the targeted acceptance tests**

Run:

```bash
cargo test --features serde --test execution_strategy_acceptance supervision -- --nocapture
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/orchestration/service.rs tests/execution_strategy_acceptance.rs
git commit -m "service: run supervision executions"
```

## Chunk 3: Bridge And CLI

### Task 5: Add failing bridge tests for supervision execution detail

**Files:**
- Modify: `tests/execution_bridge.rs`
- Modify: `src/bridge.rs`

- [ ] **Step 1: Write failing bridge tests**

Add coverage for:
- create execution route accepts `mode: supervision`
- get execution route returns supervision mode detail
- result payload includes approved/finalized worker summary

- [ ] **Step 2: Run the targeted bridge tests and verify failure**

Run:

```bash
cargo test --features serde --test execution_bridge supervision -- --nocapture
```

Expected: failure because bridge detail does not yet expose supervision semantics.

- [ ] **Step 3: Implement minimal bridge support**

Modify `src/bridge.rs` to:
- accept `mode: supervision`
- expose supervision-specific summary fields through existing execution detail
- preserve backward compatibility for swarm/runtime flows

- [ ] **Step 4: Re-run the targeted bridge tests**

Run:

```bash
cargo test --features serde --test execution_bridge supervision -- --nocapture
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/bridge.rs tests/execution_bridge.rs
git commit -m "bridge: expose supervision executions"
```

### Task 6: Add failing CLI tests for supervision inspect/result/runtime output

**Files:**
- Modify: `tests/voidctl_execution_cli.rs`
- Modify: `src/bin/voidctl.rs`

- [ ] **Step 1: Write failing CLI tests**

Cover:
- `inspect` shows supervisor state and worker counts
- `result` shows finalized/approved worker summary
- `runtime` resolves the selected worker runtime run

- [ ] **Step 2: Run the targeted CLI tests and verify failure**

Run:

```bash
cargo test --features serde --test voidctl_execution_cli supervision -- --nocapture
```

Expected: failure because CLI text output is swarm/runtime-only.

- [ ] **Step 3: Implement minimal CLI supervision rendering**

Modify `src/bin/voidctl.rs` to:
- recognize supervision execution detail
- print worker/review summaries in `inspect`
- print approval/finalization summary in `result`
- keep `runtime` selection compatible with worker IDs

- [ ] **Step 4: Re-run the targeted CLI tests**

Run:

```bash
cargo test --features serde --test voidctl_execution_cli supervision -- --nocapture
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/bin/voidctl.rs tests/voidctl_execution_cli.rs
git commit -m "cli: render supervision executions"
```

## Chunk 4: UI

### Task 7: Add failing orchestration derivation tests for supervision

**Files:**
- Modify: `web/void-control-ux/src/lib/orchestration.ts`
- Modify or create: `web/void-control-ux/src/lib/orchestration.test.ts`

- [ ] **Step 1: Write failing derivation tests**

Cover:
- execution detail + events derive a supervisor node
- worker nodes and review state are exposed
- approved/finalized worker is identified

- [ ] **Step 2: Run the targeted frontend test or build check and verify failure**

Use the repo’s existing frontend test path if present. If there is no unit test harness yet, add the derivation test only if consistent with the repo; otherwise use `npm run build` after implementation as the validation gate and note the gap in the commit message.

- [ ] **Step 3: Add supervision derivation support**

Modify `web/void-control-ux/src/lib/orchestration.ts` to derive:
- supervisor node
- worker nodes
- revision links
- approved/finalized state

- [ ] **Step 4: Re-run the targeted check**

Run:

```bash
cd web/void-control-ux
npm run build
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add web/void-control-ux/src/lib/orchestration.ts web/void-control-ux/src/lib/orchestration.test.ts
git commit -m "ui: derive supervision execution state"
```

### Task 8: Add supervision graph and inspector components

**Files:**
- Create: `web/void-control-ux/src/components/SupervisionGraph.tsx`
- Create: `web/void-control-ux/src/components/SupervisionInspector.tsx`
- Modify: `web/void-control-ux/src/App.tsx`

- [ ] **Step 1: Add failing component integration expectation**

If there is an existing browser/DOM test path, add a failing expectation there. If not, document the expectation in the commit and validate by live build + browser inspection.

Expected behavior:
- center graph shows supervisor + worker layout
- right inspector shows review state and runtime jump

- [ ] **Step 2: Implement `SupervisionGraph.tsx`**

Render:
- one supervisor node
- worker fan-out
- revision edges
- approved/finalized highlighting

- [ ] **Step 3: Implement `SupervisionInspector.tsx`**

Render:
- selected node state
- review outcome
- revision history
- runtime jump

- [ ] **Step 4: Wire supervision mode in `App.tsx`**

Keep the same shell:
- left executions
- center graph
- bottom event strip
- right inspector

- [ ] **Step 5: Run the frontend build**

Run:

```bash
cd web/void-control-ux
npm run build
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add web/void-control-ux/src/components/SupervisionGraph.tsx web/void-control-ux/src/components/SupervisionInspector.tsx web/void-control-ux/src/App.tsx
git commit -m "ui: add supervision execution view"
```

## Chunk 5: Docs And Final Verification

### Task 9: Update docs after implementation lands

**Files:**
- Modify: `README.md`
- Modify: `docs/architecture.md`
- Modify: `AGENTS.md` if workflow commands or examples need changes

- [ ] **Step 1: Update strategy documentation**

Document:
- `supervision` as implemented orchestrator-worker mode
- how it differs from `swarm`
- which communication model it uses (`void-mcp` primary, `void-message` secondary)

- [ ] **Step 2: Verify docs read cleanly against the implementation**

Manually inspect the changed sections to ensure terminology matches the code.

- [ ] **Step 3: Commit**

```bash
git add README.md docs/architecture.md AGENTS.md
git commit -m "docs: document supervision strategy"
```

### Task 10: Run final verification before completion

**Files:**
- No new file changes expected

- [ ] **Step 1: Run Rust validation**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo test --features serde
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
```

Expected: all pass

- [ ] **Step 2: Run UI validation**

```bash
cd web/void-control-ux
npm run build
```

Expected: PASS

- [ ] **Step 3: Run live spot-check if daemon is available**

Use the existing local stack only if it is already running:

```bash
target/debug/voidctl execution dry-run <supervision-spec>
target/debug/voidctl execution submit <supervision-spec>
target/debug/voidctl execution inspect <execution-id>
```

Expected: execution is created and supervision detail renders coherently.

- [ ] **Step 4: Commit final cleanup if needed**

```bash
git status --short
```

Only create a cleanup commit if there are deliberate follow-up doc/test fixes.
