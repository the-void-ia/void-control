# Compute Sandbox API Phase 1 Implementation Plan

## Date

2026-04-21

## Scope

This plan covers the first credible compute-API slice after the template,
batch/yolo, and team authoring work.

It does **not** pretend that `void-control` can ship a full BoxLite/E2B-style
compute API by itself today. The current `void-box` daemon still exposes a
run-centric surface, not first-class sandbox, exec, and snapshot-management
routes.

So this plan is deliberately split into:

1. control-plane contract work in `void-control`
2. prerequisite daemon/runtime work in `void-box`
3. bridge and SDK wiring only after those runtime primitives exist

## Current Boundary

`void-control` currently has:

- `src/bridge.rs`
  - execution/template/batch/team bridge routes
- `src/runtime/mod.rs`
  - `ExecutionRuntime` with run-only lifecycle methods
- `src/runtime/void_box.rs`
  - `VoidBoxRuntimeClient` over `/v1/runs`

`void-box` currently has:

- daemon routes for `/v1/runs`
- internal snapshot store support
- run creation support for snapshot restore
- no public daemon routes yet for:
  - sandbox create/get/list/remove
  - exec against an existing sandbox
  - snapshot create/list/get/delete/replicate

## Goal

Define and stage a compute-oriented API around:

- `SandboxSpec`
- `SnapshotSpec`
- `SandboxPoolSpec`

without violating the `void-control` / `void-box` boundary.

## File Map

### `void-control`

- `docs/superpowers/specs/2026-04-21-compute-sandbox-api-draft.md`
  - design source of truth
- `docs/superpowers/plans/2026-04-21-compute-sandbox-api-phase1-implementation.md`
  - this implementation plan
- future Rust files after prerequisite runtime support exists:
  - `src/sandbox/mod.rs`
  - `src/sandbox/schema.rs`
  - `src/sandbox/compile.rs`
  - `src/bridge.rs`
  - `src/bin/voidctl.rs`
  - `tests/sandbox_api.rs`
  - `tests/voidctl_execution_cli.rs`
  - `sdks/python/src/void_control/...`
  - `sdks/node/src/...`
  - `sdks/go/...`

### `void-box`

- `src/daemon.rs`
- `src/snapshot_store.rs`
- runtime/backend modules for sandbox lifecycle and exec

## Phase Split

## Chunk 1: Freeze The Control-Plane Contract

Purpose:
- make the compute object model explicit and truthful in docs before code

Files:
- `docs/superpowers/specs/2026-04-21-compute-sandbox-api-draft.md`
- `README.md`
- `AGENTS.md`

Steps:
- [ ] **Step 1: tighten the draft around current runtime reality**
  - note that `void-control` cannot yet expose a real sandbox API because the
    `void-box` daemon remains run-centric
  - define canonical nouns:
    - `SandboxSpec`
    - `SnapshotSpec`
    - `SandboxPoolSpec`

- [ ] **Step 2: define the phase-1 public API shape**
  - document the intended control-plane routes:
    - `POST /v1/sandboxes`
    - `GET /v1/sandboxes`
    - `GET /v1/sandboxes/{id}`
    - `POST /v1/sandboxes/{id}/exec`
    - `POST /v1/sandboxes/{id}/stop`
    - `DELETE /v1/sandboxes/{id}`
  - document that these stay unimplemented in `void-control` until the runtime
    daemon contract exists

- [ ] **Step 3: define snapshot and pool routes**
  - snapshots:
    - `POST /v1/snapshots`
    - `GET /v1/snapshots`
    - `GET /v1/snapshots/{id}`
    - `POST /v1/snapshots/{id}/replicate`
    - `DELETE /v1/snapshots/{id}`
  - pools:
    - `POST /v1/pools`
    - `GET /v1/pools/{id}`
    - `POST /v1/pools/{id}/scale`

- [ ] **Step 4: document the ComputeSDK compatibility mapping**
  - `compute.sandbox.create` -> `/v1/sandboxes`
  - `compute.sandbox.runCommand` -> `/v1/sandboxes/{id}/exec`
  - `compute.sandbox.runCode` -> `/v1/sandboxes/{id}/exec`
  - `compute.sandbox.destroy` -> `DELETE /v1/sandboxes/{id}`

- [ ] **Step 5: commit**

```bash
git add docs/superpowers/specs/2026-04-21-compute-sandbox-api-draft.md README.md AGENTS.md
git commit -m "docs: freeze compute sandbox contract"
```

## Chunk 2: Define The `void-box` Prerequisite Work

Purpose:
- avoid implementing a fake `void-control` API against missing runtime routes

Files:
- external dependency work in `/home/diego/github/agent-infra/void-box`
- optional follow-up note in `docs/`
- `docs/superpowers/specs/2026-04-27-void-box-compute-daemon-prerequisites.md`

Steps:
- [ ] **Step 1: write the daemon prerequisite list**
  - `void-box` daemon must expose:
    - sandbox create/get/list/remove
    - exec against an existing sandbox
    - snapshot create/get/list/delete
    - snapshot replication or a lower-level primitive that `void-control`
      can orchestrate

- [ ] **Step 2: specify the minimum request/response contracts**
  - sandbox create returns sandbox id, state, node, restored snapshot metadata
  - exec returns command/code result plus timing and exit status
  - snapshot create returns snapshot id plus source sandbox metadata
  - snapshot replicate returns target-node state per node

- [ ] **Step 3: define what can remain internal**
  - raw microVM internals
  - node-local snapshot storage layout
  - backend-specific restore mechanics

- [ ] **Step 4: document the cross-repo dependency**
  - `void-control` implementation starts only after those daemon routes exist

## Chunk 3: Add `SandboxSpec` Types In `void-control`

Purpose:
- once daemon support exists, add typed control-plane models with no bridge
  routes yet

Files:
- `src/sandbox/mod.rs`
- `src/sandbox/schema.rs`
- `src/lib.rs`
- `tests/sandbox_api.rs`

Steps:
- [ ] **Step 1: write failing parser tests for `SandboxSpec`**
  - parse YAML and JSON
  - reject missing runtime section
  - reject invalid lifecycle values
  - reject invalid snapshot distribution modes

- [ ] **Step 2: implement schema types**
  - `SandboxSpec`
  - `SnapshotSpec`
  - `SandboxPoolSpec`
  - use public doc comments per `rustdoc`

- [ ] **Step 3: export the new module**
  - wire `src/lib.rs`

- [ ] **Step 4: run targeted tests**

```bash
cargo test --features serde --test sandbox_api -- --nocapture
```

- [ ] **Step 5: commit**

```bash
git add src/sandbox src/lib.rs tests/sandbox_api.rs
git commit -m "sandbox: add compute api schemas"
```

## Chunk 4: Add Runtime Adapter Traits

Purpose:
- avoid shoving sandbox lifecycle into the run-only `ExecutionRuntime` trait

Files:
- `src/runtime/mod.rs`
- `src/runtime/void_box.rs`
- `src/runtime/mock.rs`
- new tests near runtime modules

Steps:
- [ ] **Step 1: define a dedicated sandbox runtime trait**
  - `create_sandbox`
  - `inspect_sandbox`
  - `list_sandboxes`
  - `exec_sandbox`
  - `stop_sandbox`
  - `delete_sandbox`
  - later snapshot operations through either the same trait or a sibling trait

- [ ] **Step 2: keep `ExecutionRuntime` unchanged**
  - do not overload orchestration runtime calls with compute semantics

- [ ] **Step 3: implement the trait for mock runtime first**
  - establish testable bridge behavior without live daemon dependency

- [ ] **Step 4: implement the trait for `VoidBoxRuntimeClient`**
  - only after the daemon contract exists

- [ ] **Step 5: run focused runtime tests**

```bash
cargo test --features serde runtime:: -- --nocapture
```

- [ ] **Step 6: commit**

```bash
git add src/runtime
git commit -m "runtime: add sandbox lifecycle adapter"
```

## Chunk 5: Add Bridge Routes

Purpose:
- expose the compute API through `void-control`

Files:
- `src/bridge.rs`
- `tests/sandbox_api.rs`

Steps:
- [ ] **Step 1: write failing bridge tests**
  - `POST /v1/sandboxes`
  - `GET /v1/sandboxes`
  - `GET /v1/sandboxes/{id}`
  - `POST /v1/sandboxes/{id}/exec`
  - `POST /v1/sandboxes/{id}/stop`
  - `DELETE /v1/sandboxes/{id}`

- [ ] **Step 2: implement bridge request parsing and responses**
  - mirror current execution/template route style
  - return stable JSON resource views

- [ ] **Step 3: add snapshot routes**
  - list/get/create/delete/replicate

- [ ] **Step 4: add pool routes**
  - create, inspect, scale

- [ ] **Step 5: run bridge tests**

```bash
cargo test --features serde --test sandbox_api -- --nocapture
```

- [ ] **Step 6: commit**

```bash
git add src/bridge.rs tests/sandbox_api.rs
git commit -m "bridge: add compute sandbox routes"
```

## Chunk 6: Add CLI And SDK Surfaces

Purpose:
- make the new compute API usable from operators and clients

Files:
- `src/bin/voidctl.rs`
- `tests/voidctl_execution_cli.rs`
- `sdks/python/...`
- `sdks/node/...`
- `sdks/go/...`
- `README.md`
- `AGENTS.md`

Steps:
- [ ] **Step 1: add `voidctl sandbox ...`**
  - `create`
  - `list`
  - `get`
  - `exec`
  - `stop`
  - `delete`

- [ ] **Step 2: add `voidctl snapshot ...`**
  - `create`
  - `list`
  - `get`
  - `replicate`
  - `delete`

- [ ] **Step 3: add SDK clients**
  - Python async-first client
  - Node client
  - Go client

- [ ] **Step 4: add examples**
  - sandbox create + exec
  - snapshot create + restore
  - pool prewarm example once pool routes exist

- [ ] **Step 5: run CLI and SDK tests**

```bash
cargo test --features serde --test voidctl_execution_cli -- --nocapture
python3 -m unittest sdks.python.tests.test_client
node --test sdks/node/test/client.test.mjs
cd sdks/go && GOCACHE=/tmp/go-build go test ./...
```

- [ ] **Step 6: commit**

```bash
git add src/bin/voidctl.rs tests/voidctl_execution_cli.rs sdks README.md AGENTS.md
git commit -m "sdk: add compute sandbox clients"
```

## Chunk 7: Final Verification

- [ ] **Step 1: run formatting**

```bash
cargo fmt --all -- --check
```

- [ ] **Step 2: run linting**

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

- [ ] **Step 3: run tests**

```bash
cargo test
cargo test --features serde
```

- [ ] **Step 4: run docs**

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
```

## Recommendation

Do **not** start Chunk 3 until Chunk 2 is satisfied or intentionally mocked
behind a clearly documented adapter seam. Otherwise `void-control` will grow a
fake compute surface that no real runtime can satisfy.
