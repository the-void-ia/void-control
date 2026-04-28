# Execution Examples

These examples are intentionally split across two layers:

- `examples/*.yaml`: `void-control` execution specs
- `examples/compute/*.yaml`: bridge-managed compute resource payloads
- `examples/runtime-templates/*.yaml`: runtime workflow templates launched by `void-box`
- `examples/runtime-assets/`: helper scripts and data mounted into runtime templates

Boundary:

- `void-control` owns orchestration concerns such as `mode`, `policy`, `evaluation`,
  `variation`, `swarm`, and `supervision`.
- `void-box` only receives a runtime template path plus a patched per-candidate
  workflow spec. It does not need to understand swarm/supervision strategies.

## Files

- `swarm-transform-optimization-3way.yaml`
  - canonical Transform-02 swarm test
  - launches 3 sibling candidates in parallel
  - most reliable live validation path today
- `swarm-transform-optimization.yaml`
  - wide Transform-02 swarm stress example
  - launches 8 sibling candidates in parallel
  - scores `latency_p99_ms`, `error_rate`, and `cpu_pct`
- `supervision-transform-review.yaml`
  - canonical Transform-02 supervision example
  - launches 3 worker runs under a flat supervisor
  - finalizes once every worker emits `metrics.approved = 1.0`
- `runtime-templates/transform_optimizer_agent.yaml`
  - plain runtime template used by the swarm example
- `runtime-templates/transform_supervision_worker.yaml`
  - runtime template used by the supervision example
- `compute/sandbox-python.yaml`
  - checked-in bridge payload for a reusable Python sandbox
- `compute/snapshot-from-sandbox.yaml`
  - checked-in bridge payload for snapshot creation metadata
- `compute/pool-python.yaml`
  - checked-in bridge payload for a warm-capacity pool definition

## Transform Swarm Examples

Both Transform-02 swarm examples use measured metrics from a local fixture
replay, not prompt-invented values.

Recommended test path:

- use `swarm-transform-optimization-3way.yaml` for routine live validation
- use `swarm-transform-optimization.yaml` only when you explicitly want the
  wider 8-candidate stress case

Flow:

```text
examples/swarm-transform-optimization-3way.yaml
    ->
void-control creates sibling candidates
    ->
void-box launches service-mode runs
    ->
each run mounts examples/runtime-assets read-only
    ->
python3 /workspace/runtime-assets/transform_benchmark.py
    ->
benchmark processes the same transform_02 fixture corpus
    ->
benchmark writes /workspace/output.json
    ->
void-control collects latency_p99_ms, error_rate, cpu_pct
    ->
weighted scoring picks the best candidate
```

Baseline:

- candidate 1 in both Transform-02 swarm specs is the baseline
- it uses `TRANSFORM_STRATEGY=baseline`
- every other candidate runs the same fixture corpus with different strategy
  env overrides

Metric source of truth:

- `examples/runtime-assets/transform_benchmark.py` computes the metrics
- `latency_p99_ms` comes from measured per-record timings
- `error_rate` comes from actual validation/transform failures on the fixtures
- `cpu_pct` comes from measured process CPU time versus wall-clock time
- the agent reads the measured result and may summarize it, but it must not
  invent or overwrite the metrics

## Prerequisites

These examples assume a sibling checkout layout:

```text
<workspace>/void-box       # the runtime repo
<workspace>/void-control   # this repo
```

Substitute `<workspace>` for your local path (e.g. `~/dev/repos`,
`~/github`, etc.). The runtime template mount path
`../../void-control/examples/runtime-assets` resolves to
`<workspace>/void-control/examples/runtime-assets` when the daemon is
started from `<workspace>/void-box`.

Build the production `void-box` rootfs in the sibling repo:

```bash
cd <workspace>/void-box
TMPDIR=$PWD/target/tmp scripts/build_claude_rootfs.sh
```

Start the `void-box` daemon (Linux):

```bash
cd <workspace>/void-box
export ANTHROPIC_API_KEY=sk-ant-...   # or use provider: claude-personal, see below
export VOID_BOX_KERNEL=/boot/vmlinuz-$(uname -r)
export VOID_BOX_INITRAMFS=$PWD/target/void-box-rootfs.cpio.gz
cargo run --bin voidbox -- serve --listen 127.0.0.1:43100
```

Start the `void-box` daemon (macOS, Apple Silicon):

```bash
cd <workspace>/void-box
export VOID_BOX_KERNEL=$PWD/target/vmlinuz
export VOID_BOX_INITRAMFS=$PWD/target/void-box-claude.cpio.gz
# ANTHROPIC_API_KEY is not needed if the runtime template sets
# `llm.provider: claude-personal` and you have a Claude subscription
# (credentials come from the macOS Keychain or ~/.claude/.credentials.json).
cargo run --bin voidbox -- serve --listen 127.0.0.1:43100
```

Start the `void-control` bridge:

```bash
cd <workspace>/void-control
cargo run --features serde --bin voidctl -- serve
```

### Using a Claude personal plan instead of an API key

The checked-in runtime templates hardcode `llm.provider: claude` so CI keeps
using an API key. To opt into `claude-personal` without editing tracked
templates, set `VOID_CONTROL_LLM_PROVIDER` when starting the bridge (or any
other process that launches runs through `void-control`):

```bash
cd <workspace>/void-control
VOID_CONTROL_LLM_PROVIDER=claude-personal \
  cargo run --features serde --bin voidctl -- serve
```

This patches `llm.provider` on every candidate's runtime template at launch
time. Per-candidate `variation.explicit[].overrides` still win, so you can
keep a mixed setup if needed.

## Launch From CLI

Swarm:

```bash
cd <workspace>/void-control
curl -sS -X POST http://127.0.0.1:43210/v1/executions \
  -H 'Content-Type: text/yaml' \
  --data-binary @examples/swarm-transform-optimization-3way.yaml
```

Supervision:

```bash
cd <workspace>/void-control
curl -sS -X POST http://127.0.0.1:43210/v1/executions \
  -H 'Content-Type: text/yaml' \
  --data-binary @examples/supervision-transform-review.yaml
```

Stress swarm:

```bash
cd <workspace>/void-control
curl -sS -X POST http://127.0.0.1:43210/v1/executions \
  -H 'Content-Type: text/yaml' \
  --data-binary @examples/swarm-transform-optimization.yaml
```

## Launch From UI

1. Start the UI:

```bash
cd <workspace>/void-control/web/void-control-ux
npm run dev -- --host 127.0.0.1 --port 3000
```

2. Open the Launch modal.
3. Upload one of the top-level execution specs from `examples/`.
4. Keep the referenced runtime template path unchanged unless you moved the repo.

## Notes

- The transform runtime template uses `agent.output_file: /workspace/output.json`
  so `void-control` can collect structured metrics directly.
- The supervision worker template uses
  `examples/runtime-assets/transform_supervision_worker.py` to append
  `metrics.approved` after the measured benchmark run. That is the current v1
  supervision approval contract.
- `examples/runtime-templates/transform_optimizer_agent.yaml` uses
  `sandbox.image: "python:3.12-slim"` because the production Claude rootfs does
  not include `python3`.
- The transform benchmark and fixtures are mounted into the guest read-only from
  the host repo. Keep the canonical sibling repo layout unless you also update
  the mount path in the template.
- Candidate variation is applied by `void-control` by patching fields such as
  `agent.prompt` and `sandbox.env.*` before launch.
- The strategy labels in env vars are only inputs for the prompt/runtime
  template. `void-box` does not interpret them semantically.
- The 3-agent swarm is the preferred test target because it currently completes
  more reliably than the 8-agent stress variant while exercising the same
  control-plane and service-mode primitives.
