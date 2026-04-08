---
name: void-control
description: Operate void-control from the terminal. Submit orchestration or runtime specs, watch execution progress, inspect results, and resolve runtime runs through the unified execution model.
---

# Void-Control Operator Skill

Use this skill when the task is to operate `void-control` from the terminal instead of the UI.

This skill is over `void-control`, not over `void-box`.

## Core Model

Everything is an execution.

- orchestration spec such as `swarm` -> native execution
- raw runtime spec such as a pipeline, agent, or workload -> wrapped execution

After submission, use the same commands in both cases:

- `voidctl execution watch <execution-id>`
- `voidctl execution inspect <execution-id>`
- `voidctl execution events <execution-id>`
- `voidctl execution result <execution-id>`
- `voidctl execution runtime <execution-id> [candidate-id]`

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

## Expectations

- Use `void-control` bridge APIs through `voidctl`, not raw `curl`, unless the user explicitly asks for raw HTTP.
- Prefer checked-in example specs for reproducible runs.
- For generated specs, prefer `--stdin` so the execution record owns the submitted YAML.
