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

- `swarm-transform-optimization.yaml`
  - wide swarm example for `Transform-02`
  - launches 8 sibling candidates in parallel
  - scores `latency_p99_ms`, `error_rate`, and `cpu_pct`
- `search-rate-limit-optimization.yaml`
  - incumbent-centered search example for `Transform-02`
  - mutates around a promising rate-limit policy
- `void-box/transform_optimizer_agent.yaml`
  - plain runtime template used by the swarm example
- `void-box/rate_limit_optimizer_agent.yaml`
  - plain runtime template used by the search example

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

Start the `void-control` bridge:

```bash
cd /home/diego/github/void-control
cargo run --features serde --bin voidctl -- serve
```

## Launch From CLI

Swarm:

```bash
cd /home/diego/github/void-control
cargo run --features serde --bin voidctl -- execution create examples/swarm-transform-optimization.yaml
```

Search:

```bash
cd /home/diego/github/void-control
cargo run --features serde --bin voidctl -- execution create examples/search-rate-limit-optimization.yaml
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

- The runtime templates use `agent.output_file: /workspace/result.json` so
  `void-control` can collect structured metrics directly.
- Candidate variation is applied by `void-control` by patching fields such as
  `agent.prompt` and `sandbox.env.*` before launch.
- The strategy labels in env vars are only inputs for the prompt/runtime
  template. `void-box` does not interpret them semantically.
