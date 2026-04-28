# Compute Sandbox API Draft

## Date

2026-04-21

## Status

Draft

## Problem

`void-control` is currently strong as an orchestration and execution-tracking
API, but weak as a direct compute API.

That leaves a product gap against systems like:

- E2B
- BoxLite / BoxRun
- ComputeSDK providers

Today, `void-control` can:

- create executions from raw specs
- compile and execute checked-in templates
- inspect execution progress, results, and events

Today, it cannot expose a first-class compute surface such as:

- create a reusable sandbox
- execute a command in that sandbox
- execute code in that sandbox
- manage snapshots explicitly
- replicate snapshots to multiple nodes
- maintain prewarmed sandbox pools

## Current Code Reality

The current `void-control` runtime boundary is still execution-centric.

- `ExecutionRuntime` only exposes one-shot run lifecycle methods:
  `start_run`, `inspect_run`, and `take_structured_output`
- `VoidBoxRuntimeClient` talks to the `void-box` daemon as a run client over
  `/v1/runs`
- the bridge currently wraps:
  - executions
  - templates
  - batch/yolo
  - teams

The current `void-box` daemon surface does not yet expose a general sandbox
management API equivalent to E2B or BoxRun.

What exists today:

- `/v1/runs`
- `/v1/runs/{id}/...`
- `/v1/sessions/{id}/messages`
- run submission already accepts snapshot restore input via run creation

What is not exposed today as daemon routes:

- `POST /v1/sandboxes`
- `POST /v1/sandboxes/{id}/exec`
- `POST /v1/snapshots`
- `GET /v1/snapshots`
- `POST /v1/snapshots/{id}/replicate`

The concrete daemon contract expected by this branch is captured in:

- `docs/superpowers/specs/2026-04-27-void-box-compute-daemon-prerequisites.md`

So this design must be implemented in phases:

1. align the control-plane object model and API shape in `void-control`
2. add the missing daemon/runtime primitives in `void-box`
3. wire the bridge and SDKs only after the runtime support exists

## Design Principle

Use a dedicated compute-oriented `SandboxSpec` as the base primitive.

This must stay separate from the orchestration-oriented `ExecutionSpec`.

The distinction is:

- `SandboxSpec`
  - defines the runtime environment and lifecycle
- `ExecutionSpec`
  - defines orchestration, evaluation, variation, and reduction
- templates
  - compile user-facing inputs into either of the above

This separation is necessary to support both:

- direct compute APIs similar to BoxRun / E2B
- higher-level orchestration and swarm APIs

## Architectural Boundary

### `void-box`

Owns:

- sandbox lifecycle
- runtime creation
- command/code execution
- filesystem operations
- snapshot create/restore/delete primitives
- node-local snapshot storage and runtime restore
- low-level resource and isolation behavior

### `void-control`

Owns:

- `SandboxSpec`
- sandbox metadata and persistence
- pool and lease management
- prewarm policy
- snapshot inventory and replication planning
- node placement policy
- ComputeSDK-style or BoxRun-style management APIs
- orchestration over sandbox-backed runs when needed

## Primary Objects

### `SandboxSpec`

Defines one reusable or ephemeral compute environment.

Suggested shape:

```yaml
api_version: v1
kind: sandbox

metadata:
  name: python-benchmark-box
  labels:
    workload: benchmark
    language: python

runtime:
  image: python:3.12-slim
  cpus: 2
  memory_mb: 2048
  network: true
  env:
    FOO: bar
  mounts:
    - host: /data/fixtures
      guest: /workspace/fixtures
      mode: ro
  ports:
    - 3000
    - 8080

snapshot:
  restore_from: snapshot-transform-v1

lifecycle:
  auto_remove: false
  detach: true
  idle_timeout_secs: 900
  prewarm: true

identity:
  reusable: true
  pool: benchmark-python
```

### `SnapshotSpec`

Defines snapshot metadata plus replication intent.

Suggested shape:

```yaml
api_version: v1
kind: snapshot

metadata:
  name: snapshot-transform-v1
  labels:
    workload: benchmark

source:
  sandbox_id: sbx-123

distribution:
  mode: cached
  targets:
    - node-a
    - node-b
    - node-c
```

### `SandboxPoolSpec`

Defines prewarmed capacity for a common sandbox shape.

Suggested shape:

```yaml
api_version: v1
kind: sandbox_pool

metadata:
  name: benchmark-python-pool

sandbox_spec:
  runtime:
    image: python:3.12-slim
    cpus: 2
    memory_mb: 2048
  snapshot:
    restore_from: snapshot-transform-v1
  lifecycle:
    auto_remove: false
    detach: true
    idle_timeout_secs: 900
    prewarm: true
  identity:
    reusable: true
    pool: benchmark-python

capacity:
  warm: 5
  max: 20
```

## Why This Enables Prewarm

Prewarm is not a separate primitive. It is pool management over `SandboxSpec`.

Flow:

1. define a sandbox shape
2. optionally restore from a snapshot
3. keep `N` instances already started
4. lease one for execution
5. return, recycle, or destroy it according to policy

This is a better fit than forcing prewarm to live inside orchestration specs.

## Why This Enables ComputeSDK Compatibility

ComputeSDK-style flows are sandbox-action flows.

They map naturally to this model:

- `compute.sandbox.create`
  - create one sandbox from `SandboxSpec`
- `compute.sandbox.runCommand`
  - execute a command inside an existing sandbox
- `compute.sandbox.runCode`
  - execute code inside an existing sandbox
- filesystem actions
  - read/write/list/remove inside an existing sandbox
- `compute.sandbox.destroy`
  - stop and remove one sandbox

That compatibility layer should be built on top of the compute API, not on top
of orchestration templates.

## Snapshot Model

Snapshots must be first-class.

Required operations:

- create snapshot from a sandbox
- inspect snapshot metadata
- list snapshots
- delete snapshots
- restore a sandbox from a snapshot
- replicate a snapshot to multiple nodes

Important distinction:

- `restore_from`
  - boot a sandbox from an existing snapshot
- `replicate`
  - distribute snapshot data to target nodes
- `prewarm`
  - keep already-restored sandboxes warm and ready

These are related but different lifecycle operations.

## Multi-Node Snapshot Replication

Snapshot replication should be modeled as control-plane policy plus runtime
execution.

Recommended responsibilities:

- `void-control`
  - decides target nodes
  - tracks replication state
  - exposes replication APIs and status
- `void-box`
  - performs the actual node-local snapshot import/export and restore

Suggested states:

- `Pending`
- `Copying`
- `Ready`
- `Failed`

Suggested replication modes:

- `copy`
  - eagerly copy to all target nodes
- `cached`
  - copy on demand, then retain locally
- `lazy`
  - register targets but do not pre-copy

## Proposed HTTP API

### Sandbox lifecycle

- `POST /v1/sandboxes`
- `GET /v1/sandboxes`
- `GET /v1/sandboxes/{id}`
- `POST /v1/sandboxes/{id}/exec`
- `POST /v1/sandboxes/{id}/stop`
- `DELETE /v1/sandboxes/{id}`

Create request:

```json
{
  "spec": {
    "metadata": { "name": "python-benchmark-box" },
    "runtime": {
      "image": "python:3.12-slim",
      "cpus": 2,
      "memory_mb": 2048,
      "network": true
    },
    "snapshot": {
      "restore_from": "snapshot-transform-v1"
    },
    "lifecycle": {
      "auto_remove": false,
      "detach": true,
      "idle_timeout_secs": 900,
      "prewarm": false
    },
    "identity": {
      "reusable": true
    }
  }
}
```

Exec request:

```json
{
  "kind": "command",
  "command": ["python3", "-c", "print('hello')"]
}
```

Later extension:

```json
{
  "kind": "code",
  "runtime": "python",
  "code": "print('hello')"
}
```

### Snapshot lifecycle

- `POST /v1/snapshots`
- `GET /v1/snapshots`
- `GET /v1/snapshots/{id}`
- `POST /v1/snapshots/{id}/replicate`
- `DELETE /v1/snapshots/{id}`

Create request:

```json
{
  "source_sandbox_id": "sbx-123",
  "name": "snapshot-transform-v1"
}
```

Replicate request:

```json
{
  "targets": ["node-a", "node-b", "node-c"],
  "mode": "copy"
}
```

### Pool lifecycle

- `POST /v1/pools`
- `GET /v1/pools`
- `GET /v1/pools/{id}`
- `POST /v1/pools/{id}/scale`
- `POST /v1/pools/{id}/lease`
- `POST /v1/pools/{id}/release`

## Suggested Response Shape

Sandbox response:

```json
{
  "sandbox_id": "sbx-123",
  "status": "Running",
  "node_id": "node-a",
  "spec": { "...": "..." }
}
```

Snapshot response:

```json
{
  "snapshot_id": "snapshot-transform-v1",
  "status": "Ready",
  "source_sandbox_id": "sbx-123",
  "replication": {
    "mode": "copy",
    "targets": [
      { "node_id": "node-a", "status": "Ready" },
      { "node_id": "node-b", "status": "Ready" },
      { "node_id": "node-c", "status": "Copying" }
    ]
  }
}
```

Pool response:

```json
{
  "pool_id": "pool-benchmark-python",
  "warm": 5,
  "leased": 2,
  "available": 3,
  "max": 20
}
```

## Recommended Rollout

### Phase 1

- define `SandboxSpec`
- add sandbox create/get/list/stop/delete
- add sandbox `exec` for commands
- add snapshot create/list/get/delete
- add snapshot restore on sandbox creation

### Phase 2

- add snapshot replication status and control
- add pool create/get/scale
- add prewarm and lease/release

### Phase 3

- add code helpers such as `runCode`
- add filesystem APIs
- add ComputeSDK-style compatibility routes

## Recommendation

The next implementation work should start with `SandboxSpec` and the sandbox
lifecycle routes.

That creates a base compute API that can later support:

- prewarmed pools
- multi-node snapshot restore
- ComputeSDK compatibility
- BoxRun-style management APIs

without overloading the current orchestration spec model.
