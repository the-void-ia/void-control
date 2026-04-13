---
name: void-control
description: Use when operating void-control from the terminal, launching orchestration or runtime specs, watching execution progress, inspecting results, or resolving runtime runs through the unified execution model.
---

# Void-Control Operator Skill

Use this skill when the task is to operate `void-control` from the terminal instead of the UI.

This skill is over `void-control`, not over `void-box`.

## Core Model

Everything is an execution.

- orchestration spec such as `swarm` or `supervision` -> native execution
- raw runtime spec such as a pipeline, agent, or workload -> wrapped execution

After submission, use the same commands in both cases:

- `voidctl execution watch <execution-id>`
- `voidctl execution inspect <execution-id>`
- `voidctl execution events <execution-id>`
- `voidctl execution result <execution-id>`
- `voidctl execution runtime <execution-id> [candidate-id]`

## Spec Choices

Choose the spec shape before you submit.

- orchestration spec
  - use for swarm-style exploration or supervision-style orchestrator-worker flows
  - choose this when the problem needs multiple workers, iterations, review/finalization, or agent collaboration
- workflow or pipeline spec
  - use for one structured execution flow with ordered stages
  - choose this when the user wants one concrete process to run
- agent spec
  - use for one focused agent task
  - choose this when the job is a single agent behavior, not a multi-candidate search
- raw runtime or workload spec
  - use when the user already has a `void-box` runtime document
  - submit it through `voidctl` and let `void-control` wrap it into an execution

Decision rule:

- parallel exploration or orchestrator-worker problem -> orchestration spec
- single structured run -> workflow or pipeline spec
- single focused agent task -> agent spec
- existing runtime YAML from the user -> submit it as-is

Strategy rule inside orchestration specs:

- use `swarm` when multiple sibling candidates should explore and compete in parallel
- use `supervision` when one supervisor should review worker outputs and finalize only after approval

## Known-Good Patterns

For Claude-backed swarm or service runs, prefer the validated service template
shape over ad hoc runtime invention.

- use `agent.mode: service`
- set `llm.provider: claude`
- set `sandbox.network: true` for remote LLM access
- set `agent.output_file`
- prefer mounting a small runtime-assets directory instead of a one-off file
- for sibling swarm runs, prefer `agent.messaging.enabled: true`

If a known-good checked-in template exists, prefer adapting it over inventing a
new runtime shape from scratch.

## Avoid Unsafe Runtime Specs

Do not invent arbitrary Claude runtime combinations when a known-good service
pattern already exists.

Avoid:

- remote LLM provider with `network: false`
- arbitrary provider names when `claude` is the validated path
- ad hoc runtime images or container assumptions if a repo template already
  exists
- raw `curl` against bridge endpoints when `voidctl execution ...` can answer
  the question
- ad hoc grep polling against mixed output when execution-level commands exist

## Commands

Submit a checked-in spec:

```bash
voidctl execution submit examples/swarm-transform-optimization-3way.yaml
```

Submit a generated spec from stdin:

```bash
cat generated.yaml | voidctl execution submit --stdin
```

Dry-run a spec:

```bash
voidctl execution dry-run examples/swarm-transform-optimization-3way.yaml
cat generated.yaml | voidctl execution dry-run --stdin
```

Watch an execution:

```bash
voidctl execution watch <execution-id>
```

Inspect an execution:

```bash
voidctl execution inspect <execution-id>
```

Show execution events:

```bash
voidctl execution events <execution-id>
```

Summarize the result:

```bash
voidctl execution result <execution-id>
```

Resolve the runtime run behind the execution:

```bash
voidctl execution runtime <execution-id>
voidctl execution runtime <execution-id> <candidate-id>
```

## Working Pattern

1. Decide whether the problem should become an orchestration spec or a raw runtime spec.
2. If needed, generate YAML on the fly.
3. Submit it through `voidctl execution submit`.
4. Watch or inspect the execution.
5. Use `result` for the terminal summary.
6. Use `runtime` when you need the underlying runtime run.

## Example Usage

Use the skill from a problem statement, not only from checked-in examples.

- "Use the void-control skill to optimize this API transform stage for latency and CPU."
- "Use the void-control skill to run this snapshot pipeline and summarize the result."
- "Use the void-control skill to inspect why this execution failed and show me the runtime run behind the winning candidate."
- "Use the void-control skill to generate a swarm spec for this workload and submit it through `voidctl`."
- "Use the void-control skill to generate a supervision spec for this review workflow and finalize only after approved worker output."
- "Use the void-control skill to dispatch a swarm of agents for this complex problem, let it continue in the background, and later summarize the result."

Typical generated workflow:

1. turn the problem into orchestration YAML or a raw runtime spec
2. submit it with `voidctl execution submit --stdin`
3. follow it with `watch` or `inspect`
4. summarize it with `result`
5. drill into runtime with `runtime` if needed

For background work, keep the returned `execution_id` and come back later with:

- `voidctl execution inspect <execution-id>`
- `voidctl execution events <execution-id>`
- `voidctl execution result <execution-id>`
- `voidctl execution runtime <execution-id>`

When polling or revisiting later, reason from the execution status first, not
from candidate lines.

## Expectations

- Use `void-control` bridge APIs through `voidctl`, not raw `curl`, unless the user explicitly asks for raw HTTP.
- Prefer checked-in example specs for reproducible runs.
- For generated specs, prefer `--stdin` so the execution record owns the submitted YAML.
