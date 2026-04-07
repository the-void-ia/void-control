# Real Transform Swarm Example Design

## Goal

Replace the self-reported metrics in `examples/swarm-transform-optimization.yaml`
with measured metrics from a deterministic local workload, while preserving the
current swarm orchestration flow in `void-control`.

The result should remain self-contained inside this repo and runnable through the
existing `void-control` -> `void-box` live workflow.

## Current Problem

The current example is operationally real but benchmark-wise synthetic:

- `void-control` really creates and tracks swarm candidates
- `void-box` really launches service-mode runs
- candidates really run in parallel
- winner selection is real
- but the metrics are currently produced by the agent prompt rather than a
  measured workload

That makes the example good for orchestration validation but weak as a
performance optimization example.

## Target Shape

```text
examples/swarm-transform-optimization.yaml
    ->
void-control creates 8 candidates
    ->
void-box launches 8 service-mode runs
    ->
each run executes the same local benchmark runner
    ->
candidate env vars change benchmark behavior
    ->
runner measures latency/error/cpu
    ->
runner writes /workspace/output.json
    ->
void-control collects metrics and scores candidates
```

## Repository Layout

```text
examples/
  swarm-transform-optimization.yaml
  void-box/
    transform_optimizer_agent.yaml
    transform_benchmark.py
    fixtures/
      transform_02/
        batch_01.jsonl
        batch_02.jsonl
        batch_03.jsonl
        expected_summary.json
```

## Boundary

- `void-control` remains responsible for:
  - candidate creation
  - per-candidate override injection
  - dispatch/scheduling
  - result collection
  - scoring and convergence
- `void-box` example files remain responsible for:
  - the actual candidate workload
  - metric production
  - writing `/workspace/output.json`

The benchmark runner belongs under `examples/runtime-assets/` because that is the
workload the candidate run actually executes.

## Fixture Design

The benchmark uses a deterministic local fixture corpus in JSONL format. Each
line is one work item.

Example record:

```json
{"id":"evt-001","kind":"order","payload":{"account":"A12","region":"us-east","amount":42.10,"items":[1,2,3]},"expected_valid":true}
```

The fixture corpus should intentionally include:

- repeated keys to make cache behavior observable
- mixed record sizes to create batching tradeoffs
- records that are easy to validate
- a small number of malformed or edge-case records to expose error-rate
  differences

`expected_summary.json` stores corpus metadata such as total records and expected
validation counts. It should not store target performance metrics.

## Benchmark Runner

`transform_benchmark.py` is the source of truth for measured metrics.

Responsibilities:

- load all fixture files
- read candidate behavior from environment variables
- process the same corpus for every candidate
- record actual execution timings
- count failures and invalid outputs
- estimate CPU usage from process CPU time and wall-clock time
- write `/workspace/output.json`

The benchmark runner should be deterministic enough for example/demo use, even
if the exact measured numbers vary slightly between runs.

## Output Contract

The runner writes the final structured output directly:

```json
{
  "status": "success",
  "summary": "measured benchmark result for cache-locality role",
  "metrics": {
    "latency_p99_ms": 42.0,
    "error_rate": 0.012,
    "cpu_pct": 52.0
  },
  "artifacts": []
}
```

The agent must not invent metric values. If the agent remains in the flow, it
may only help with collaboration or produce a short summary that is derived from
measured results.

Preferred implementation: the benchmark runner writes the full output JSON and
the template treats that file as the canonical result.

## Metrics

### latency_p99_ms

- measure elapsed time per processed unit or per benchmark batch
- compute the 99th percentile from observed timings
- emit milliseconds

### error_rate

- compute `failed_units / total_units`
- failures include parse errors, validation failures, and transform mismatches

### cpu_pct

- compute:

```text
process_cpu_time_delta / wall_clock_delta * 100
```

- clamp to `0..100` for this example
- measured but approximate is acceptable; invented is not

## Candidate Semantics

Each candidate env override must change actual benchmark behavior.

### baseline

- sequential/default implementation
- default batch size
- cache disabled
- balanced validation
- no prefetch

### vectorized-parse

- faster parsing path over homogeneous records
- expected to reduce latency on parse-heavy inputs
- may be slightly less robust on malformed edge cases

### batch-fusion

- larger batch aggregation before transform
- expected to improve throughput and some latency profiles
- may increase CPU or tail behavior on larger batches

### cache-aware

- memoize repeated normalization/lookups for repeated keys
- expected to help repeated-account and repeated-region fixture patterns

### conservative-validation

- apply extra validation checks before transform commit
- expected to lower error rate at the cost of latency

### speculative-prefetch

- precompute or look ahead for upcoming records
- expected to improve latency on some access patterns
- may consume more CPU

### low-cpu

- intentionally reduced work chunking or concurrency
- expected to reduce `cpu_pct`
- expected to worsen latency

### high-throughput

- aggressive chunking or concurrency
- expected to reduce latency
- expected to increase `cpu_pct`
- may expose more edge-case failures

## Baseline Definition

Baseline is candidate 1 from `examples/swarm-transform-optimization.yaml`:

- `TRANSFORM_STRATEGY=baseline`
- `TRANSFORM_PARALLELISM=2`
- `TRANSFORM_ROLE=latency-baseline`

All candidates are evaluated against:

- the same fixture corpus
- the same runner
- the same output schema
- the same scoring logic

Only the candidate strategy knobs differ.

## Scoring

The top-level orchestration spec can remain mostly unchanged:

- `latency_p99_ms`: `-0.50`
- `error_rate`: `-0.35`
- `cpu_pct`: `-0.15`

Lower values remain better for all three metrics.

`void-control` continues to score candidates from collected structured output.

## Agent Role

The agent still has value for:

- swarm messaging
- role-specific observations
- leader recommendations

But the agent should not be the measurement authority.

Recommended split:

- benchmark runner owns `metrics`
- agent may add context to `summary`
- `void-control` consumes the final structured output exactly as before

## Why This Counts As Real

This remains an example, but it becomes a real benchmarked example because:

- each candidate executes actual code
- each candidate processes the same local workload
- metrics come from measured execution
- winner selection is based on observed results rather than narrative output

## Success Criteria

The updated example should:

- run from the existing live swarm workflow
- produce stable enough metrics for demos and debugging
- show visible tradeoffs between candidate strategies
- keep the baseline explicit and understandable
- complete quickly enough for local operator use

## Implementation Order

1. Add deterministic fixture corpus under `examples/runtime-assets/fixtures/transform_02/`
2. Add `examples/runtime-assets/transform_benchmark.py`
3. Update `examples/runtime-templates/transform_optimizer_agent.yaml` to execute the runner
4. Keep `examples/swarm-transform-optimization.yaml` as the top-level swarm spec
5. Update `examples/README.md` with an ASCII run/metrics diagram
6. Run a live swarm and verify that the winner is derived from measured output
