# Real Transform Swarm Example Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the synthetic metric generation in `examples/swarm-transform-optimization.yaml` with a self-contained measured benchmark that writes real latency, error-rate, and CPU metrics to `/workspace/output.json`.

**Architecture:** Keep `void-control` unchanged as the orchestrator and scorer. Move metric truth into a deterministic benchmark runner under `examples/void-box/` that processes a local fixture corpus, writes structured output directly, and is launched by the existing service-mode `void-box` template.

**Tech Stack:** Rust orchestration already in repo, YAML runtime templates, Python benchmark runner, JSONL fixtures, serde-based bridge/runtime collection.

---

## File Structure

### Existing files to modify

- Modify: `examples/README.md`
  - explain that the transform swarm now uses measured metrics from a local fixture replay
  - add an ASCII run/metrics diagram
- Modify: `examples/void-box/transform_optimizer_agent.yaml`
  - replace metric invention instructions with benchmark-runner execution flow
  - keep service-mode output contract compatible with `void-control`

### New files to create

- Create: `examples/void-box/transform_benchmark.py`
  - benchmark runner and metric writer
- Create: `examples/void-box/fixtures/transform_02/batch_01.jsonl`
- Create: `examples/void-box/fixtures/transform_02/batch_02.jsonl`
- Create: `examples/void-box/fixtures/transform_02/batch_03.jsonl`
- Create: `examples/void-box/fixtures/transform_02/expected_summary.json`
  - deterministic corpus metadata and validation expectations

### Existing tests/docs to review while implementing

- Review: `examples/swarm-transform-optimization.yaml`
- Review: `src/runtime/void_box.rs`
  - confirm output-file retrieval expectations remain unchanged
- Review: `tests/void_box_contract.rs`
  - reuse the structured output shape assumptions

## Chunk 1: Add Deterministic Fixture Corpus

### Task 1: Create the fixture directory and metadata file

**Files:**
- Create: `examples/void-box/fixtures/transform_02/expected_summary.json`

- [ ] **Step 1: Write the corpus metadata file**

Include fields for:

```json
{
  "dataset_name": "transform_02",
  "total_records": 0,
  "expected_invalid_records": 0,
  "notes": [
    "repeated accounts for cache-aware candidate",
    "mixed record sizes for batch tradeoffs",
    "edge-case malformed records for validation/error-rate differences"
  ]
}
```

- [ ] **Step 2: Fill in real counts after fixture files are added**

No command yet. Update the numbers only after Tasks 2-4 are complete.

- [ ] **Step 3: Commit**

```bash
git add examples/void-box/fixtures/transform_02/expected_summary.json
git commit -m "examples: add transform fixture metadata"
```

### Task 2: Add first batch of valid repeated-key records

**Files:**
- Create: `examples/void-box/fixtures/transform_02/batch_01.jsonl`

- [ ] **Step 1: Write a small JSONL batch with repeated keys**

Use one record per line shaped like:

```json
{"id":"evt-001","kind":"order","payload":{"account":"A12","region":"us-east","amount":42.10,"items":[1,2,3]},"expected_valid":true}
```

Requirements:
- repeated `account` and `region` values
- small and medium payload sizes
- all records initially valid

- [ ] **Step 2: Sanity-check the file parses as JSONL**

Run:

```bash
python3 - <<'PY'
import json, pathlib
path = pathlib.Path("examples/void-box/fixtures/transform_02/batch_01.jsonl")
for idx, line in enumerate(path.read_text().splitlines(), 1):
    json.loads(line)
print("ok")
PY
```

Expected: `ok`

- [ ] **Step 3: Commit**

```bash
git add examples/void-box/fixtures/transform_02/batch_01.jsonl
git commit -m "examples: add first transform fixture batch"
```

### Task 3: Add mixed-size and malformed edge-case records

**Files:**
- Create: `examples/void-box/fixtures/transform_02/batch_02.jsonl`
- Create: `examples/void-box/fixtures/transform_02/batch_03.jsonl`

- [ ] **Step 1: Write the second batch with mixed payload sizes**

Include:
- larger `items` arrays
- mixed `kind` values if useful
- repeated keys that still benefit from caching

- [ ] **Step 2: Write the third batch with edge cases**

Include a small number of:
- malformed structures
- missing required fields
- values that should fail strict validation

Every line still needs to be valid JSON; the records should be semantically invalid, not unparsable JSON.

- [ ] **Step 3: Verify both files parse**

Run:

```bash
python3 - <<'PY'
import json, pathlib
root = pathlib.Path("examples/void-box/fixtures/transform_02")
for name in ("batch_02.jsonl", "batch_03.jsonl"):
    for idx, line in enumerate((root / name).read_text().splitlines(), 1):
        json.loads(line)
print("ok")
PY
```

Expected: `ok`

- [ ] **Step 4: Update `expected_summary.json` with real counts**

Set:
- `total_records`
- `expected_invalid_records`

- [ ] **Step 5: Commit**

```bash
git add examples/void-box/fixtures/transform_02/batch_02.jsonl examples/void-box/fixtures/transform_02/batch_03.jsonl examples/void-box/fixtures/transform_02/expected_summary.json
git commit -m "examples: add transform edge-case fixtures"
```

## Chunk 2: Implement the Benchmark Runner

### Task 4: Write a failing runner smoke test command manually

**Files:**
- Create: `examples/void-box/transform_benchmark.py`

- [ ] **Step 1: Create the runner file with a stub main that exits non-zero**

Minimal stub:

```python
#!/usr/bin/env python3
raise SystemExit("not implemented")
```

- [ ] **Step 2: Run it to verify failure**

Run:

```bash
python3 examples/void-box/transform_benchmark.py
```

Expected: non-zero exit with `not implemented`

- [ ] **Step 3: Commit**

```bash
git add examples/void-box/transform_benchmark.py
git commit -m "examples: scaffold transform benchmark runner"
```

### Task 5: Implement fixture loading and baseline processing

**Files:**
- Modify: `examples/void-box/transform_benchmark.py`

- [ ] **Step 1: Add a failing direct-run smoke path**

Expected behavior:
- load all `batch_*.jsonl` files
- count records
- emit a structured output JSON file when `OUTPUT_FILE` or `/workspace/output.json` is available

- [ ] **Step 2: Run the runner and verify failure explains missing logic**

Run:

```bash
python3 examples/void-box/transform_benchmark.py
```

Expected: FAIL because processing/output writing is not implemented yet

- [ ] **Step 3: Implement minimal fixture loading and baseline transform behavior**

Implementation responsibilities:
- read fixture files from `examples/void-box/fixtures/transform_02/`
- implement default baseline processing path
- record per-record timing
- compute invalid vs valid counts

- [ ] **Step 4: Write output JSON to a temp path during local dev**

Support:
- `OUTPUT_FILE` env override for local runs
- fallback to `/workspace/output.json` in real service runs

- [ ] **Step 5: Run the runner and verify output file exists**

Run:

```bash
OUTPUT_FILE=/tmp/transform-example-output.json python3 examples/void-box/transform_benchmark.py
cat /tmp/transform-example-output.json
```

Expected:
- valid JSON
- `status: success`
- `metrics` object present

- [ ] **Step 6: Commit**

```bash
git add examples/void-box/transform_benchmark.py
git commit -m "examples: implement baseline transform benchmark"
```

### Task 6: Implement candidate strategy branches

**Files:**
- Modify: `examples/void-box/transform_benchmark.py`

- [ ] **Step 1: Add a failing local comparison check**

Run two local commands with different env vars and confirm they currently produce identical behavior:

```bash
OUTPUT_FILE=/tmp/baseline.json TRANSFORM_STRATEGY=baseline python3 examples/void-box/transform_benchmark.py
OUTPUT_FILE=/tmp/cache.json TRANSFORM_STRATEGY=cache-aware TRANSFORM_CACHE_MODE=hot-path python3 examples/void-box/transform_benchmark.py
```

Expected before implementation:
- outputs too similar to demonstrate meaningful strategy variance

- [ ] **Step 2: Implement real behavior differences for all strategy modes**

Implement:
- baseline
- vectorized-parse
- batch-fusion
- cache-aware
- conservative-validation
- speculative-prefetch
- low-cpu
- high-throughput

The implementation should avoid fake constants. Strategy differences should come from actual code-path changes.

- [ ] **Step 3: Re-run local comparisons**

Run:

```bash
OUTPUT_FILE=/tmp/baseline.json TRANSFORM_STRATEGY=baseline python3 examples/void-box/transform_benchmark.py
OUTPUT_FILE=/tmp/cache.json TRANSFORM_STRATEGY=cache-aware TRANSFORM_CACHE_MODE=hot-path python3 examples/void-box/transform_benchmark.py
OUTPUT_FILE=/tmp/strict.json TRANSFORM_STRATEGY=conservative-validation TRANSFORM_VALIDATION_MODE=strict python3 examples/void-box/transform_benchmark.py
```

Expected:
- all files valid JSON
- at least some metric differences across strategies

- [ ] **Step 4: Commit**

```bash
git add examples/void-box/transform_benchmark.py
git commit -m "examples: add transform strategy benchmark variants"
```

### Task 7: Implement metric calculations explicitly

**Files:**
- Modify: `examples/void-box/transform_benchmark.py`

- [ ] **Step 1: Add or refine metric helpers**

Implement helpers for:
- p99 latency
- error-rate calculation
- CPU percentage estimation from process CPU time and wall time

- [ ] **Step 2: Verify output schema and numeric bounds**

Run:

```bash
OUTPUT_FILE=/tmp/transform-example-output.json python3 examples/void-box/transform_benchmark.py
python3 - <<'PY'
import json
data = json.load(open('/tmp/transform-example-output.json'))
assert data["status"] == "success"
metrics = data["metrics"]
assert metrics["latency_p99_ms"] > 0
assert 0 <= metrics["error_rate"] <= 1
assert 0 <= metrics["cpu_pct"] <= 100
print("ok")
PY
```

Expected: `ok`

- [ ] **Step 3: Commit**

```bash
git add examples/void-box/transform_benchmark.py
git commit -m "examples: finalize measured transform metrics"
```

## Chunk 3: Wire the Runner into the `void-box` Template

### Task 8: Update the runtime template to execute the benchmark runner

**Files:**
- Modify: `examples/void-box/transform_optimizer_agent.yaml`

- [ ] **Step 1: Write the failing expectation down**

Document in the file comments or working notes:
- the agent must stop inventing metrics
- the benchmark runner must own `/workspace/output.json`

- [ ] **Step 2: Update the template**

Required changes:
- ensure `transform_benchmark.py` and fixture files are available in the candidate workspace
- have the candidate execute the runner before completion
- preserve `agent.mode: service`
- preserve `agent.output_file: /workspace/output.json`
- keep swarm messaging instructions if still needed, but make measured output authoritative

- [ ] **Step 3: Verify local YAML structure**

Run:

```bash
sed -n '1,260p' examples/void-box/transform_optimizer_agent.yaml
```

Expected:
- service mode still present
- output file still `/workspace/output.json`
- prompt no longer claims the agent should invent metrics

- [ ] **Step 4: Commit**

```bash
git add examples/void-box/transform_optimizer_agent.yaml
git commit -m "examples: wire transform benchmark into service template"
```

## Chunk 4: Documentation and Live Validation

### Task 9: Update example docs with the real benchmark explanation

**Files:**
- Modify: `examples/README.md`

- [ ] **Step 1: Add an ASCII diagram**

Show:
- top-level swarm spec
- per-candidate runtime launch
- local benchmark runner
- `/workspace/output.json`
- metric collection and winner reduction

- [ ] **Step 2: Explain baseline and metric origin**

Add a short section covering:
- baseline candidate definition
- metric source of truth
- why the example is now measured rather than self-reported

- [ ] **Step 3: Verify the doc reads cleanly**

Run:

```bash
sed -n '1,260p' examples/README.md
```

Expected:
- new run-flow section visible
- baseline and metric source explained clearly

- [ ] **Step 4: Commit**

```bash
git add examples/README.md
git commit -m "docs: explain real transform swarm benchmark"
```

### Task 10: Run local non-live benchmark smoke checks

**Files:**
- Test: `examples/void-box/transform_benchmark.py`

- [ ] **Step 1: Run the benchmark locally under several strategies**

Run:

```bash
OUTPUT_FILE=/tmp/baseline.json TRANSFORM_STRATEGY=baseline python3 examples/void-box/transform_benchmark.py
OUTPUT_FILE=/tmp/cache.json TRANSFORM_STRATEGY=cache-aware TRANSFORM_CACHE_MODE=hot-path python3 examples/void-box/transform_benchmark.py
OUTPUT_FILE=/tmp/strict.json TRANSFORM_STRATEGY=conservative-validation TRANSFORM_VALIDATION_MODE=strict python3 examples/void-box/transform_benchmark.py
```

Expected:
- all complete successfully
- metric values differ in plausible ways

- [ ] **Step 2: Compare outputs manually**

Run:

```bash
cat /tmp/baseline.json
cat /tmp/cache.json
cat /tmp/strict.json
```

Expected:
- lower latency for some optimized variants
- lower error rate for strict validation
- observable CPU tradeoffs across strategies

- [ ] **Step 3: Commit if code changed during smoke-fix iteration**

```bash
git add examples/void-box/transform_benchmark.py examples/void-box/transform_optimizer_agent.yaml examples/README.md
git commit -m "examples: polish transform benchmark smoke path"
```

### Task 11: Run the live swarm example against production `void-box`

**Files:**
- Test: `examples/swarm-transform-optimization.yaml`
- Test: `examples/void-box/transform_optimizer_agent.yaml`
- Test: `examples/void-box/transform_benchmark.py`

- [ ] **Step 1: Build the production `void-box` rootfs**

Run:

```bash
cd /home/diego/github/agent-infra/void-box
TMPDIR=$PWD/target/tmp scripts/build_claude_rootfs.sh
```

Expected: production rootfs built successfully

- [ ] **Step 2: Start live services**

Run in the sibling repo:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
export VOID_BOX_KERNEL=/boot/vmlinuz-$(uname -r)
export VOID_BOX_INITRAMFS=$PWD/target/void-box-rootfs.cpio.gz
cargo run --bin voidbox -- serve --listen 127.0.0.1:43100
```

Run in this repo:

```bash
cargo run --features serde --bin voidctl -- serve
```

- [ ] **Step 3: Submit the swarm execution**

Run:

```bash
curl -sS -X POST http://127.0.0.1:43210/v1/executions \
  -H 'Content-Type: text/yaml' \
  --data-binary @examples/swarm-transform-optimization.yaml
```

Expected:
- execution created successfully

- [ ] **Step 4: Poll until terminal**

Run:

```bash
curl -sS http://127.0.0.1:43210/v1/executions/<execution_id>
```

Expected:
- candidates dispatch in parallel
- outputs collected
- execution reaches terminal state

- [ ] **Step 5: Verify winner metrics came from measured output**

Inspect:
- the winning candidate record in `/tmp/void-control/executions/<execution_id>/candidates/`
- the winning `void-box` run output file endpoint

Expected:
- metrics match benchmark-produced output
- winner explanation is rooted in measured tradeoffs, not prompt-only narrative

- [ ] **Step 6: Final commit**

```bash
git add examples/README.md examples/void-box/transform_optimizer_agent.yaml examples/void-box/transform_benchmark.py examples/void-box/fixtures/transform_02
git commit -m "examples: make transform swarm metrics real"
```
