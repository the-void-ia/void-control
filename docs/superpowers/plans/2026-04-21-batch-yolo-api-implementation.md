# Batch / Yolo API Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a high-level `batch` authoring surface to `void-control` for remote background/offloaded work, with `yolo` accepted as an alias, compiling into the existing execution engine instead of exposing raw `ExecutionSpec` authoring to users.

**Architecture:** Introduce a small `src/batch/` module that parses `BatchSpec`, normalizes `kind: yolo` to `batch`, validates jobs and worker defaults, and compiles the spec into a normal `ExecutionSpec` using the current swarm machinery. Add thin bridge routes for `run`, `dry-run`, and `inspect`, and extend SDKs with `batch` plus `yolo` alias helpers.

**Tech Stack:** Rust, serde/serde_yaml/serde_json, existing `bridge.rs`, existing `ExecutionSpec` validation and orchestration service, Python/Node/Go SDKs already in-repo.

---

## File Map

**Create:**

- `src/batch/mod.rs`
- `src/batch/schema.rs`
- `src/batch/compile.rs`
- `tests/batch_api.rs`
- `examples/batch/background_repo_work.yaml`

**Modify:**

- `src/lib.rs`
- `src/bridge.rs`
- `src/bin/voidctl.rs`
- `README.md`
- `AGENTS.md`
- `sdks/python/src/void_control/client.py`
- `sdks/python/src/void_control/models.py`
- `sdks/python/tests/test_client.py`
- `sdks/node/src/client.js`
- `sdks/node/src/index.js`
- `sdks/node/test/client.test.mjs`
- `sdks/go/client.go`
- `sdks/go/client_test.go`

**Reference:**

- `docs/superpowers/specs/2026-04-21-team-spec-authoring-draft.md`
- `src/orchestration/spec.rs`
- `tests/execution_bridge.rs`

## Chunk 1: Batch Core

### Task 1: Add batch module skeleton

**Files:**
- Create: `src/batch/mod.rs`
- Modify: `src/lib.rs`
- Test: `tests/batch_api.rs`

- [ ] **Step 1: Write the failing test/module import**

Add a tiny test that references a public `void_control::batch` export.

- [ ] **Step 2: Run the targeted test**

Run: `cargo test --features serde --test batch_api module -- --nocapture`
Expected: FAIL with unresolved module or missing export.

- [ ] **Step 3: Add the module skeleton**

Create `src/batch/mod.rs` with public exports for:
- schema
- parse
- compile

- [ ] **Step 4: Export the module from `src/lib.rs`**

Add `pub mod batch;`.

- [ ] **Step 5: Re-run the targeted test**

Run: `cargo test --features serde --test batch_api module -- --nocapture`
Expected: FAIL later, now due to missing schema behavior instead of missing module wiring.

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs src/batch/mod.rs tests/batch_api.rs
git commit -m "batch: add module skeleton"
```

### Task 2: Define `BatchSpec` and alias normalization

**Files:**
- Create: `src/batch/schema.rs`
- Modify: `src/batch/mod.rs`
- Test: `tests/batch_api.rs`

- [ ] **Step 1: Write failing schema tests**

Add tests that parse inline YAML/JSON and assert support for:
- `kind: batch`
- `kind: yolo` normalized to `batch`
- `worker.template`
- `jobs[*].prompt`
- `mode.parallelism`

- [ ] **Step 2: Run the targeted schema tests**

Run: `cargo test --features serde --test batch_api schema -- --nocapture`
Expected: FAIL because schema/types do not exist yet.

- [ ] **Step 3: Implement schema types**

Define:
- `BatchSpec`
- `BatchMetadata`
- `BatchWorker`
- `BatchMode`
- `BatchJob`

And parsing helpers:
- `parse_batch_yaml`
- `parse_batch_json`

- [ ] **Step 4: Add validation and normalization**

Validate:
- `kind` must be `batch` or `yolo`
- normalize `yolo -> batch`
- at least one job
- each job has a prompt
- positive `parallelism`

- [ ] **Step 5: Re-run the targeted schema tests**

Run: `cargo test --features serde --test batch_api schema -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/batch/mod.rs src/batch/schema.rs tests/batch_api.rs
git commit -m "batch: add spec schema"
```

### Task 3: Compile `BatchSpec` into `ExecutionSpec`

**Files:**
- Create: `src/batch/compile.rs`
- Modify: `src/batch/mod.rs`
- Test: `tests/batch_api.rs`

- [ ] **Step 1: Write failing compile tests**

Add tests that compile a batch spec and assert:
- `mode == "swarm"`
- `variation.source == "explicit"`
- one explicit proposal per job
- `candidates_per_iteration` tracks job count or requested parallelism appropriately
- worker template/prompt/provider data lands in candidate overrides

- [ ] **Step 2: Run the targeted compile tests**

Run: `cargo test --features serde --test batch_api compile -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Implement compile logic**

Recommended mapping:
- `mode: swarm`
- `swarm: true`
- `variation.source: explicit`
- one explicit proposal per job
- `candidates_per_iteration = min(job_count, parallelism)`
- simple success-oriented evaluation defaults
- conservative failure defaults for batch background work

- [ ] **Step 4: Re-run the targeted compile tests**

Run: `cargo test --features serde --test batch_api compile -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/batch/mod.rs src/batch/compile.rs tests/batch_api.rs
git commit -m "batch: compile specs into execution plans"
```

## Chunk 2: Bridge API

### Task 4: Add batch bridge routes

**Files:**
- Modify: `src/bridge.rs`
- Test: `tests/batch_api.rs`

- [ ] **Step 1: Write failing bridge tests**

Add tests for:
- `POST /v1/batch/dry-run`
- `POST /v1/batch/run`
- `GET /v1/batch-runs/{id}`
- `POST /v1/yolo/run` alias

Assert:
- `kind: yolo` normalizes correctly
- response includes `compiled_primitive = "swarm"`
- persisted execution exists and is a normal `Execution`

- [ ] **Step 2: Run the targeted bridge tests**

Run: `cargo test --features serde --test batch_api bridge -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Implement the routes**

Suggested routes:
- `POST /v1/batch/dry-run`
- `POST /v1/batch/run`
- `GET /v1/batch-runs/{id}`

Accepted aliases:
- `POST /v1/yolo/dry-run`
- `POST /v1/yolo/run`
- `GET /v1/yolo-runs/{id}`

- [ ] **Step 4: Re-run the targeted bridge tests**

Run: `cargo test --features serde --test batch_api bridge -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/bridge.rs tests/batch_api.rs
git commit -m "bridge: add batch run routes"
```

### Task 5: Add a checked-in batch example

**Files:**
- Create: `examples/batch/background_repo_work.yaml`
- Test: `tests/batch_api.rs`

- [ ] **Step 1: Write a failing example-load test**

Assert the example file exists, parses, and compiles.

- [ ] **Step 2: Run the targeted test**

Run: `cargo test --features serde --test batch_api example -- --nocapture`
Expected: FAIL with missing file.

- [ ] **Step 3: Add the example**

Model a simple background repo work batch with:
- worker template
- provider
- parallelism
- 3 jobs

- [ ] **Step 4: Re-run the targeted test**

Run: `cargo test --features serde --test batch_api example -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add examples/batch/background_repo_work.yaml tests/batch_api.rs
git commit -m "examples: add batch background work example"
```

## Chunk 3: CLI

### Task 6: Add `voidctl batch ...` commands

**Files:**
- Modify: `src/bin/voidctl.rs`
- Test: `tests/voidctl_execution_cli.rs`

- [ ] **Step 1: Write failing CLI tests**

Add coverage for:
- `voidctl batch dry-run --stdin`
- `voidctl batch run --stdin`
- `voidctl batch inspect <run-id>`
- alias form `voidctl yolo run --stdin`

- [ ] **Step 2: Run the targeted CLI tests**

Run: `cargo test --features serde --test voidctl_execution_cli batch -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Implement CLI commands**

Recommended commands:
- `voidctl batch dry-run <spec-path>|--stdin`
- `voidctl batch run <spec-path>|--stdin`
- `voidctl batch inspect <run-id>`

Accepted alias:
- `voidctl yolo ...`

- [ ] **Step 4: Re-run the targeted CLI tests**

Run: `cargo test --features serde --test voidctl_execution_cli batch -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/bin/voidctl.rs tests/voidctl_execution_cli.rs
git commit -m "cli: add batch commands"
```

## Chunk 4: SDKs

### Task 7: Extend Python SDK with `batch` and `yolo`

**Files:**
- Modify: `sdks/python/src/void_control/client.py`
- Modify: `sdks/python/src/void_control/models.py`
- Modify: `sdks/python/tests/test_client.py`

- [ ] **Step 1: Write failing SDK tests**

Add tests for:
- `client.batch.run(...)`
- `client.batch_runs.wait(...)`
- alias `client.yolo.run(...)`

- [ ] **Step 2: Run the Python SDK tests**

Run: `python3 -m unittest sdks.python.tests.test_client`
Expected: FAIL

- [ ] **Step 3: Implement the client surface**

Add canonical:
- `batch`
- `batch_runs`

And alias:
- `yolo`
- `yolo_runs`

- [ ] **Step 4: Re-run the Python SDK tests**

Run: `python3 -m unittest sdks.python.tests.test_client`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add sdks/python
git commit -m "sdk-python: add batch client surface"
```

### Task 8: Extend Node and Go SDKs with `batch` and `yolo`

**Files:**
- Modify: `sdks/node/src/client.js`
- Modify: `sdks/node/test/client.test.mjs`
- Modify: `sdks/go/client.go`
- Modify: `sdks/go/client_test.go`

- [ ] **Step 1: Write failing Node/Go tests**

Add coverage for canonical plus alias surfaces.

- [ ] **Step 2: Run the targeted SDK tests**

Run:
- `node --test sdks/node/test/client.test.mjs`
- `cd sdks/go && GOCACHE=/tmp/go-build go test ./...`

Expected: FAIL

- [ ] **Step 3: Implement canonical plus alias clients**

Node:
- `client.batch`
- `client.batchRuns`
- `client.yolo`
- `client.yoloRuns`

Go:
- `client.Batch`
- `client.BatchRuns`
- `client.Yolo`
- `client.YoloRuns`

- [ ] **Step 4: Re-run the targeted SDK tests**

Run:
- `node --test sdks/node/test/client.test.mjs`
- `cd sdks/go && GOCACHE=/tmp/go-build go test ./...`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add sdks/node sdks/go
git commit -m "sdks: add batch and yolo aliases"
```

## Chunk 5: Docs and Verification

### Task 9: Document the new authoring surface

**Files:**
- Modify: `README.md`
- Modify: `AGENTS.md`

- [ ] **Step 1: Update docs**

Document:
- what `batch` is for
- that `yolo` is an alias
- example routes and CLI usage
- relation to raw `ExecutionSpec`

- [ ] **Step 2: Run focused verification**

Run:
- `cargo fmt --all -- --check`
- `cargo test --features serde --test batch_api -- --nocapture`
- `cargo test --features serde --test voidctl_execution_cli batch -- --nocapture`
- `python3 -m unittest sdks.python.tests.test_client`
- `node --test sdks/node/test/client.test.mjs`
- `cd sdks/go && GOCACHE=/tmp/go-build go test ./...`

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add README.md AGENTS.md
git commit -m "docs: add batch authoring workflow"
```

## Recommended Execution Order

1. batch module and schema
2. compile layer
3. bridge routes
4. example spec
5. CLI
6. Python SDK
7. Node and Go SDKs
8. docs and final verification

## Recommendation

Implement `batch` first.

Why:

- it directly addresses the remote offload/background use case
- it is a much thinner layer over the existing swarm engine than `TeamSpec`
- it gives immediate user-facing value without requiring the compute sandbox API first
- it lets `void-control` improve usability now while preserving the current execution engine
