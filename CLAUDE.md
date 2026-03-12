# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

void-control is the control-plane orchestration layer for the `void-box` runtime. It launches/manages runs, tracks run/stage/event lifecycle, and enforces runtime contract compatibility. The project has two main parts: a Rust library/CLI and a React operator dashboard.

## Build & Test Commands

```bash
# Core unit tests (no serde feature)
cargo test

# JSON compatibility + fixture-based tests
cargo test --features serde

# Mocked transport contract tests for void-box client
cargo test --features serde runtime::void_box::

# Live daemon contract tests (requires running void-box on port 43100)
VOID_BOX_BASE_URL=http://127.0.0.1:43100 \
  cargo test --features serde --test void_box_contract -- --ignored --nocapture

# Run a single test
cargo test --features serde test_name_here

# Terminal console
cargo run --features serde --bin voidctl

# Bridge server (port 43210)
cargo run --features serde --bin voidctl -- serve
```

**Always validate both paths before PRs:** `cargo test` AND `cargo test --features serde`.

### Web UI (web/void-control-ux/)

```bash
cd web/void-control-ux
npm install
VITE_VOID_BOX_BASE_URL=http://127.0.0.1:43100 npm run dev   # dev server on port 5174
npm run build                                                 # production build (tsc -b && vite build)
```

## Architecture

### Rust Crate (src/)

Single crate with two main modules, feature-gated with `serde`:

- **`contract/`** — Control-plane type definitions (no runtime dependencies)
  - `api.rs` — Request/response types: `StartRequest`, `StopRequest`, `RuntimeInspection`, `ConvertedRunView`
  - `state.rs` — `RunState` enum with strict lifecycle transitions: Pending→Starting→Running→{Succeeded|Failed|Canceled}
  - `event.rs` — `EventEnvelope`, `EventType` enum, `EventSequenceTracker` (enforces monotonic seq ordering)
  - `policy.rs` — `ExecutionPolicy` (max_parallel_microvms, stage_timeout, retry config)
  - `error.rs` — `ContractError` with code, message, retryable flag
  - `compat.rs` / `compat_json.rs` — Normalization from void-box raw format to canonical types

- **`runtime/`** — Client implementations (behind `serde` feature)
  - `void_box.rs` — `VoidBoxRuntimeClient` (HTTP client to void-box daemon)
  - `mock.rs` — `MockRuntime` for testing

- **`bin/`** — `voidctl` (console + bridge server), `normalize_fixture`

### Web UI (web/void-control-ux/src/)

React 18 + TypeScript dashboard:

- **State:** Zustand (`store/ui.ts`) for selection state; TanStack Query for server state with tiered polling (active runs 2.5s, terminal 5s, events 1.2s)
- **Key components:** `RunsList`, `RunGraph` (Sigma/Graphology DAG), `EventTimeline`, `NodeInspector`, `LaunchRunModal`
- **API layer:** `lib/api.ts` wraps daemon endpoints (`/v1/runs/*`) and bridge endpoint (`/v1/launch`)
- **Types:** `lib/types.ts` mirrors the Rust contract types

### API Surface

- Daemon (void-box): `/v1/runs`, `/v1/runs/{id}`, `/v1/runs/{id}/events`, `/v1/runs/{id}/stages`, `/v1/runs/{id}/telemetry`, `/v1/runs/{id}/cancel`
- Bridge (voidctl serve): `POST /v1/launch`

## Coding Conventions

- **Naming:** Use boundary-focused names from the spec: `Run`, `Stage`, `Attempt`, `Runtime`, `Controller`
- **Testing:** Keep contract tests in `#[cfg(test)]` blocks near conversion/runtime logic. Fixture-based tests require `--features serde`
- **Feature gating:** JSON serialization, HTTP client, and server code live behind the `serde` feature flag
- **Commits:** Imperative style, format `area: concise action` (e.g., `spec: clarify cancellation semantics`)
- **Specs:** Add new specs to `spec/` with version in filename (e.g., `*-v0.2.md`)

## Environment Variables

- `VOID_BOX_BASE_URL` — void-box daemon endpoint (default: `http://127.0.0.1:43100`)
- `VITE_VOID_BOX_BASE_URL` — daemon URL for web UI
- `VITE_VOID_CONTROL_BASE_URL` — bridge URL for web UI (e.g., `http://127.0.0.1:43210`)
- `VOID_BOX_TIMEOUT_SPEC_FILE`, `VOID_BOX_PARALLEL_SPEC_FILE`, `VOID_BOX_RETRY_SPEC_FILE`, `VOID_BOX_NO_POLICY_SPEC_FILE` — Optional spec file overrides for policy behavior tests
