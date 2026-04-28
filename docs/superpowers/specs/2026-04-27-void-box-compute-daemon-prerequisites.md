# `void-box` Compute Daemon Prerequisites

## Date

2026-04-27

## Status

Draft

## Purpose

`feat/compute-sandbox-api` in `void-control` now defines a control-plane
contract for:

- sandboxes
- snapshots
- pools
- compute-oriented CLI and SDK surfaces

The control-plane side is intentionally ahead of the live runtime integration.
This document defines the `void-box` daemon changes required before
`VoidBoxRuntimeClient` can stop returning unsupported errors for the compute
surface.

## Important Boundary

### Must be implemented in `void-box`

- sandbox lifecycle primitives
- exec inside an existing sandbox
- snapshot lifecycle primitives
- node-local snapshot restore support
- replication primitive or an equivalent lower-level distribution primitive

### Must remain in `void-control`

- pool definitions
- warm-capacity targets
- pool scaling policy
- lease/reuse policy
- global placement decisions
- orchestration over multiple sandboxes

`pool` is not a `void-box` abstraction.

## Current `void-control` Assumption

The compute branch currently assumes the daemon can eventually expose:

- `POST /v1/sandboxes`
- `GET /v1/sandboxes`
- `GET /v1/sandboxes/{id}`
- `POST /v1/sandboxes/{id}/exec`
- `POST /v1/sandboxes/{id}/stop`
- `DELETE /v1/sandboxes/{id}`
- `POST /v1/snapshots`
- `GET /v1/snapshots`
- `GET /v1/snapshots/{id}`
- `POST /v1/snapshots/{id}/replicate`
- `DELETE /v1/snapshots/{id}`

The current `VoidBoxRuntimeClient` still returns:

- `sandbox api is not supported by the current void-box daemon`

for sandbox lifecycle calls because those routes do not exist live yet.

## Required Daemon Routes

## 1. Sandbox Lifecycle

### `POST /v1/sandboxes`

Purpose:
- create one sandbox from a `SandboxSpec`

Request body:

```json
{
  "api_version": "v1",
  "kind": "sandbox",
  "metadata": {
    "name": "python-benchmark-box"
  },
  "runtime": {
    "image": "python:3.12-slim",
    "cpus": 2,
    "memory_mb": 2048
  },
  "snapshot": {
    "restore_from": "snapshot-transform-v1"
  }
}
```

Minimum response:

```json
{
  "kind": "sandbox",
  "sandbox": {
    "sandbox_id": "sbx-123",
    "state": "running",
    "restore_from_snapshot": "snapshot-transform-v1"
  }
}
```

Required semantics:
- daemon assigns a stable `sandbox_id`
- if `snapshot.restore_from` is present, response must report the restored
  snapshot id in `restore_from_snapshot`
- initial state should be `running` after successful creation

### `GET /v1/sandboxes`

Purpose:
- list sandboxes known to the daemon

Minimum response:

```json
{
  "kind": "sandbox_list",
  "sandboxes": [
    {
      "sandbox_id": "sbx-123",
      "state": "running",
      "restore_from_snapshot": "snapshot-transform-v1"
    }
  ]
}
```

### `GET /v1/sandboxes/{id}`

Purpose:
- inspect one sandbox

Minimum success response:

```json
{
  "kind": "sandbox",
  "sandbox": {
    "sandbox_id": "sbx-123",
    "state": "running",
    "restore_from_snapshot": "snapshot-transform-v1"
  }
}
```

Minimum failure response:

```json
{
  "message": "sandbox 'sbx-123' not found"
}
```

Recommended status:
- `404` when missing

### `POST /v1/sandboxes/{id}/exec`

Purpose:
- execute work inside an existing sandbox

Minimum request forms:

Command execution:

```json
{
  "kind": "command",
  "command": ["python3", "-V"]
}
```

Code execution:

```json
{
  "kind": "code",
  "runtime": "python",
  "code": "print('hello')"
}
```

Minimum response:

```json
{
  "kind": "sandbox_exec",
  "result": {
    "exit_code": 0,
    "stdout": "python3 -V",
    "stderr": ""
  }
}
```

Required semantics:
- `kind=command` must execute the provided argv inside the sandbox
- `kind=code` must execute code using the requested runtime if supported
- missing sandbox should return `404`
- stopped sandbox should return a non-success error with a clear message

### `POST /v1/sandboxes/{id}/stop`

Purpose:
- stop a running sandbox without deleting its identity immediately

Minimum success response:

```json
{
  "kind": "sandbox",
  "sandbox": {
    "sandbox_id": "sbx-123",
    "state": "stopped",
    "restore_from_snapshot": "snapshot-transform-v1"
  }
}
```

### `DELETE /v1/sandboxes/{id}`

Purpose:
- delete a sandbox identity and its live runtime instance

Minimum success response:

```json
{
  "kind": "sandbox_deleted",
  "sandbox_id": "sbx-123"
}
```

Recommended status:
- `404` when missing

## 2. Snapshot Lifecycle

### `POST /v1/snapshots`

Purpose:
- create a snapshot from an existing sandbox

Request body:

```json
{
  "api_version": "v1",
  "kind": "snapshot",
  "metadata": {
    "name": "snapshot-transform-v1"
  },
  "source": {
    "sandbox_id": "sbx-123"
  },
  "distribution": {
    "mode": "cached",
    "targets": ["node-a"]
  }
}
```

Minimum response:

```json
{
  "kind": "snapshot",
  "snapshot": {
    "snapshot_id": "snap-123",
    "source_sandbox_id": "sbx-123",
    "distribution": {
      "mode": "cached",
      "targets": ["node-a"]
    }
  }
}
```

Required semantics:
- snapshot creation must fail clearly if the source sandbox does not exist
- the daemon may ignore `distribution` during creation if replication is a
  separate step, but it must return a normalized snapshot record

### `GET /v1/snapshots`

Minimum response:

```json
{
  "kind": "snapshot_list",
  "snapshots": [
    {
      "snapshot_id": "snap-123",
      "source_sandbox_id": "sbx-123",
      "distribution": {
        "mode": "cached",
        "targets": ["node-a"]
      }
    }
  ]
}
```

### `GET /v1/snapshots/{id}`

Minimum response:

```json
{
  "kind": "snapshot",
  "snapshot": {
    "snapshot_id": "snap-123",
    "source_sandbox_id": "sbx-123",
    "distribution": {
      "mode": "cached",
      "targets": ["node-a"]
    }
  }
}
```

### `POST /v1/snapshots/{id}/replicate`

Purpose:
- copy or stage a snapshot to multiple nodes

Request body:

```json
{
  "mode": "copy",
  "targets": ["node-a", "node-b", "node-c"]
}
```

Minimum response:

```json
{
  "kind": "snapshot",
  "snapshot": {
    "snapshot_id": "snap-123",
    "source_sandbox_id": "sbx-123",
    "distribution": {
      "mode": "copy",
      "targets": ["node-a", "node-b", "node-c"]
    }
  }
}
```

Required semantics:
- this route may be backed by a more primitive implementation internally
- if `void-box` prefers a lower-level replication operation, `void-control`
  can adapt later, but the daemon still needs some public primitive that:
  - accepts a snapshot id
  - accepts target nodes
  - returns resulting distribution state

### `DELETE /v1/snapshots/{id}`

Minimum success response:

```json
{
  "kind": "snapshot_deleted",
  "snapshot_id": "snap-123"
}
```

## Required Enum/State Compatibility

For the current `void-control` branch, daemon responses should match these
simple states:

- sandbox `state`:
  - `running`
  - `stopped`

For snapshot distribution, current parser/test assumptions are:

- `mode`:
  - `cached`
  - `copy`

If `void-box` wants richer internal state, that is fine, but the public daemon
surface should preserve these minimum values or `void-control` will need an
adapter layer.

## What Does Not Need To Be In Scope Yet

The following are intentionally out of scope for the first daemon slice:

- filesystem routes
- PTY / terminal routes
- port exposure routes
- global lease management
- pool lifecycle
- scheduler-aware placement
- cross-node pool balancing

Those can land later after the basic sandbox and snapshot primitives are real.

## Minimum Integration Goal

`void-control` can replace the current unsupported `VoidBoxRuntimeClient`
compute paths once the daemon can satisfy this round trip:

1. `POST /v1/sandboxes`
2. `GET /v1/sandboxes`
3. `GET /v1/sandboxes/{id}`
4. `POST /v1/sandboxes/{id}/exec`
5. `POST /v1/sandboxes/{id}/stop`
6. `DELETE /v1/sandboxes/{id}`
7. `POST /v1/snapshots`
8. `GET /v1/snapshots`
9. `GET /v1/snapshots/{id}`
10. `POST /v1/snapshots/{id}/replicate`
11. `DELETE /v1/snapshots/{id}`

That is the minimum `void-box` daemon contract needed for the current
`feat/compute-sandbox-api` branch to become live instead of mock-backed.
