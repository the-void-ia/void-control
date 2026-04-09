# Void-Control CLI Skill Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a non-interactive `voidctl execution ...` operator surface and a first `void-control` skill that can submit, inspect, watch, and summarize both orchestration specs and raw runtime specs through the same execution model.

**Architecture:** Extend `voidctl` with dedicated execution subcommands that call the existing bridge APIs instead of raw `curl`. Add bridge-side classification and minimal wrapping for raw runtime specs, persist submitted spec text for fileless agent-generated launches, and then write a thin `void-control` skill that shells out to those commands.

**Tech Stack:** Rust (`voidctl`, bridge, orchestration store), existing bridge HTTP routes, serde YAML/JSON parsing, markdown skill docs.

---

## File Map

- Modify: `src/bin/voidctl.rs`
  - Add non-interactive `execution` subcommands for submit, dry-run, watch, inspect, events, result, and runtime.
- Modify: `src/bridge.rs`
  - Add/extend request parsing, spec classification, runtime wrapping, and persisted submitted-spec handling.
- Modify: `src/orchestration/spec.rs`
  - Add helper(s) if needed for classifying orchestration-vs-runtime spec input cleanly.
- Modify: `src/orchestration/store/fs.rs`
  - Persist and reload submitted spec text or wrapped runtime-spec records if new store support is needed.
- Modify: `src/orchestration/service.rs`
  - Reuse existing execution loading/result logic for runtime-wrapped executions rather than inventing a second reporting path.
- Modify: `tests/execution_bridge.rs`
  - Add bridge coverage for stdin/fileless submissions and runtime wrapping.
- Modify: `tests/execution_spec_validation.rs`
  - Add classification coverage and ambiguous-input failure cases.
- Modify: `tests/execution_strategy_acceptance.rs`
  - Add execution-level acceptance coverage for wrapped runtime specs if current tests do not already cover that operator path.
- Modify: `README.md`
  - Add the new CLI workflow once the implementation exists.
- Create: `.claude/skills/void-control/SKILL.md`
  - First repo-local operator skill for the new CLI.
- Create: `docs/superpowers/specs/2026-04-07-void-control-cli-skill-design.md`
  - Already written; use as the implementation reference.
- Test: `tests/execution_bridge.rs`
- Test: `tests/execution_spec_validation.rs`
- Test: `tests/execution_strategy_acceptance.rs`

## Chunk 1: CLI Surface

### Task 1: Add non-interactive execution subcommand parsing

**Files:**
- Modify: `src/bin/voidctl.rs`
- Test: `src/bin/voidctl.rs` existing parser logic

- [ ] **Step 1: Write the failing parser test or isolate parser cases**

Add coverage or refactorable assertions for:
- `voidctl execution submit <spec-path>`
- `voidctl execution submit --stdin`
- `voidctl execution dry-run <spec-path>`
- `voidctl execution dry-run --stdin`
- `voidctl execution watch <execution-id>`
- `voidctl execution inspect <execution-id>`
- `voidctl execution events <execution-id>`
- `voidctl execution result <execution-id>`
- `voidctl execution runtime <execution-id> [candidate-id]`

- [ ] **Step 2: Run the targeted parser test or compile check**

Run: `cargo test --features serde --bin voidctl -- --nocapture`
Expected: FAIL on unknown subcommands or missing parse support.

- [ ] **Step 3: Implement minimal command enum and parse changes**

Update `Command` in `src/bin/voidctl.rs` to include:
- `ExecutionSubmit { spec: Option<String>, stdin: bool }`
- `ExecutionDryRun { spec: Option<String>, stdin: bool }`
- `ExecutionWatch { execution_id: String }`
- `ExecutionInspect { execution_id: String }`
- `ExecutionEvents { execution_id: String }`
- `ExecutionResult { execution_id: String }`
- `ExecutionRuntime { execution_id: String, candidate_id: Option<String> }`

Update help text and completion candidates to reflect the new non-interactive surface.

- [ ] **Step 4: Run the targeted parser test again**

Run: `cargo test --features serde --bin voidctl -- --nocapture`
Expected: PASS for command parsing.

- [ ] **Step 5: Commit**

```bash
git add src/bin/voidctl.rs
git commit -m "cli: add execution subcommand surface"
```

### Task 2: Add bridge-backed execution command handlers

**Files:**
- Modify: `src/bin/voidctl.rs`

- [ ] **Step 1: Write a focused failing test or harnessable helper test**

Cover helper behavior for:
- reading YAML from file path
- reading YAML from stdin
- printing execution summary
- printing result/runtime resolution output

- [ ] **Step 2: Run targeted test**

Run: `cargo test --features serde --bin voidctl -- --nocapture`
Expected: FAIL for missing handlers.

- [ ] **Step 3: Implement minimal bridge client helpers**

In `src/bin/voidctl.rs`, add reusable helpers to:
- post raw YAML or wrapped JSON to `/v1/executions`
- post raw YAML to `/v1/executions/dry-run`
- fetch `/v1/executions/<id>`
- fetch `/v1/executions/<id>/events`
- poll for `watch`

Keep output deterministic and text-first.

- [ ] **Step 4: Re-run targeted test**

Run: `cargo test --features serde --bin voidctl -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/bin/voidctl.rs
git commit -m "cli: wire execution commands to bridge"
```

## Chunk 2: Bridge Classification And Wrapping

### Task 3: Add spec classification helpers

**Files:**
- Modify: `src/bridge.rs`
- Modify: `src/orchestration/spec.rs`
- Test: `tests/execution_spec_validation.rs`

- [ ] **Step 1: Write the failing classification tests**

Add tests for:
- native orchestration spec accepted as orchestration
- raw runtime workload/agent/pipeline spec detected as runtime
- ambiguous document rejected with a clear error
- invalid document rejected with current validation behavior

- [ ] **Step 2: Run the targeted tests**

Run: `cargo test --features serde --test execution_spec_validation -- --nocapture`
Expected: FAIL on missing classification helpers.

- [ ] **Step 3: Implement minimal classification logic**

Add a helper that inspects submitted YAML and classifies it as:
- orchestration execution spec
- raw runtime spec
- invalid/ambiguous

Do not hardcode swarm-only behavior into the top-level classification.

- [ ] **Step 4: Re-run the targeted tests**

Run: `cargo test --features serde --test execution_spec_validation -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/bridge.rs src/orchestration/spec.rs tests/execution_spec_validation.rs
git commit -m "bridge: classify execution submissions"
```

### Task 4: Support stdin/fileless submission by persisting submitted spec text

**Files:**
- Modify: `src/bridge.rs`
- Modify: `src/orchestration/store/fs.rs`
- Test: `tests/execution_bridge.rs`

- [ ] **Step 1: Write the failing bridge tests**

Add tests for:
- `POST /v1/executions` from inline YAML without a permanent source file
- `POST /v1/executions/dry-run` from inline YAML
- persisted execution retains submitted spec text or equivalent recoverable source

- [ ] **Step 2: Run the targeted tests**

Run: `cargo test --features serde --test execution_bridge -- --nocapture`
Expected: FAIL on missing inline-spec persistence.

- [ ] **Step 3: Implement minimal storage support**

Extend bridge/store code so inline-submitted specs:
- are accepted without a source path
- are persisted in execution storage
- remain reloadable for inspect/result flows

Prefer a small store addition over inventing a separate submission cache.

- [ ] **Step 4: Re-run the targeted tests**

Run: `cargo test --features serde --test execution_bridge -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/bridge.rs src/orchestration/store/fs.rs tests/execution_bridge.rs
git commit -m "bridge: persist inline execution specs"
```

### Task 5: Wrap raw runtime specs into minimal executions

**Files:**
- Modify: `src/bridge.rs`
- Modify: `src/orchestration/service.rs`
- Test: `tests/execution_bridge.rs`
- Test: `tests/execution_strategy_acceptance.rs`

- [ ] **Step 1: Write the failing wrapping test**

Add coverage that:
- submits a raw runtime spec
- receives an `execution_id`
- can load execution detail through the bridge
- does not create a parallel runtime-only model

- [ ] **Step 2: Run the targeted tests**

Run: `cargo test --features serde --test execution_bridge -- --nocapture`
Expected: FAIL because raw runtime specs still go through separate launch paths.

- [ ] **Step 3: Implement minimal wrapping**

Bridge-side behavior:
- classify raw runtime spec
- wrap into a minimal control-plane execution document
- preserve original runtime payload
- launch through existing runtime path
- persist execution state so later CLI commands can inspect it uniformly

- [ ] **Step 4: Add acceptance coverage**

Add one acceptance test that proves a wrapped runtime execution can be:
- submitted
- inspected
- summarized as an execution

- [ ] **Step 5: Re-run both test targets**

Run:
- `cargo test --features serde --test execution_bridge -- --nocapture`
- `cargo test --features serde --test execution_strategy_acceptance -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/bridge.rs src/orchestration/service.rs tests/execution_bridge.rs tests/execution_strategy_acceptance.rs
git commit -m "bridge: wrap runtime specs as executions"
```

## Chunk 3: Result And Runtime Drill-Down

### Task 6: Implement `inspect`, `events`, and `result` output

**Files:**
- Modify: `src/bin/voidctl.rs`
- Test: `tests/execution_bridge.rs`

- [ ] **Step 1: Write focused failing output tests**

Cover:
- orchestration inspect output includes best candidate and metrics
- runtime-wrapped inspect output includes stage/result summary
- events output renders execution events in stable order
- result output resolves winner/runtime summary correctly

- [ ] **Step 2: Run targeted tests**

Run: `cargo test --features serde --test execution_bridge -- --nocapture`
Expected: FAIL because output formatters do not exist yet.

- [ ] **Step 3: Implement minimal formatters**

In `src/bin/voidctl.rs`, add small formatting helpers for:
- execution summary
- candidate list
- event lines
- final result summary

Avoid terminal-only UI tricks. Optimize for deterministic text.

- [ ] **Step 4: Re-run targeted tests**

Run: `cargo test --features serde --test execution_bridge -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/bin/voidctl.rs tests/execution_bridge.rs
git commit -m "cli: add execution inspect and result output"
```

### Task 7: Implement `watch` and `runtime`

**Files:**
- Modify: `src/bin/voidctl.rs`
- Test: `tests/execution_bridge.rs`

- [ ] **Step 1: Write focused failing tests**

Cover:
- `watch` polling uses execution state, not raw runtime state
- `runtime` resolves best candidate runtime by default
- `runtime` resolves explicit candidate runtime when provided
- runtime resolution errors are clear when no runtime handle exists

- [ ] **Step 2: Run targeted tests**

Run: `cargo test --features serde --test execution_bridge -- --nocapture`
Expected: FAIL on missing watch/runtime resolution.

- [ ] **Step 3: Implement minimal polling and resolution**

`watch`:
- poll `/v1/executions/<id>`
- stop on terminal state
- print compact deltas only

`runtime`:
- choose best candidate when present
- otherwise choose the most relevant running candidate
- print runtime run ID deterministically

- [ ] **Step 4: Re-run targeted tests**

Run: `cargo test --features serde --test execution_bridge -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/bin/voidctl.rs tests/execution_bridge.rs
git commit -m "cli: add execution watch and runtime resolution"
```

## Chunk 4: Skill And Docs

### Task 8: Add the first repo-local `void-control` skill

**Files:**
- Create: `.claude/skills/void-control/SKILL.md`
- Modify: `README.md`

- [ ] **Step 1: Write the skill doc**

Document the operator flow:
- generate or locate spec
- `voidctl execution submit <path>` or `--stdin`
- `watch`
- `inspect`
- `result`
- `runtime`

Make it explicit that the skill is over `void-control`, not `void-box`.

- [ ] **Step 2: Add README CLI examples**

Add a compact section showing:
- swarm submission
- pipeline/workload submission
- generated YAML via stdin

- [ ] **Step 3: Verify docs are coherent**

Read:
- `README.md`
- `.claude/skills/void-control/SKILL.md`
- `docs/superpowers/specs/2026-04-07-void-control-cli-skill-design.md`

Expected: no contradiction between CLI commands and documented behavior.

- [ ] **Step 4: Commit**

```bash
git add .claude/skills/void-control/SKILL.md README.md
git commit -m "docs: add void-control operator skill"
```

## Chunk 5: Final Verification

### Task 9: Run the quality gate and smoke the new CLI surface

**Files:**
- Modify: none unless fixes are required

- [ ] **Step 1: Run format check**

Run: `cargo fmt --all -- --check`
Expected: PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS

- [ ] **Step 3: Run serde test suite**

Run: `cargo test --features serde`
Expected: PASS

- [ ] **Step 4: Run focused CLI smoke checks**

Run representative commands against a local bridge, for example:

```bash
cargo run --features serde --bin voidctl -- serve
voidctl execution dry-run examples/swarm-transform-optimization-3way.yaml
voidctl execution submit examples/swarm-transform-optimization-3way.yaml
cat examples/swarm-transform-optimization-3way.yaml | voidctl execution dry-run --stdin
```

Expected:
- dry-run returns candidate/iteration summary
- submit returns an execution ID
- stdin path behaves the same as file path for orchestration input

- [ ] **Step 5: Commit final fixes if needed**

```bash
git add <files>
git commit -m "cli: polish execution operator workflow"
```

