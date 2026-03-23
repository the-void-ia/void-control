# Search Strategy Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `search` as a second supported orchestration strategy using incumbent-centered refinement with optional bootstrap, and add an acceptance suite that exercises every supported strategy end to end.

**Architecture:** Keep the execution loop, runtime contract, scheduler, and bridge unchanged. Implement `SearchStrategy` inside the existing `IterationStrategy` boundary, extend the accumulator with only the state search needs, and route mode selection through the existing service/strategy layer. Add an integration acceptance suite that runs `swarm` and `search` through the same orchestration path so supported-strategy coverage is explicit.

**Tech Stack:** Rust 2021, existing orchestration modules under `src/orchestration/`, filesystem-backed execution store, `serde`-gated bridge/integration tests, current `MockRuntime` and live bridge test infrastructure.

---

## Scope Check

This plan includes:
1. `search` mode validation and strategy selection,
2. incumbent-centered refinement with optional bootstrap,
3. minimal accumulator/state additions for explored signatures and search phase,
4. strategy-focused unit coverage,
5. a strategy acceptance suite that runs all supported strategies.

This plan intentionally excludes:
- adaptive `swarm -> search` mode switching,
- new evaluation models such as pairwise/tournament scoring,
- UI changes,
- non-`swarm` / non-`search` strategy implementations,
- broader runtime or scheduler refactors unrelated to search semantics.

## File Map

### Primary files

- Modify: `src/orchestration/spec.rs`
  Responsibility: allow `search` as a valid execution mode and validate any mode-specific constraints added for the first cut.
- Modify: `src/orchestration/types.rs`
  Responsibility: add minimal accumulator fields needed by search, such as explored signatures and optional search phase.
- Modify: `src/orchestration/strategy.rs`
  Responsibility: add `SearchStrategy`, keep `SwarmStrategy` intact, and expose a shared strategy-selection boundary.
- Modify: `src/orchestration/mod.rs`
  Responsibility: export `SearchStrategy` and any new search-specific types.
- Modify: `src/orchestration/service.rs`
  Responsibility: select `SwarmStrategy` vs `SearchStrategy` by mode without changing execution/runtime behavior.
- Modify: `src/orchestration/variation.rs`
  Responsibility: add small helper logic if search needs reusable mutation/signature helpers.

### Tests

- Modify: `tests/execution_spec_validation.rs`
  Responsibility: validate that `search` is accepted and bad unknown modes still reject.
- Modify: `tests/execution_swarm_strategy.rs`
  Responsibility: rename or broaden where useful so strategy-specific unit tests cover both swarm and search.
- Create: `tests/execution_search_strategy.rs`
  Responsibility: focused unit tests for bootstrap, incumbent refinement, explored-signature avoidance, and reduce behavior.
- Create: `tests/execution_strategy_acceptance.rs`
  Responsibility: integration acceptance suite that runs every supported strategy (`swarm`, `search`) through the same orchestration execution path.
- Modify: `tests/execution_bridge.rs`
  Responsibility: ensure bridge create/get surfaces work with `search` specs as well as `swarm`.

## Delivery Strategy

Implement in this order:

1. add validation and accumulator support for `search`,
2. add `SearchStrategy` with bootstrap and refinement behavior,
3. wire mode selection in the service,
4. add focused unit tests,
5. add the supported-strategy acceptance suite.

This keeps strategy behavior isolated from the dispatcher and bridge machinery already stabilized on the branch.

## Chunk 1: Mode and State Support

### Task 1: Add `search` as a supported mode with minimal accumulator extensions

**Files:**
- Modify: `src/orchestration/spec.rs`
- Modify: `src/orchestration/types.rs`
- Modify: `src/orchestration/mod.rs`
- Test: `tests/execution_spec_validation.rs`

- [ ] **Step 1: Write the failing validation test**

Add a test in `tests/execution_spec_validation.rs` that submits a `search` mode spec and expects validation success.

- [ ] **Step 2: Run the validation test to verify it fails**

Run: `cargo test --features serde --test execution_spec_validation accepts_search_mode -- --exact --nocapture`
Expected: FAIL because `search` is not yet an accepted mode.

- [ ] **Step 3: Allow `search` in spec validation**

Update `src/orchestration/spec.rs` so `search` is accepted alongside `swarm`. Keep unknown modes rejected.

- [ ] **Step 4: Add minimal search state to the accumulator**

Update `src/orchestration/types.rs`:
- add `search_phase: Option<String>` or a small enum-like string field,
- add `explored_signatures: Vec<String>` or another minimal persisted representation.

Do not add adaptive mode-switching state yet.

- [ ] **Step 5: Export any new state types**

Update `src/orchestration/mod.rs` so new search-related types are available to tests.

- [ ] **Step 6: Run the validation file**

Run: `cargo test --features serde --test execution_spec_validation -- --nocapture`
Expected: PASS.

## Chunk 2: Search Strategy Core

### Task 2: Implement `SearchStrategy`

**Files:**
- Modify: `src/orchestration/strategy.rs`
- Modify: `src/orchestration/variation.rs`
- Create: `tests/execution_search_strategy.rs`

- [ ] **Step 1: Write the failing bootstrap test**

Add a test in `tests/execution_search_strategy.rs` for:
- no seed / no incumbent,
- bootstrap round returns a small non-empty candidate batch,
- bootstrap is smaller than unconstrained broad swarm behavior for the same variation source.

- [ ] **Step 2: Run the bootstrap test to verify it fails**

Run: `cargo test --features serde --test execution_search_strategy search_bootstraps_when_no_seed_exists -- --exact --nocapture`
Expected: FAIL because `SearchStrategy` does not exist yet.

- [ ] **Step 3: Add `SearchStrategy` type**

In `src/orchestration/strategy.rs`, add `SearchStrategy` implementing the same trait surface as `SwarmStrategy`:
- `materialize_inboxes()`
- `plan_candidates()`
- `evaluate()`
- `should_stop()`
- `reduce()`

- [ ] **Step 4: Implement bootstrap planning**

For iteration 0 when no incumbent/seed exists:
- generate a constrained bootstrap batch,
- reuse existing variation helpers where possible,
- keep candidate count intentionally small.

- [ ] **Step 5: Implement refinement planning**

For iterations after bootstrap or when a seed/incumbent exists:
- generate candidates by mutating/refining around the incumbent,
- avoid signatures already in `explored_signatures`,
- keep the first cut simple and deterministic.

- [ ] **Step 6: Implement reduce behavior**

`reduce()` should:
- preserve/update incumbent best,
- append signatures for completed candidates,
- update `search_phase` from bootstrap to refine once an incumbent exists,
- keep existing scoring-history and failure-count behavior aligned with swarm.

- [ ] **Step 7: Implement stop behavior**

`should_stop()` should reuse existing threshold/plateau/budget logic and additionally allow stop when no unexplored refinement candidates remain.

- [ ] **Step 8: Add unit tests**

In `tests/execution_search_strategy.rs`, add tests for:
- bootstrap with no seed,
- refine around incumbent,
- explored-signature avoidance,
- reduce updates incumbent and phase,
- stop when no new neighbors remain.

- [ ] **Step 9: Run the search strategy tests**

Run: `cargo test --features serde --test execution_search_strategy -- --nocapture`
Expected: PASS.

## Chunk 3: Service Wiring

### Task 3: Select strategy by execution mode

**Files:**
- Modify: `src/orchestration/service.rs`
- Test: `tests/execution_strategy_acceptance.rs`

- [ ] **Step 1: Write the failing acceptance test for `search`**

Create `tests/execution_strategy_acceptance.rs` with a test that:
- builds a valid `search` spec,
- runs it through `ExecutionService::run_to_completion(...)`,
- expects a valid terminal execution result.

- [ ] **Step 2: Run the new acceptance test to verify it fails**

Run: `cargo test --features serde --test execution_strategy_acceptance search_strategy_runs_end_to_end -- --exact --nocapture`
Expected: FAIL because the service still hardcodes `SwarmStrategy`.

- [ ] **Step 3: Add strategy selection in `ExecutionService`**

Update `src/orchestration/service.rs` so mode dispatch selects:
- `SwarmStrategy` for `swarm`
- `SearchStrategy` for `search`

Do not fork the execution loop. Keep only strategy creation mode-specific.

- [ ] **Step 4: Keep runtime/scheduler behavior unchanged**

Confirm no changes are needed to:
- candidate dispatch,
- artifact retrieval,
- bridge routes,
- scheduler rebuild,
- reconciliation.

- [ ] **Step 5: Run the targeted acceptance test**

Run: `cargo test --features serde --test execution_strategy_acceptance search_strategy_runs_end_to_end -- --exact --nocapture`
Expected: PASS.

## Chunk 4: Supported-Strategy Acceptance Suite

### Task 4: Add acceptance coverage for all supported strategies

**Files:**
- Create: `tests/execution_strategy_acceptance.rs`
- Modify: `tests/execution_bridge.rs`

- [ ] **Step 1: Add one shared acceptance helper**

In `tests/execution_strategy_acceptance.rs`, create a helper that:
- constructs a spec for a named mode,
- seeds the mock runtime,
- executes through the same orchestration path,
- asserts terminal success plus a non-empty result shape.

- [ ] **Step 2: Add one acceptance test per supported strategy**

Add tests for:
- `swarm_strategy_runs_end_to_end`
- `search_strategy_runs_end_to_end`

The point is not strategy-specific internals; the point is that every supported strategy runs successfully through the shared execution path.

- [ ] **Step 3: Add a bridge-level `search` route test**

In `tests/execution_bridge.rs`, add one test that:
- submits a `search` spec through `POST /v1/executions`,
- verifies the execution resource is created normally.

- [ ] **Step 4: Run the acceptance suite**

Run:

```bash
cargo test --features serde --test execution_strategy_acceptance -- --nocapture
cargo test --features serde --test execution_bridge -- --nocapture
```

Expected: PASS.

## Chunk 5: Final Verification

### Task 5: Verify the branch-wide strategy surface

**Files:**
- No new files

- [ ] **Step 1: Run focused strategy files**

Run:

```bash
cargo test --features serde --test execution_search_strategy -- --nocapture
cargo test --features serde --test execution_swarm_strategy -- --nocapture
cargo test --features serde --test execution_strategy_acceptance -- --nocapture
```

Expected: PASS.

- [ ] **Step 2: Run branch-wide verification**

Run:

```bash
cargo test --features serde
```

Expected: PASS.

- [ ] **Step 3: Optional live sanity**

If the local daemon is available, rerun:

```bash
TMPDIR=/tmp CARGO_TARGET_DIR=/home/diego/github/void-control/target VOID_BOX_BASE_URL=http://127.0.0.1:43100 cargo test --features serde --test execution_bridge_live -- --ignored --nocapture --test-threads=1
```

Expected: PASS. This is a regression check only; no search-specific live daemon fixture is required for the first cut.

