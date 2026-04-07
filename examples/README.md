# Execution Examples

These examples are intentionally split across two layers:

- `examples/*.yaml`: `void-control` execution specs
- `examples/void-box/*.yaml`: runtime workflow templates launched by `void-box`

Boundary:

- `void-control` owns orchestration concerns such as `mode`, `policy`, `evaluation`,
  `variation`, and `swarm`.
- `void-box` only receives a runtime template path plus a patched per-candidate
  workflow spec. It does not need to understand swarm/search strategies.

## Files

- `swarm-transform-optimization-3way.yaml`
  - canonical Transform-02 swarm test
  - launches 3 sibling candidates in parallel
  - most reliable live validation path today
- `swarm-transform-optimization.yaml`
  - wide Transform-02 swarm stress example
  - launches 8 sibling candidates in parallel
  - scores `latency_p99_ms`, `error_rate`, and `cpu_pct`
- `search-rate-limit-optimization.yaml`
  - incumbent-centered search example for `Transform-02`
  - mutates around a promising rate-limit policy
- `void-box/transform_optimizer_agent.yaml`
  - plain runtime template used by the swarm example
- `void-box/rate_limit_optimizer_agent.yaml`
  - plain runtime template used by the search example

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
each run mounts examples/void-box read-only
    ->
python3 /workspace/transform-example/transform_benchmark.py
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

- `examples/void-box/transform_benchmark.py` computes the metrics
- `latency_p99_ms` comes from measured per-record timings
- `error_rate` comes from actual validation/transform failures on the fixtures
- `cpu_pct` comes from measured process CPU time versus wall-clock time
- the agent reads the measured result and may summarize it, but it must not
  invent or overwrite the metrics

## Prerequisites

Build the production `void-box` rootfs in the sibling repo:

```bash
cd /home/diego/github/agent-infra/void-box
TMPDIR=$PWD/target/tmp scripts/build_claude_rootfs.sh
```

Start the `void-box` daemon with a real Anthropic key:

```bash
cd /home/diego/github/agent-infra/void-box
export ANTHROPIC_API_KEY=sk-ant-...
export VOID_BOX_KERNEL=/boot/vmlinuz-$(uname -r)
export VOID_BOX_INITRAMFS=$PWD/target/void-box-rootfs.cpio.gz
cargo run --bin voidbox -- serve --listen 127.0.0.1:43100
```

The transform example also assumes the daemon is started from the sibling
`void-box` repo root so this runtime template mount resolves correctly:

```yaml
../../void-control/examples/void-box -> /workspace/transform-example
```

Start the `void-control` bridge:

```bash
cd /home/diego/github/void-control
cargo run --features serde --bin voidctl -- serve
```

## Launch From CLI

Swarm:

```bash
cd /home/diego/github/void-control
curl -sS -X POST http://127.0.0.1:43210/v1/executions \
  -H 'Content-Type: text/yaml' \
  --data-binary @examples/swarm-transform-optimization-3way.yaml
```

Stress swarm:

```bash
cd /home/diego/github/void-control
curl -sS -X POST http://127.0.0.1:43210/v1/executions \
  -H 'Content-Type: text/yaml' \
  --data-binary @examples/swarm-transform-optimization.yaml
```

Search:

```bash
cd /home/diego/github/void-control
curl -sS -X POST http://127.0.0.1:43210/v1/executions \
  -H 'Content-Type: text/yaml' \
  --data-binary @examples/search-rate-limit-optimization.yaml
```

## Launch From UI

1. Start the UI:

```bash
cd /home/diego/github/void-control/web/void-control-ux
npm run dev -- --host 127.0.0.1 --port 3000
```

2. Open the Launch modal.
3. Upload one of the top-level execution specs from `examples/`.
4. Keep the referenced runtime template path unchanged unless you moved the repo.

## Notes

- The transform runtime template uses `agent.output_file: /workspace/output.json`
  so `void-control` can collect structured metrics directly.
- `examples/void-box/transform_optimizer_agent.yaml` uses
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
