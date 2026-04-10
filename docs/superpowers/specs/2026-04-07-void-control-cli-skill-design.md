# Void-Control CLI Skill Design

Date: 2026-04-07

## Goal

Define a first-release `void-control` skill that can drive the same control-plane
flows as the UI from the terminal. The skill must support both orchestration
specs such as `swarm` and raw runtime specs such as pipelines, agents, and
workloads.

The CLI and the skill should operate on one model: `execution`.

## Problem

`void-control` already has the operator UI and the bridge APIs needed to launch,
observe, and inspect orchestration work. It also already has partial interactive
console support in `voidctl`. What it does not have is a stable non-interactive
CLI surface that an operator skill can use to do the same job as the UI.

Without that surface:

- agents have to call raw bridge endpoints directly
- humans and agents end up with different workflows
- runtime specs and orchestration specs feel like separate products
- result inspection and runtime drill-down stay UI-only

## First-Release Principle

The skill is over `void-control`, not over `void-box`.

That means:

- `void-control` owns submit, inspect, events, metrics, result, and runtime
  drill-down at the execution level
- `void-box` remains the runtime executor and source of run/stage/artifact data
- the skill uses `voidctl`, and `voidctl` talks to the bridge

The stack is:

```text
skill
  -> voidctl execution ...
       -> bridge HTTP API
            -> execution service / store / runtime
```

## Scope

### In scope

- one non-interactive `voidctl execution ...` command family
- one `void-control` skill that uses those commands
- orchestration-spec submission
- raw-runtime-spec submission through internal wrapping into an execution
- execution watch, inspect, result, and runtime drill-down
- stable text output suitable for both humans and agents

### Out of scope

- direct `curl`-based skill workflows
- a separate skill for each strategy
- MCP-native CLI integration in the first release
- replacing the UI
- changing the `void-box` runtime contract

## User Experience

The first-release user should be able to do both of these with the same skill:

```text
1. Drop a problem into a swarm spec and let the strategy explore candidates.
2. Run a plain runtime spec such as a pipeline and inspect the result.
```

After submission, both flows become the same:

- there is an execution ID
- the operator can watch progress
- inspect candidates or stages
- inspect metrics and events
- drill into runtime details
- summarize the result

## CLI Surface

The recommended non-interactive CLI surface is:

```text
voidctl execution submit <spec-path>
voidctl execution submit --stdin
voidctl execution dry-run <spec-path>
voidctl execution dry-run --stdin
voidctl execution watch <execution-id>
voidctl execution inspect <execution-id>
voidctl execution events <execution-id>
voidctl execution result <execution-id>
voidctl execution runtime <execution-id> [candidate-id]
```

## Command Semantics

### `voidctl execution submit`

Accepts either a YAML path or YAML from stdin and detects document type.

- orchestration spec:
  - submit directly through the execution create path
- raw runtime spec:
  - wrap into a minimal control-plane execution
  - persist it as an execution before launch

Returns:

- execution ID
- mode
- goal
- initial status

This command must support agent-generated specs that do not exist as permanent
files yet.

Examples:

```bash
voidctl execution submit examples/swarm-transform-optimization-3way.yaml
cat generated.yaml | voidctl execution submit --stdin
```

### `voidctl execution dry-run`

Validates and plans without launching.

Returns:

- mode
- iteration shape
- candidate count
- warnings
- validation errors

### `voidctl execution watch`

Polls the execution until terminal state or interruption.

Shows compact progress updates:

- status
- completed iterations
- queued/running/completed/failed counts
- current best candidate
- latest execution events

For runtime-wrapped runs, the output stays execution-centric instead of exposing
a different runtime-only product mode.

### `voidctl execution inspect`

Shows execution detail suitable for both operators and agents.

For orchestration executions:

- execution status
- completed iterations
- best candidate
- candidate list
- metrics summary
- runtime run IDs
- message stats when present

For wrapped runtime executions:

- execution status
- mapped stage summary
- terminal state
- runtime run IDs
- available artifacts/output summary

### `voidctl execution events`

Prints the control-plane event stream for an execution.

For swarm, this includes:

- `CandidateQueued`
- `CandidateDispatched`
- `CommunicationIntentEmitted`
- `MessageRouted`
- `MessageDelivered`
- `CandidateScored`
- `IterationCompleted`
- `ExecutionCompleted`

### `voidctl execution result`

Returns the final operator summary.

For swarm:

- winner
- metric comparison
- ranking summary
- candidate failures if any

For wrapped runtime:

- stage summary
- terminal outcome
- output summary

### `voidctl execution runtime`

Resolves the runtime run behind an execution.

Behavior:

- default to the best candidate when available
- otherwise default to the most relevant active candidate
- allow explicit `candidate-id`

Returns:

- runtime run ID
- selected candidate ID when applicable

This is the CLI equivalent of the UI `Open Runtime Graph` action.

## Wrapping Model For Raw Runtime Specs

Raw runtime specs should not create a parallel operator model.

Instead:

- `void-control` wraps the runtime spec internally
- the wrapped document becomes a minimal execution
- the bridge and CLI operate on that execution ID

The wrapper must stay minimal:

- preserve the original runtime spec
- add only control-plane metadata needed for launch and inspection
- do not invent swarm-like semantics for plain runtime runs

## Spec Classification

`voidctl execution submit <spec-path>` must classify the input document before
launch.

There are two first-release categories:

### 1. Orchestration specs

These are native `void-control` execution documents.

Examples:

- `examples/swarm-transform-optimization-3way.yaml`
- future `supervision` specs

Behavior:

- submit directly through the execution create path
- persist as an execution without wrapping

Example:

```bash
voidctl execution submit examples/swarm-transform-optimization-3way.yaml
```

### 2. Raw runtime specs

These are `void-box` runtime documents such as agents, pipelines, and
workloads.

Examples:

- `/home/diego/github/agent-infra/void-box/examples/specs/snapshot_pipeline.yaml`
- local pipeline specs
- local workload specs
- local agent specs

Behavior:

- detect that the document is a raw runtime spec
- wrap it into a minimal control-plane execution
- persist that wrapped execution
- launch the runtime work through the normal runtime path

Example:

```bash
voidctl execution submit /home/diego/github/agent-infra/void-box/examples/specs/snapshot_pipeline.yaml
```

The result should still be an execution ID, not a separate runtime-only handle.

### 3. Agent-generated specs

The skill must be able to create a spec on the fly from a problem statement.

Examples:

- "optimize this transform workload with a swarm"
- "run this pipeline and summarize the result"

Behavior:

- the agent chooses the appropriate spec shape
- the agent renders YAML
- the YAML is submitted through stdin
- `void-control` persists the submitted YAML with the execution
- the execution then follows the same operator lifecycle as any other launch

Example:

```bash
cat <<'EOF' | voidctl execution submit --stdin
mode: swarm
goal: optimize transform strategy
workflow:
  template: examples/runtime-templates/transform_optimizer_agent.yaml
...
EOF
```

### Classification outcome

In both cases, `submit` returns:

- execution ID
- mode
- goal
- status

After that, the rest of the CLI is the same:

```bash
voidctl execution watch <execution-id>
voidctl execution inspect <execution-id>
voidctl execution result <execution-id>
```

## On-The-Fly Spec Creation

The first-release skill must support turning a problem statement into a launchable
spec without requiring a manually prepared file.

Required behavior:

- agent receives a problem statement
- agent decides whether the problem maps to:
  - orchestration execution
  - raw runtime execution
- agent generates YAML
- agent submits via `voidctl execution submit --stdin`
- `void-control` persists the submitted spec text as part of the execution record

That persistence matters because the generated spec is part of the operator
evidence trail. The execution should remain inspectable even when the spec never
existed as a standalone checked-in file.

## Output Design

The first release should optimize for stable text output, not decorative CLI
rendering.

Requirements:

- machine-readable enough for agent use
- readable enough for operators
- deterministic field order
- no dependence on terminal interactivity

Recommended output strategy:

- default: concise human-readable blocks
- optional later extension: `--json`

## Why The Skill Should Use `voidctl`, Not Raw HTTP

Using dedicated `voidctl` subcommands is better than having the skill call the
bridge directly.

Benefits:

- one stable operator surface for humans and agents
- bridge URL, formatting, and error mapping stay centralized
- CLI behavior can evolve without rewriting the skill
- UI and CLI stay aligned on the same control-plane primitives

## Relation To The UI

The UI remains the graph-first operator surface.

The CLI skill becomes the terminal/operator equivalent using the same
underlying primitives:

- submit
- inspect
- events
- metrics
- result
- runtime drill-down

The CLI is not trying to reproduce the graph visually. It is trying to expose
the same execution model.

## Relation To Strategies

This design is strategy-agnostic at the operator layer.

For first release:

- `swarm` is the implemented orchestration strategy

Later:

- `supervision` should fit into the same CLI contract

That means the CLI must not hardcode swarm-only semantics into top-level command
shapes. Swarm-specific details belong in result and inspect output, not in the
existence of separate command families.

## Implementation Order

Recommended implementation order:

1. `voidctl execution submit`
2. `voidctl execution inspect`
3. `voidctl execution result`
4. `voidctl execution watch`
5. `voidctl execution runtime`
6. `void-control` skill document built against those commands

This order gives an agent-usable path early while keeping the command family
coherent.

## Risks

### Runtime wrapping ambiguity

Detection between orchestration and raw runtime specs must be strict enough to
avoid misclassification. The wrapper path should fail clearly when a document is
ambiguous or unsupported.

### Output drift

If human-readable output changes frequently, the skill becomes brittle. The CLI
must define a stable output contract early.

### Interactive-console overlap

`voidctl` already has an interactive console. The new subcommands should reuse
shared logic where possible, but they should not inherit console-specific
behavior or state assumptions.

## Recommendation

Build the non-interactive `voidctl execution ...` family first, then write the
`void-control` skill on top of it. That keeps the skill thin and makes the CLI a
real operator surface instead of a hidden helper for one agent implementation.
