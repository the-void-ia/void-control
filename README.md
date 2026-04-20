# void-control

Control-plane orchestration for `void-box` runtime execution.

![void-control hero](docs/assets/void-control-hero.png)

## Orchestration Strategies

`void-control` should be understood as a host for orchestration strategies, not
as a single-purpose swarm console.

Current direction:

- `swarm`: first implemented orchestration strategy
- `supervision`: implemented orchestrator-worker strategy

Shared control-plane primitives across strategies:

- execution specs and policies
- candidate planning and reduction
- persisted control-plane events
- message-box / MCP-backed collaboration state
- graph-first execution inspection in the UI

The strategy changes the orchestration semantics. It should not require a
different product surface or a different backend contract family.

## Demo

[![void-control demo](docs/assets/void-control-demo.gif)](docs/assets/void-control-demo.mp4)

Click the preview above for the full-quality MP4, or use the direct file link: [void-control demo video](docs/assets/void-control-demo.mp4).

## Swarm Execution Demo

[![void-control swarm demo](docs/assets/void-control-swarm-demo.gif)](docs/assets/void-control-swarm-demo.mp4)

This recording shows the canonical first-release flow:

- a live 3-agent swarm execution
- graph-first orchestration inspection
- right-side metrics and event inspection
- runtime drill-down through `Open Runtime Graph`

Direct link: [void-control swarm execution demo](docs/assets/void-control-swarm-demo.mp4).

What this example does:

- runs three sibling optimization strategies against the same Transform-02 workload
- compares candidates by measured metrics, not invented estimates
- uses swarm reduction to select the best candidate for the iteration

What to look for:

- candidate fan-out in the graph
- metrics and event inspection on the right
- winner selection and runtime drill-down through `Open Runtime Graph`

## Supervision Execution Demo

[![void-control supervision demo](docs/assets/void-control-supervision-demo.gif)](docs/assets/void-control-supervision-demo.webm)

This recording shows the supervision operator flow in the real UI:

- `Launch Spec` with the checked-in supervision example
- supervision execution selection in the left rail
- supervision graph in the center pane
- supervision-specific inspector state on the right
- runtime drill-down through `Open Runtime Graph`

Direct link: [void-control supervision execution demo](docs/assets/void-control-supervision-demo.webm).

What this example does:

- runs three specialized Transform-02 workers under one supervisor
- collects each worker output and evaluates `metrics.approved`
- finalizes only after the workers are reviewed and approved

What to look for:

- supervisor-to-worker graph semantics instead of swarm fan-out/ranking
- review and approval state in the right inspector
- finalization flow and runtime drill-down through `Open Runtime Graph`

## Release

- Current release target: `v0.0.2`
- Release artifacts are published through GitHub Releases
- Supported `void-box` baseline for `v0.0.2`: `void-box` `v0.1.2` or an equivalent validated production build
- Release process and compatibility gate details: [docs/release-process.md](docs/release-process.md)

## What It Is

`void-control` is the control-plane side of the stack:

- launches and manages runtime work on `void-box`
- normalizes runtime payloads into a stable control-plane contract
- plans and tracks orchestration executions across multiple candidates
- persists execution, event, candidate, and message-box state
- provides terminal-first and graph-first operator UX
- enforces runtime contract compatibility with `void-box`

## Documentation

- Architecture: [docs/architecture.md](docs/architecture.md)
- Contributor and agent guide: [AGENTS.md](AGENTS.md)
- Release and compatibility process: [docs/release-process.md](docs/release-process.md)
- Execution examples and live swarm workflow: [examples/README.md](examples/README.md)

## Project Components

- `spec/`: Runtime and orchestration contracts.
- `src/`: Rust orchestration client/runtime normalization logic.
- `templates/`: File-backed template-first API definitions for single-agent and warm-agent execution.
- `tests/`: Contract and compatibility tests.
- `web/void-control-ux/`: React operator dashboard (graph + inspector).

## Quick Start

### 1) Start `void-box` daemon

Linux:

```bash
cargo run --bin voidbox -- serve --listen 127.0.0.1:43100
```

macOS (Apple Silicon, Virtualization.framework):

```bash
# after building the guest kernel + rootfs per void-box's macOS guide
VOID_BOX_KERNEL=target/vmlinuz \
VOID_BOX_INITRAMFS=target/void-box-claude.cpio.gz \
  cargo run --bin voidbox -- serve --listen 127.0.0.1:43100
```

macOS requires `VOID_BOX_KERNEL` and `VOID_BOX_INITRAMFS` pointing at the
pre-built guest artifacts. The initramfs filename on macOS is
`target/void-box-claude.cpio.gz` (not the Linux `void-box-rootfs.cpio.gz`).
Running through `cargo run` also applies `codesign` automatically — direct
binary invocation needs manual codesigning. See the `void-box` macOS guide
for full details.

### 2) Run `void-control` tests

```bash
cargo test --features serde
```

All test targets are currently gated behind the `serde` feature, so
`cargo test` without it runs zero tests. Use `--features serde` as the
one validation path.

### 3) Run live daemon contract gate

```bash
VOID_BOX_BASE_URL=http://127.0.0.1:43100 \
cargo test --features serde --test void_box_contract -- --ignored --nocapture
```

### 4) Start graph dashboard

```bash
cd web/void-control-ux
npm install
npm run dev
```

The dev server proxies `/api` to the daemon at `http://127.0.0.1:43100` (see
`vite.config.ts`), so the browser stays same-origin. `void-box` does not set
CORS headers, so do **not** set `VITE_VOID_BOX_BASE_URL` during local dev —
leave it unset to use the proxy. Only override it when the daemon is reachable
from a host that returns CORS (e.g., a reverse proxy in front of void-box).

### 5) Launch from YAML editor/upload (bridge)

Run bridge mode in another terminal:

```bash
cargo run --features serde --bin voidctl -- serve
```

Then start UI with bridge URL:

```bash
cd web/void-control-ux
VITE_VOID_CONTROL_BASE_URL=http://127.0.0.1:43210 \
npm run dev
```

The bridge serves CORS headers, so `VITE_VOID_CONTROL_BASE_URL` can point
directly at it. Continue to leave `VITE_VOID_BOX_BASE_URL` unset so the
Vite `/api` proxy is used for daemon calls.

### 6) Run the canonical live swarm test

Use the three-candidate swarm as the default validation path:

```bash
curl -sS -X POST http://127.0.0.1:43210/v1/executions \
  -H 'Content-Type: text/yaml' \
  --data-binary @examples/swarm-transform-optimization-3way.yaml
```

`examples/swarm-transform-optimization.yaml` remains available as the wider
eight-candidate stress case, but it is less reliable for routine validation.

This is also the canonical first-release orchestration workflow:

- load a top-level orchestration YAML
- launch through the bridge or UI
- inspect the execution graph, inspector, and event stream
- follow candidate metrics and `leader` / `broadcast` collaboration events

### Template-first bridge API

Phase 1 also exposes file-backed templates through the bridge:

```bash
curl -sS http://127.0.0.1:43210/v1/templates

curl -sS http://127.0.0.1:43210/v1/templates/single-agent-basic

curl -sS -X POST http://127.0.0.1:43210/v1/templates/single-agent-basic/dry-run \
  -H 'Content-Type: application/json' \
  -d '{
    "inputs": {
      "goal": "Summarize this repo",
      "prompt": "Read the repo and summarize risks",
      "provider": "claude"
    }
  }'

curl -sS -X POST http://127.0.0.1:43210/v1/templates/warm-agent-basic/execute \
  -H 'Content-Type: application/json' \
  -d '{
    "inputs": {
      "goal": "Keep a warm agent ready",
      "prompt": "Stay alive for follow-up repo work."
    }
  }'
```

These template endpoints compile into normal `ExecutionSpec` objects and then
reuse the existing dry-run and execution creation flow. Phase 1 ships two
starter templates:

- `single-agent-basic`
- `warm-agent-basic`

Terminal access is also available through `voidctl`:

```bash
voidctl template list
voidctl template get single-agent-basic
voidctl template dry-run single-agent-basic template-inputs.json
voidctl template execute warm-agent-basic template-inputs.json
```

`template-inputs.json` must be a JSON request body in the same shape the bridge
accepts, for example:

```json
{
  "inputs": {
    "goal": "Summarize this repo",
    "prompt": "Read the repo and summarize risks",
    "provider": "claude"
  }
}
```

Inside the interactive `voidctl` console, the same surface is available as:

```text
/template list
/template get single-agent-basic
/template dry-run single-agent-basic template-inputs.json
/template execute warm-agent-basic template-inputs.json
```

### 7) Run the supervision example

Use the checked-in supervision example to exercise the flat
orchestrator-worker path:

```bash
curl -sS -X POST http://127.0.0.1:43210/v1/executions \
  -H 'Content-Type: text/yaml' \
  --data-binary @examples/supervision-transform-review.yaml
```

Current v1 supervision contract:

- workers still run a normal runtime template on `void-box`
- approval is reducer-driven in `void-control`
- worker output must include `metrics.approved`
- the bundled supervision worker template appends that metric after the measured
  benchmark run

## Development

Rust validation:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --features serde
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
```

UI validation:

```bash
cd web/void-control-ux
npm ci
npm run build
```

Optional local pre-commit setup:

```bash
pip install pre-commit
pre-commit install
pre-commit run --all-files
```

## Terminal Console

```bash
cargo run --features serde --bin voidctl
```

## CLI Operator Flow

The non-interactive `voidctl execution ...` commands are the terminal equivalent
of the UI launcher and inspector.

Submit an orchestration spec:

```bash
voidctl execution submit examples/swarm-transform-optimization-3way.yaml
```

Dry-run the same spec:

```bash
voidctl execution dry-run examples/swarm-transform-optimization-3way.yaml
```

Submit a generated spec from stdin:

```bash
cat generated.yaml | voidctl execution submit --stdin
```

Inspect and follow an execution:

```bash
voidctl execution watch <execution-id>
voidctl execution inspect <execution-id>
voidctl execution events <execution-id>
voidctl execution result <execution-id>
voidctl execution runtime <execution-id>
```

Template-backed agent runs use the `voidctl template ...` surface and expect a
JSON request body on disk or stdin:

```json
{
  "inputs": {
    "goal": "Summarize this repo",
    "prompt": "Read the repo and summarize risks",
    "provider": "claude"
  }
}
```

Dry-run and execute a checked-in template:

```bash
voidctl template list
voidctl template get single-agent-basic
voidctl template dry-run single-agent-basic template-inputs.json
voidctl template execute warm-agent-basic template-inputs.json
```

The interactive `voidctl` console exposes the same path:

```text
/template list
/template get single-agent-basic
/template dry-run single-agent-basic template-inputs.json
/template execute warm-agent-basic template-inputs.json
```

Example execution:

```text
problem:
  optimize Transform-02 with multiple competing approaches in parallel

generated flow:
  voidctl execution submit --stdin

execution_id: exec-1775679556549
status: Completed
winner: candidate-2
strategy: vectorized-parse
runtime_run_id: run-1775679567037
```

Final candidate scores from that run:

```text
candidate-1  baseline          latency_p99_ms=3.027  cpu_pct=93.4  error_rate=0.333
candidate-2  vectorized-parse  latency_p99_ms=1.740  cpu_pct=75.8  error_rate=0.333
candidate-3  cache-aware       latency_p99_ms=3.287  cpu_pct=91.0  error_rate=0.333
candidate-4  high-throughput   latency_p99_ms=2.110  cpu_pct=97.0  error_rate=0.333
```

Follow-up commands for the same execution:

```bash
voidctl execution inspect exec-1775679556549
voidctl execution events exec-1775679556549
voidctl execution result exec-1775679556549
voidctl execution runtime exec-1775679556549
voidctl execution runtime exec-1775679556549 candidate-2
```

## Install The `void-control` Skill

The repo packages a `void-control` skill so Claude or Codex can operate the
control plane from the terminal instead of the UI.

Canonical skill source:

- [`skills/void-control/SKILL.md`](skills/void-control/SKILL.md)

Claude wrapper:

- [`.claude/skills/void-control/SKILL.md`](.claude/skills/void-control/SKILL.md)

Codex install entrypoint:

- [`.codex/INSTALL.md`](.codex/INSTALL.md)

Codex follows the same install pattern used by Superpowers: tell Codex to fetch
and follow the repo-hosted `.codex/INSTALL.md` file.

Example prompts after installation:

- `Use the void-control skill to optimize this workload with a swarm.`
- `Use the void-control skill to run this snapshot pipeline and summarize the result.`
- `Use the void-control skill to inspect why this execution failed and resolve the runtime run behind it.`
- `Use the void-control skill to generate a spec from this problem statement and submit it through voidctl.`
- `Use the void-control skill to dispatch a swarm of agents for a complex problem, let it continue in the background, and later summarize the result.`

For Claude-backed swarm/service runs, the skill should prefer the validated
service pattern:

- `agent.mode: service`
- `llm.provider: claude`
- `sandbox.network: true`
- `agent.output_file` set
- runtime-assets directory mount when possible
- `agent.messaging.enabled: true` for sibling swarm candidates

## Notes

- Dashboard uses daemon APIs (`/v1/runs`, `/v1/runs/{id}/events`, `/v1/runs/{id}/stages`, `/v1/runs/{id}/telemetry`).
- `+ Launch Spec` supports:
  - orchestration YAML through bridge execution create (`POST /v1/executions`)
  - raw runtime spec upload through bridge launch (`POST /v1/launch`)
  - path-only fallback launch (`POST /v1/runs`) when no spec text is provided
