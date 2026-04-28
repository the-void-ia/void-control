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

## Async runtime

`voidctl serve` runs on a multi-threaded tokio runtime (plain
`#[tokio::main]` in `src/bin/voidctl.rs`) — the conventional default for
HTTP services in Rust. The bridge HTTP server (`axum`) and the worker tick
(`process_pending_executions_once`) both run as `tokio::spawn` tasks on
that runtime.

All async traits in the orchestration and runtime layers
(`ExecutionRuntime`, `MessageDeliveryAdapter`, `HttpTransport`,
`ProviderLaunchAdapter`) are bounded `Send + Sync`. Trait objects
(`Box<dyn ProviderLaunchAdapter>`, etc.) are `Send + Sync` by way of the
trait's supertrait. Test mocks use `Arc<Mutex<…>>` for shared recorders.

The trait surface also supports `current_thread` via
`#[tokio::main(flavor = "current_thread")]` for any future workload that
prefers it.

## Repository layout

- `spec/`: canonical specifications and design contracts
- `src/contract/`: runtime contract types, normalization, and compatibility logic
- `src/runtime/`: runtime adapter implementations (`MockRuntime`, `VoidBoxRuntimeClient`)
- `src/orchestration/`: planning, persistence, scheduling, reduction, strategies
- `src/bridge.rs`: HTTP bridge for launch, dry-run, execution inspection, and policy patching
- `src/templates/`: file-backed template schema, loading, and compilation into `ExecutionSpec`
- `src/bin/voidctl.rs`: CLI entrypoint and bridge server
- `tests/`: orchestration, bridge, runtime, and compatibility coverage
- `web/void-control-ux/`: React/Vite operator dashboard
- `docs/`: architecture notes, release process, and internal plans/specs
- `templates/`: checked-in template-first API definitions

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
  - swarm planning and reduction logic; supervision strategy work lands here
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
  - execution routes plus template-first bridge routes
- `src/templates/`
  - phase-1 control template schema, checked-in loader, and compile logic

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
cargo run --bin voidbox -- serve
```

The daemon defaults to AF_UNIX at mode `0o600` (path-discovery chain:
`$XDG_RUNTIME_DIR/voidbox.sock` → `$TMPDIR/voidbox-$UID.sock` →
`/tmp/voidbox-$UID.sock`). Same-uid `void-control` finds it with no
configuration. To listen on TCP instead, pass
`--listen tcp://127.0.0.1:43100`; TCP requires a bearer token resolved
from `VOIDBOX_DAEMON_TOKEN_FILE`, `VOIDBOX_DAEMON_TOKEN`, or
`$XDG_CONFIG_HOME/voidbox/daemon-token`.

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
  --data-binary @examples/swarm-transform-optimization-3way.yaml
```

Use `examples/swarm-transform-optimization-3way.yaml` as the default live
validation path. It is the more reliable three-candidate version of the
Transform-02 swarm example. Keep `examples/swarm-transform-optimization.yaml`
as the wider eight-candidate stress case.

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

Health check (AF_UNIX default; pass `--unix-socket` to curl):

```bash
curl -sS --unix-socket "$XDG_RUNTIME_DIR/voidbox.sock" http://localhost/v1/health
```

When the daemon listens on TCP:

```bash
curl -sS http://127.0.0.1:43100/v1/health \
  -H "Authorization: Bearer $(cat "$XDG_CONFIG_HOME/voidbox/daemon-token")"
```

Execution examples:

```bash
curl -sS -X POST http://127.0.0.1:43210/v1/executions \
  -H 'Content-Type: text/yaml' \
  --data-binary @examples/swarm-transform-optimization-3way.yaml

curl -sS -X POST http://127.0.0.1:43210/v1/executions \
  -H 'Content-Type: text/yaml' \
  --data-binary @examples/swarm-transform-optimization.yaml
```

Important:

- top-level execution specs in `examples/*.yaml` are `void-control` documents
- referenced files under `examples/runtime-templates/*.yaml` are runtime templates for `void-box`
- non-interactive `voidctl` exposes:
  - `serve`
  - `execution submit <spec-path>`
  - `execution submit --stdin`
  - `execution dry-run <spec-path>`
  - `execution dry-run --stdin`
  - `execution watch <execution-id>`
  - `execution inspect <execution-id>`
  - `execution events <execution-id>`
  - `execution result <execution-id>`
  - `execution runtime <execution-id> [candidate-id]`
  - `template list`
  - `template get <template-id>`
  - `template dry-run <template-id> [<inputs-json-path> | --stdin]`
  - `template execute <template-id> [<inputs-json-path> | --stdin]`
  - `batch dry-run <spec-path>`
  - `batch dry-run --stdin`
  - `batch run <spec-path>`
  - `batch run --stdin`
  - `yolo dry-run <spec-path>`
  - `yolo dry-run --stdin`
  - `yolo run <spec-path>`
  - `yolo run --stdin`
  - `team dry-run <spec-path>`
  - `team dry-run --stdin`
  - `team run <spec-path>`
  - `team run --stdin`
- interactive `voidctl` console also exposes:
  - `/template list`
  - `/template get <template-id>`
  - `/template dry-run <template-id> <inputs-json-path>`
  - `/template execute <template-id> <inputs-json-path>`
  - `/batch dry-run <spec-path>`
  - `/batch run <spec-path>`
  - `/yolo dry-run <spec-path>`
  - `/yolo run <spec-path>`
  - `/team dry-run <spec-path>`
  - `/team run <spec-path>`
- `batch` is the canonical high-level remote background execution surface
- `yolo` is an accepted alias for `batch`
- `team` is the phase-1 high-level multi-agent authoring surface
- current phase-1 `team` limitations:
  - `depends_on` is not supported yet
  - `sequential` preserves ordering only; task outputs are not threaded between agents
- use `voidctl execution ...` for terminal operator workflows; use the bridge
  HTTP API or UI when you need direct API-driven inspection or browser workflows
- quote URLs that contain `?` when using `curl` from `zsh`
- template-first bridge endpoints:
  - `GET /v1/templates`
  - `GET /v1/templates/{id}`
  - `POST /v1/templates/{id}/dry-run`
  - `POST /v1/templates/{id}/execute`
- batch bridge endpoints:
  - `POST /v1/batch/dry-run`
  - `POST /v1/batch/run`
  - `GET /v1/batch-runs/{id}`
- accepted aliases:
  - `POST /v1/yolo/dry-run`
  - `POST /v1/yolo/run`
  - `GET /v1/yolo-runs/{id}`
- team bridge endpoints:
  - `POST /v1/teams/dry-run`
  - `POST /v1/teams/run`
  - `GET /v1/team-runs/{id}`

## Runtime compatibility commands

Live daemon contract gate. The contract test dials the daemon directly,
so `VOID_BOX_BASE_URL` must be set; both shapes are accepted.

```bash
# AF_UNIX (default daemon listener)
VOID_BOX_BASE_URL=unix://$XDG_RUNTIME_DIR/voidbox.sock \
cargo test --features serde --test void_box_contract -- --ignored --nocapture

# TCP
VOID_BOX_BASE_URL=http://127.0.0.1:43100 \
cargo test --features serde --test void_box_contract -- --ignored --nocapture
```

## Environment variables

Control-plane / bridge:

- `VOID_BOX_BASE_URL` — void-box daemon endpoint. Default: auto-discover
  the AF_UNIX socket the daemon advertises (`$XDG_RUNTIME_DIR/voidbox.sock`
  → `$TMPDIR/voidbox-$UID.sock` → `/tmp/voidbox-$UID.sock`). Override
  with `unix:///abs/path` for an explicit AF_UNIX path or
  `http://host:port` to talk to a TCP-listening daemon. TCP requires a
  bearer token via `VOIDBOX_DAEMON_TOKEN_FILE`, `VOIDBOX_DAEMON_TOKEN`,
  or `$XDG_CONFIG_HOME/voidbox/daemon-token`; construction fails closed
  if none resolves.
- `VOIDBOX_DAEMON_TOKEN_FILE` / `VOIDBOX_DAEMON_TOKEN` — bearer-token
  sources for the TCP transport (mirrors void-box's resolution chain).
  Token files must be owner-only (`mode & 0o077 == 0`).
- `VOID_CONTROL_BRIDGE_LISTEN` — bridge listen address (default:
  `127.0.0.1:43210`). The Vite dev server proxies `/api` here.
- `VOID_CONTROL_LLM_PROVIDER` — optional global override that patches
  `llm.provider` on every runtime template at launch. Set to
  `claude-personal` to use OAuth from the macOS Keychain or
  `~/.claude/.credentials.json` without editing tracked templates.
  Per-candidate `variation.explicit[].overrides` still win.

Web UI:

- `VITE_VOID_BOX_BASE_URL` — daemon URL for the operator dashboard.
  Leave unset during local dev so the Vite `/api` proxy is used;
  void-box serves no CORS headers, so setting this sends the browser
  straight into the CORS pit.
- `VITE_VOID_CONTROL_BASE_URL` — bridge URL for the operator dashboard
  (e.g., `http://127.0.0.1:43210`). The bridge sets CORS, so this can
  point at a direct origin.

Optional policy fixture overrides (used by contract tests):

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
