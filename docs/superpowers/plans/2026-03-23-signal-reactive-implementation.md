# Signal-Reactive Planning Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the `signal_reactive` variation mode in orchestration code, keeping `leader_directed` as legacy behavior while adding metadata-driven planning inputs for `swarm` and `search`.

**Architecture:** Extend orchestration types with a routed-message-based `MessageStats` summary, add deterministic extraction from persisted message-box state, teach `SwarmStrategy` and `SearchStrategy` to bias planning from those stats, and update variation/config parsing so `signal_reactive` is first-class without rewriting legacy `leader_directed` executions.

**Tech Stack:** Rust, Cargo tests, existing orchestration/message-box/store modules, serde-gated execution artifacts.

---

## Chunk 1: Variation Source And Public API

### Task 1: Add `signal_reactive` variation support without breaking legacy `leader_directed`

**Files:**
- Modify: `src/orchestration/variation.rs`
- Modify: `src/orchestration/spec.rs`
- Modify: `src/bridge.rs`
- Test: `tests/execution_swarm_strategy.rs`
- Test: `tests/execution_spec_validation.rs`

- [ ] **Step 1: Write failing tests for `signal_reactive` parsing/validation**
- [ ] **Step 2: Run the targeted tests and verify they fail for the missing mode**
- [ ] **Step 3: Add `VariationConfig::signal_reactive()` and generation behavior that keeps proposals planner-generated, not leader-authored**
- [ ] **Step 4: Update spec validation and bridge/config parsing to accept `signal_reactive` while preserving `leader_directed` as legacy**
- [ ] **Step 5: Run the targeted variation/spec tests and verify they pass**

## Chunk 2: MessageStats Extraction

### Task 2: Add `MessageStats` types and deterministic extraction from message-box state

**Files:**
- Modify: `src/orchestration/types.rs`
- Modify: `src/orchestration/message_box.rs`
- Modify: `src/orchestration/mod.rs`
- Test: `tests/execution_message_box.rs`

- [ ] **Step 1: Write failing tests for routed-message-based `MessageStats` extraction**
- [ ] **Step 2: Run the targeted message-box tests and verify they fail**
- [ ] **Step 3: Add `MessageStats` type plus extraction logic joined by `intent_id`**
- [ ] **Step 4: Export the new type/helpers through orchestration public API**
- [ ] **Step 5: Run the targeted message-box tests and verify they pass**

## Chunk 3: Strategy Consumption

### Task 3: Teach `swarm` and `search` to react to `MessageStats`

**Files:**
- Modify: `src/orchestration/strategy.rs`
- Modify: `src/orchestration/service.rs`
- Test: `tests/execution_swarm_strategy.rs`
- Test: `tests/execution_search_strategy.rs`
- Test: `tests/strategy_scenarios.rs`

- [ ] **Step 1: Write failing strategy tests for signal-reactive planning biases**
- [ ] **Step 2: Run the targeted strategy tests and verify they fail**
- [ ] **Step 3: Thread `MessageStats` into planning and implement minimal biasing behavior for `swarm` and `search`**
- [ ] **Step 4: Ensure empty-intent `search` falls back to incumbent-centered planning**
- [ ] **Step 5: Run the targeted strategy tests and verify they pass**

## Chunk 4: End-To-End Verification

### Task 4: Verify integrated execution behavior and prevent regression of legacy `leader_directed`

**Files:**
- Modify: `tests/execution_strategy_acceptance.rs`
- Modify: `tests/strategy_scenarios.rs`

- [ ] **Step 1: Add an acceptance test for `signal_reactive` execution planning**
- [ ] **Step 2: Add a regression test showing persisted `leader_directed` behavior still works as legacy mode**
- [ ] **Step 3: Run the focused acceptance/scenario tests and verify they pass**
- [ ] **Step 4: Run `cargo test --features serde` and verify the full suite stays green**
