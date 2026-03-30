# AGENTS.md — void-control

`void-control` is the control-plane side of the Void stack. It owns runtime
contract normalization, orchestration planning, persistence, bridge APIs, and
the operator UI. It does not implement VM isolation or guest execution; that
belongs to `void-box`.

## System boundary

- `void-control`:
  - normalizes `void-box` daemon responses into a stable contract
  - plans and tracks multi-candidate executions
  - persists execution state, events, candidate records, and message-box data
  - exposes bridge APIs for launch, dry-run, and policy operations
  - provides the graph-first web UI
- `void-box`:
  - launches isolated runtime execution
  - produces run, event, stage, and artifact data
  - enforces sandbox/runtime behavior

When changing code here, preserve that boundary. Control-plane orchestration and
runtime transport concerns should stay separate.

## Repository layout

- `spec/`: canonical specifications and design contracts
- `src/contract/`: runtime contract types, normalization, and compatibility logic
- `src/runtime/`: runtime adapter implementations (`MockRuntime`, `VoidBoxRuntimeClient`)
- `src/orchestration/`: planning, persistence, scheduling, reduction, strategies
- `src/bridge.rs`: HTTP bridge for launch, dry-run, execution inspection, and policy patching
- `src/bin/voidctl.rs`: CLI entrypoint and bridge server
- `tests/`: orchestration, bridge, runtime, and compatibility coverage
- `web/void-control-ux/`: React/Vite operator dashboard
- `docs/`: architecture notes, release process, and internal plans/specs

## Module map

### Rust library

- `src/contract/`
  - contract-facing API and normalization layer
  - converts raw `void-box` payloads into stable `void-control` views
- `src/runtime/`
  - execution runtime abstraction plus mock and live `void-box` client
  - provider launch injection for message-box inbox delivery
- `src/orchestration/spec.rs`
  - execution spec parsing and validation
- `src/orchestration/variation.rs`
  - candidate-generation sources such as `parameter_space`, `explicit`,
    `leader_directed`, and `signal_reactive`
- `src/orchestration/strategy.rs`
  - swarm/search planning and reduction logic
- `src/orchestration/message_box.rs`
  - communication intent routing, inbox snapshots, and `MessageStats` extraction
- `src/orchestration/store/`
  - persisted execution, event, candidate, and message-box data
- `src/orchestration/service.rs`
  - orchestration coordinator; plans, dispatches, reduces, and persists
- `src/orchestration/scheduler.rs`
  - global execution/candidate dispatch ordering
- `src/orchestration/reconcile.rs`
  - restart/reload of persisted active work
- `src/bridge.rs`
  - serde-gated HTTP routes for UI/bridge workflows

### Web UI

- `web/void-control-ux/`
  - graph-first operator dashboard
  - reads daemon and bridge APIs
  - build is the current validation gate for frontend changes

## Core local commands

Rust validation:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo test --features serde
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
```

UI validation:

```bash
cd web/void-control-ux
npm ci
npm run build
```

Bridge and UI local run:

```bash
cargo run --features serde --bin voidctl -- serve
cd web/void-control-ux
npm run dev -- --host 127.0.0.1 --port 3000
```

Canonical live swarm workflow:

```bash
cd /home/diego/github/agent-infra/void-box
TMPDIR=$PWD/target/tmp scripts/build_claude_rootfs.sh
export VOID_BOX_KERNEL=/boot/vmlinuz-$(uname -r)
export VOID_BOX_INITRAMFS=$PWD/target/void-box-rootfs.cpio.gz
export ANTHROPIC_API_KEY=sk-ant-...
cargo run --bin voidbox -- serve --listen 127.0.0.1:43100
```

In a second terminal:

```bash
cd /home/diego/github/void-control
cargo run --features serde --bin voidctl -- serve
```

Submit the swarm execution from a third terminal:

```bash
cd /home/diego/github/void-control
curl -sS -X POST http://127.0.0.1:43210/v1/executions \
  -H 'Content-Type: text/yaml' \
  --data-binary @examples/swarm-transform-optimization.yaml
```

Monitor progress from the bridge:

```bash
curl -sS http://127.0.0.1:43210/v1/executions/<execution_id>
```

Swarm/service template requirements:

- use the production `void-box` initramfs from `scripts/build_claude_rootfs.sh`
- do not use `/tmp/void-box-test-rootfs.cpio.gz` for Claude-backed swarm runs
- swarm runtime templates must set `agent.mode: service`
- `agent.mode: service` requires `agent.output_file`
- `agent.mode: service` must not set `agent.timeout_secs`

Health check:

```bash
curl -sS http://127.0.0.1:43100/v1/health
```

Execution examples:

```bash
curl -sS -X POST http://127.0.0.1:43210/v1/executions \
  -H 'Content-Type: text/yaml' \
  --data-binary @examples/swarm-transform-optimization.yaml

curl -sS -X POST http://127.0.0.1:43210/v1/executions \
  -H 'Content-Type: text/yaml' \
  --data-binary @examples/search-rate-limit-optimization.yaml
```

Important:

- top-level execution specs in `examples/*.yaml` are `void-control` documents
- referenced files under `examples/void-box/*.yaml` are runtime templates for `void-box`
- non-interactive `voidctl` currently exposes `serve` and `help`; use the bridge
  HTTP API or UI for execution create/dry-run flows
- quote URLs that contain `?` when using `curl` from `zsh`

## Runtime compatibility commands

Live daemon contract gate:

```bash
VOID_BOX_BASE_URL=http://127.0.0.1:43100 \
cargo test --features serde --test void_box_contract -- --ignored --nocapture
```

Optional policy fixture overrides:

- `VOID_BOX_TIMEOUT_SPEC_FILE`
- `VOID_BOX_PARALLEL_SPEC_FILE`
- `VOID_BOX_RETRY_SPEC_FILE`
- `VOID_BOX_NO_POLICY_SPEC_FILE`

## UI workflow expectations

For UI work in `web/void-control-ux`, use browser automation/inspection for DOM,
layout, resize, console, and network validation. Screenshots are fallback only.

Preferred order:

- configured browser MCP
- local Playwright if browser MCP is unavailable
- screenshots only when interactive inspection is impossible

## Documentation expectations

- add new specs under `spec/` with versioned filenames
- keep implementation-facing architecture notes in `docs/`
- update `README.md`, `AGENTS.md`, or `docs/architecture.md` when behavior or
  workflows change materially

## Testing expectations

- keep unit/contract tests close to the relevant Rust logic where practical
- use integration tests in `tests/` for orchestration, bridge, and acceptance flows
- before merging Rust changes, run:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test`
  - `cargo test --features serde`
- before merging UI changes, also run:
  - `npm run build` in `web/void-control-ux`

## Pre-commit

This repo uses a checked-in `.pre-commit-config.yaml` for local validation.

Typical setup:

```bash
pip install pre-commit
pre-commit install
pre-commit run --all-files
```

## Commit and PR guidance

- commit format: `area: concise action`
- keep commits scoped to one concern
- PRs should include:
  - problem statement
  - summary of changes
  - affected specs/docs
  - verification commands run
  - follow-up work, if intentionally deferred
