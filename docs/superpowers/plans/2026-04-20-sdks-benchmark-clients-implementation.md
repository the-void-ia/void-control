# SDK Benchmark Clients Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a benchmark-oriented `void-control` SDK stack with Python first, then Node, then Go, all targeting one multi-candidate execution flow through the bridge API.

**Architecture:** Keep benchmark comparison inside `void-control` by adding a dedicated benchmark template/spec contract and exposing it through async-first SDK clients. The SDKs talk only to `void-control` bridge routes (`/v1/templates/...` and `/v1/executions/...`), not directly to `void-box`. Snapshot support in phase 1 is limited to passing an optional snapshot input through the benchmark template into runtime launch configuration; snapshot lifecycle management remains out of scope until `void-box` exposes daemon APIs for it.

**Tech Stack:** Rust bridge/template layer, checked-in YAML templates/specs, Python `httpx` + `pydantic`, Node `fetch`/TypeScript, Go `net/http`, pytest/vitest/go test.

---

## File Map

**Create:**

- `templates/benchmark-runner-python.yaml`
- `examples/sdk-benchmark-python.yaml`
- `sdks/python/pyproject.toml`
- `sdks/python/README.md`
- `sdks/python/src/void_control/__init__.py`
- `sdks/python/src/void_control/client.py`
- `sdks/python/src/void_control/templates.py`
- `sdks/python/src/void_control/executions.py`
- `sdks/python/src/void_control/models.py`
- `sdks/python/examples/benchmark_compare.py`
- `sdks/python/tests/test_client.py`
- `sdks/node/package.json`
- `sdks/node/tsconfig.json`
- `sdks/node/README.md`
- `sdks/node/src/index.ts`
- `sdks/node/src/client.ts`
- `sdks/node/src/templates.ts`
- `sdks/node/src/executions.ts`
- `sdks/node/src/models.ts`
- `sdks/node/examples/benchmarkCompare.ts`
- `sdks/node/test/client.test.ts`
- `sdks/go/go.mod`
- `sdks/go/README.md`
- `sdks/go/client.go`
- `sdks/go/templates.go`
- `sdks/go/executions.go`
- `sdks/go/models.go`
- `sdks/go/examples/benchmark_compare/main.go`
- `sdks/go/client_test.go`

**Modify:**

- `src/templates/schema.rs`
- `src/templates/compile.rs`
- `src/bridge.rs`
- `README.md`
- `AGENTS.md`
- `tests/template_api.rs`
- `tests/execution_bridge.rs`

**Reference:**

- `examples/runtime-templates/transform_optimizer_agent.yaml`
- `examples/runtime-assets/transform_benchmark.py`
- `docs/superpowers/specs/2026-04-20-template-first-agent-api-design.md`

## Chunk 1: Benchmark Template Contract

### Task 1: Add a benchmark-specific template and compile coverage

**Files:**
- Create: `templates/benchmark-runner-python.yaml`
- Modify: `src/templates/schema.rs`
- Modify: `src/templates/compile.rs`
- Test: `tests/template_api.rs`

- [ ] **Step 1: Write the failing template tests**

Add tests that load `benchmark-runner-python` and assert:
- execution kind remains execution-centric in phase 1
- multiple explicit candidates are supported for this template
- required inputs include benchmark goal plus optional snapshot input
- compilation populates candidate overrides and preserves multi-candidate shape

- [ ] **Step 2: Run the targeted template test**

Run: `cargo test --features serde --test template_api benchmark -- --nocapture`
Expected: FAIL with missing file or unsupported shape.

- [ ] **Step 3: Add the checked-in benchmark template**

Use `examples/runtime-templates/transform_optimizer_agent.yaml` as the backing workflow and define:
- one execution goal input
- candidate-specific strategy inputs or defaults
- optional snapshot input mapped into a runtime override only if present
- multiple explicit proposals for comparison in one execution

- [ ] **Step 4: Relax or extend template validation only where needed**

Allow the benchmark template to compile with multiple explicit candidates while keeping single-agent and warm-agent constraints intact.

- [ ] **Step 5: Re-run the targeted template test**

Run: `cargo test --features serde --test template_api benchmark -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add templates/benchmark-runner-python.yaml src/templates/schema.rs src/templates/compile.rs tests/template_api.rs
git commit -m "templates: add benchmark runner template"
```

### Task 2: Expose benchmark template behavior through bridge routes

**Files:**
- Modify: `src/bridge.rs`
- Test: `tests/execution_bridge.rs`

- [ ] **Step 1: Write the failing bridge tests**

Add tests for:
- `GET /v1/templates/benchmark-runner-python`
- `POST /v1/templates/benchmark-runner-python/dry-run`
- `POST /v1/templates/benchmark-runner-python/execute`

Assert the compiled summary includes multiple explicit candidates and benchmark-oriented overrides.

- [ ] **Step 2: Run the targeted bridge tests**

Run: `cargo test --features serde --test execution_bridge benchmark_runner_python -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Adjust bridge summary shaping if necessary**

If the current dry-run summary only shows the first override set, extend it so the benchmark template preview exposes all candidate override groups clearly.

- [ ] **Step 4: Re-run the targeted bridge tests**

Run: `cargo test --features serde --test execution_bridge benchmark_runner_python -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/bridge.rs tests/execution_bridge.rs
git commit -m "bridge: expose benchmark template execution flow"
```

## Chunk 2: Python SDK

### Task 3: Scaffold `sdks/python/` package structure

**Files:**
- Create: `sdks/python/pyproject.toml`
- Create: `sdks/python/README.md`
- Create: `sdks/python/src/void_control/__init__.py`
- Create: `sdks/python/src/void_control/client.py`
- Create: `sdks/python/src/void_control/templates.py`
- Create: `sdks/python/src/void_control/executions.py`
- Create: `sdks/python/src/void_control/models.py`
- Create: `sdks/python/tests/test_client.py`

- [ ] **Step 1: Write the failing Python SDK tests**

Add tests that assert:
- the client can be instantiated
- template and execution subclients are exposed
- request models and response parsing are wired

- [ ] **Step 2: Run the Python SDK tests**

Run: `pytest sdks/python/tests/test_client.py -q`
Expected: FAIL because the package does not exist yet.

- [ ] **Step 3: Scaffold the package in BoxLite-style layout**

Add:
- `pyproject.toml`
- package under `src/void_control/`
- README with install and quick start

- [ ] **Step 4: Add minimal async client and subclient wiring**

Use `httpx.AsyncClient` and expose:
- `VoidControlClient`
- `client.templates`
- `client.executions`

- [ ] **Step 5: Re-run the Python SDK tests**

Run: `pytest sdks/python/tests/test_client.py -q`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add sdks/python
git commit -m "sdk-python: scaffold async client package"
```

### Task 4: Implement template and execution methods in Python

**Files:**
- Modify: `sdks/python/src/void_control/client.py`
- Modify: `sdks/python/src/void_control/templates.py`
- Modify: `sdks/python/src/void_control/executions.py`
- Modify: `sdks/python/src/void_control/models.py`
- Modify: `sdks/python/tests/test_client.py`

- [ ] **Step 1: Write failing async method tests**

Add tests for:
- `templates.list()`
- `templates.get()`
- `templates.dry_run()`
- `templates.execute()`
- `executions.get()`
- `executions.wait()`

Mock bridge responses; do not hit the live network.

- [ ] **Step 2: Run the Python SDK tests**

Run: `pytest sdks/python/tests/test_client.py -q`
Expected: FAIL

- [ ] **Step 3: Implement the minimal async methods**

Keep the public surface small:
- async methods only
- typed response models
- one shared error type for non-2xx bridge responses
- `wait()` polls until `Completed`, `Failed`, or `Canceled`

- [ ] **Step 4: Re-run the Python SDK tests**

Run: `pytest sdks/python/tests/test_client.py -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add sdks/python/src/void_control sdks/python/tests/test_client.py
git commit -m "sdk-python: add template and execution clients"
```

### Task 5: Add the first benchmark example in Python

**Files:**
- Create: `sdks/python/examples/benchmark_compare.py`
- Modify: `sdks/python/README.md`
- Modify: `sdks/python/tests/test_client.py`

- [ ] **Step 1: Write the failing example test**

Add a test that exercises the example helpers or entrypoint logic against mocked bridge responses and asserts it:
- submits one benchmark template execution
- waits to terminal
- prints winner plus candidate metrics

- [ ] **Step 2: Run the example test**

Run: `pytest sdks/python/tests/test_client.py -q`
Expected: FAIL

- [ ] **Step 3: Implement the benchmark example**

Use:
- `benchmark-runner-python` template
- optional `snapshot` input pass-through
- one execution containing multiple candidates
- printed output: execution id, winner candidate, candidate metrics

- [ ] **Step 4: Re-run the Python test suite**

Run: `pytest sdks/python/tests/test_client.py -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add sdks/python/examples/benchmark_compare.py sdks/python/README.md sdks/python/tests/test_client.py
git commit -m "sdk-python: add benchmark comparison example"
```

## Chunk 3: Node SDK

### Task 6: Scaffold `sdks/node/` package

**Files:**
- Create: `sdks/node/package.json`
- Create: `sdks/node/tsconfig.json`
- Create: `sdks/node/README.md`
- Create: `sdks/node/src/index.ts`
- Create: `sdks/node/src/client.ts`
- Create: `sdks/node/src/templates.ts`
- Create: `sdks/node/src/executions.ts`
- Create: `sdks/node/src/models.ts`
- Create: `sdks/node/test/client.test.ts`

- [ ] **Step 1: Write the failing Node package tests**

Add tests for package exports and client construction.

- [ ] **Step 2: Run the Node tests**

Run: `cd sdks/node && npm test`
Expected: FAIL

- [ ] **Step 3: Scaffold the package**

Mirror the Python client contract as closely as TypeScript allows.

- [ ] **Step 4: Re-run the Node tests**

Run: `cd sdks/node && npm test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add sdks/node
git commit -m "sdk-node: scaffold async client package"
```

### Task 7: Implement Node methods and benchmark example

**Files:**
- Modify: `sdks/node/src/client.ts`
- Modify: `sdks/node/src/templates.ts`
- Modify: `sdks/node/src/executions.ts`
- Modify: `sdks/node/src/models.ts`
- Create: `sdks/node/examples/benchmarkCompare.ts`
- Modify: `sdks/node/test/client.test.ts`

- [ ] **Step 1: Write failing Node client tests**

Cover:
- template methods
- execution methods
- `wait()` polling
- benchmark example wiring

- [ ] **Step 2: Run the Node tests**

Run: `cd sdks/node && npm test`
Expected: FAIL

- [ ] **Step 3: Implement the minimal async client**

Use the built-in fetch-compatible path or a tiny dependency only if necessary.

- [ ] **Step 4: Re-run the Node tests**

Run: `cd sdks/node && npm test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add sdks/node/src sdks/node/examples sdks/node/test sdks/node/README.md
git commit -m "sdk-node: add benchmark client surface"
```

## Chunk 4: Go SDK

### Task 8: Scaffold `sdks/go/` package

**Files:**
- Create: `sdks/go/go.mod`
- Create: `sdks/go/README.md`
- Create: `sdks/go/client.go`
- Create: `sdks/go/templates.go`
- Create: `sdks/go/executions.go`
- Create: `sdks/go/models.go`
- Create: `sdks/go/client_test.go`

- [ ] **Step 1: Write the failing Go tests**

Cover:
- client construction
- template and execution method signatures

- [ ] **Step 2: Run the Go tests**

Run: `cd sdks/go && go test ./...`
Expected: FAIL

- [ ] **Step 3: Scaffold the package**

Keep the same conceptual surface:
- templates
- executions
- wait helper

- [ ] **Step 4: Re-run the Go tests**

Run: `cd sdks/go && go test ./...`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add sdks/go
git commit -m "sdk-go: scaffold client package"
```

### Task 9: Add the Go benchmark example

**Files:**
- Create: `sdks/go/examples/benchmark_compare/main.go`
- Modify: `sdks/go/client_test.go`
- Modify: `sdks/go/README.md`

- [ ] **Step 1: Write the failing example test**

Assert the example logic:
- submits the benchmark template
- waits for terminal
- prints winner and metrics

- [ ] **Step 2: Run the Go tests**

Run: `cd sdks/go && go test ./...`
Expected: FAIL

- [ ] **Step 3: Implement the example**

Use the same benchmark template and response semantics as Python and Node.

- [ ] **Step 4: Re-run the Go tests**

Run: `cd sdks/go && go test ./...`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add sdks/go/examples/benchmark_compare/main.go sdks/go/client_test.go sdks/go/README.md
git commit -m "sdk-go: add benchmark comparison example"
```

## Chunk 5: Docs and Final Verification

### Task 10: Document the SDK workspace and benchmark flow

**Files:**
- Modify: `README.md`
- Modify: `AGENTS.md`

- [ ] **Step 1: Update top-level docs**

Document:
- `sdks/` workspace layout
- Python-first benchmark example
- Node and Go follow-up SDKs
- current snapshot limitation: launch-time restore input only, no snapshot lifecycle API yet

- [ ] **Step 2: Run formatting and relevant verification**

Run:
- `cargo fmt --all`
- `cargo test --features serde`
- `pytest sdks/python/tests/test_client.py -q`
- `cd sdks/node && npm test`
- `cd sdks/go && go test ./...`

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add README.md AGENTS.md
git commit -m "docs: describe sdk benchmark workflow"
```
