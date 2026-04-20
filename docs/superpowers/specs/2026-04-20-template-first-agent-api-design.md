# Template-First Agent API Design

## Date

2026-04-20

## Status

Draft

## Problem

`void-control` currently exposes a low-level execution-spec model that is good
for orchestration power users but too heavy for end-user authoring.

Today, users are expected to understand and provide:

- `mode`
- `workflow.template`
- `policy`
- `evaluation`
- `variation`
- `swarm`
- optional `supervision`

That is too much surface area for the common cases we want to support next:

- run one agent with one prompt and get a response
- create a reusable warm agent
- run a team over a known template

At the same time, the current runtime boundary must be preserved:

- `void-box` owns runtime execution, sandboxing, and service-mode lifecycle
- `void-control` owns control-plane concepts, orchestration, persistence, and UX

## Current Reality

From the current codebase:

- `void-control` runtime integration is run-centric
  - `ExecutionRuntime` only exposes `start_run`, `inspect_run`, and
    `take_structured_output`
- `void-control` already uses `void-box` service-mode templates, but only as
  long-running candidate runs that eventually publish `output_file`
- `void-box` supports:
  - one-shot agent execution
  - service-mode agent execution
  - Python/Node-capable sandbox images
- `void-box` does not currently expose a general daemon API for reusable
  "exec into existing session" semantics in the same way it exposes runs

That means the first template-first API should compile to existing one-shot and
service-mode run behavior, not to an invented reusable session primitive.

## Goals

- make templates the primary user-facing authoring surface
- keep templates file-backed and versioned in git first
- support both one-shot and warm-agent flows using existing runtime behavior
- expose a stable Python/JS-friendly API from `void-control`
- preserve raw execution specs as an advanced escape hatch

## Non-Goals

- redesigning the `void-box` daemon surface in this phase
- replacing raw execution specs immediately
- introducing user-authored database-backed templates
- implementing warm agent pools in the first slice

## Primary Product Concepts

The first `void-control` product concepts should be:

- one-shot agent execution
- warm service agent execution
- later, team execution

Phase 1 should keep persistence execution-centric. Templates are a simpler front
door to creating normal `Execution` records, not a new persistence model.

That means:

- phase 1 responses may describe agent-oriented behavior
- phase 1 persisted objects should still be normal `Execution`s
- new resource kinds such as `AgentService` or `TeamExecution` may be added
  later if they are still justified after the template layer lands

## Recommended Template Strategy

Start with file-backed templates inside `void-control`.

Recommended location:

- `templates/`

Alternative acceptable location:

- `spec/templates/`

Each template file should be reviewed like code, versioned in git, and shipped
with the repo. The API should read from these files at runtime.

## Template File Model

Each template file should contain five concerns:

1. metadata
2. user-editable inputs
3. defaults
4. compilation bindings into the existing `ExecutionSpec` and runtime override model
5. optional advanced override contract

Suggested shape:

```yaml
api_version: v1
kind: control_template

template:
  id: warm-agent-basic
  name: Warm Agent
  execution_kind: warm_agent
  description: Reusable long-running agent with sane defaults

inputs:
  goal:
    type: string
    required: true
  prompt:
    type: string
    required: true
  provider:
    type: enum
    values: [claude, codex]
      default: claude

defaults:
  workflow_template: examples/runtime-templates/warm_agent_basic.yaml
  execution_spec:
    goal: Default goal
    mode: swarm
    workflow:
      template: ""
    swarm: true
    policy:
      budget:
        max_iterations: 1
        max_child_runs: 1
        max_wall_clock_secs: 3600
        max_cost_usd_millis: null
      concurrency:
        max_concurrent_candidates: 1
      convergence:
        strategy: threshold
        min_score: 1.0
        max_iterations_without_improvement: 0
      max_candidate_failures_per_iteration: 1
      missing_output_policy: mark_failed
      iteration_failure_policy: fail_execution
    evaluation:
      scoring_type: weighted_metrics
      weights:
        success: 1.0
      pass_threshold: 1.0
      ranking: highest_score
      tie_breaking: success
    variation:
      source: explicit
      candidates_per_iteration: 1
      explicit:
        - overrides: {}
    supervision: null

compile:
  bindings:
    - input: goal
      target: execution_spec.goal
    - input: prompt
      target: variation.explicit[0].overrides.agent.prompt
    - input: provider
      target: variation.explicit[0].overrides.llm.provider
```

The important constraint is that phase 1 compilation targets what already
exists today:

- `ExecutionSpec`
- runtime template path
- runtime override map

Phase 1 should not invent a new intermediate persisted object model such as a
top-level `execution`/`runtime` document tree unless implementation work later
proves that is necessary.

## Concrete Phase 1 Template Schema

Phase 1 should keep the schema intentionally small.

Recommended top-level fields:

- `api_version`
- `kind`
- `template`
- `inputs`
- `defaults`
- `compile`

### `template`

Required fields:

- `id`
- `name`
- `execution_kind`
- `description`

Allowed `execution_kind` values in phase 1:

- `single_agent`
- `warm_agent`

`team` should be reserved for phase 2.

### `inputs`

Phase 1 should support these input field types only:

- `string`
- `enum`
- `integer`
- `number`
- `boolean`

Each input field supports:

- `type`
- `required`
- `description`
- `default`

Additional rules by type:

- `enum`
  - must define `values`
- `integer`
  - may define `min` and `max`
- `number`
  - may define `min` and `max`

Phase 1 should not support:

- nested object inputs
- arrays
- conditional sections
- arbitrary JSON schema

Those can be added later if real templates need them.

### `defaults`

`defaults` should compile directly into a baseline `ExecutionSpec`.

Required phase 1 fields:

- `workflow_template`
- `execution_spec`

`execution_spec` should already be structurally valid as a normal
`ExecutionSpec` after defaults are applied.

That means the simplest implementation path is:

1. load baseline `ExecutionSpec` from template defaults
2. apply template input bindings
3. validate the resulting normal `ExecutionSpec`

### `compile`

`compile` should contain explicit bindings from user inputs into the current
execution-spec model.

Recommended shape:

```yaml
compile:
  bindings:
    - input: goal
      target: execution_spec.goal
    - input: provider
      target: variation.explicit[0].overrides.llm.provider
    - input: prompt
      target: variation.explicit[0].overrides.agent.prompt
```

This list form is preferable to an object map because it is easier to extend
later with transforms or validation annotations.

## Binding Rules

Phase 1 bindings should support only a very small target vocabulary.

Allowed target roots:

- `execution_spec.goal`
- `execution_spec.workflow.template`
- `execution_spec.policy...`
- `execution_spec.evaluation...`
- `execution_spec.variation...`
- `execution_spec.swarm`
- `execution_spec.supervision...`
- `variation.explicit[0].overrides.<key>`

The last target form is special:

- it writes into the first explicit proposal override map
- `<key>` is the literal override key that existing runtime patching already
  understands, for example:
  - `llm.provider`
  - `agent.prompt`
  - `sandbox.env.TRANSFORM_ROLE`

Phase 1 should enforce:

- exactly one explicit variation proposal for `single_agent` templates
- exactly one explicit variation proposal for `warm_agent` templates

That keeps compilation deterministic and avoids inventing a new override layer.

## Compilation Algorithm

Recommended implementation algorithm:

1. Load template file.
2. Validate template file schema.
3. Validate user `inputs`.
4. Materialize baseline `ExecutionSpec` from `defaults.execution_spec`.
5. Force `workflow.template` from `defaults.workflow_template`.
6. Apply `compile.bindings` in order.
7. Validate the final `ExecutionSpec` using the existing execution-spec
   validator.
8. Return:
   - compiled `ExecutionSpec`
   - template metadata
   - normalized inputs

Phase 1 should fail fast if:

- a binding target is unsupported
- the explicit proposal shape needed for overrides is missing
- an input value cannot be coerced to the target type

## `execution_kind` Semantics

`execution_kind` should influence compilation defaults and response shaping, but
not require a new persistence model in phase 1.

### `single_agent`

Expected shape:

- `mode: swarm`
- `swarm: true`
- `variation.source: explicit`
- `candidates_per_iteration: 1`
- `max_iterations: 1`

This is intentionally implemented as a degenerate one-candidate execution using
the current model.

### `warm_agent`

Expected shape:

- same one-candidate execution structure as `single_agent`
- backing runtime template must point to `agent.mode: service`
- bridge/result shaping should make it obvious that this is a long-running
  service-style execution

Phase 1 still creates a normal `Execution`, but its underlying runtime behavior
is backed by a service-mode run.

## Example Phase 1 Templates

These examples are intentionally concrete and compile against the current
execution model.

### Example: `single-agent-basic`

```yaml
api_version: v1
kind: control_template

template:
  id: single-agent-basic
  name: Single Agent
  execution_kind: single_agent
  description: Run one agent once and return the result.

inputs:
  goal:
    type: string
    required: true
    description: High-level goal shown in the execution record.
  prompt:
    type: string
    required: true
    description: Prompt passed to the agent.
  provider:
    type: enum
    required: false
    default: claude
    values: [claude, codex]
    description: LLM provider override.

defaults:
  workflow_template: examples/runtime-templates/claude_mcp_diagnostic_agent.yaml
  execution_spec:
    mode: swarm
    goal: Single agent task
    workflow:
      template: ""
    policy:
      budget:
        max_iterations: 1
        max_child_runs: 1
        max_wall_clock_secs: 900
        max_cost_usd_millis: null
      concurrency:
        max_concurrent_candidates: 1
      convergence:
        strategy: threshold
        min_score: 1.0
        max_iterations_without_improvement: 0
      max_candidate_failures_per_iteration: 1
      missing_output_policy: mark_failed
      iteration_failure_policy: fail_execution
    evaluation:
      scoring_type: weighted_metrics
      weights:
        success: 1.0
      pass_threshold: 1.0
      ranking: highest_score
      tie_breaking: success
    variation:
      source: explicit
      candidates_per_iteration: 1
      explicit:
        - overrides: {}
    swarm: true
    supervision: null

compile:
  bindings:
    - input: goal
      target: execution_spec.goal
    - input: prompt
      target: variation.explicit[0].overrides.agent.prompt
    - input: provider
      target: variation.explicit[0].overrides.llm.provider
```

### Example: `warm-agent-basic`

```yaml
api_version: v1
kind: control_template

template:
  id: warm-agent-basic
  name: Warm Agent
  execution_kind: warm_agent
  description: Start one long-running service-mode agent.

inputs:
  goal:
    type: string
    required: true
    description: High-level goal shown in the execution record.
  prompt:
    type: string
    required: true
    description: Prompt passed to the service agent.
  provider:
    type: enum
    required: false
    default: claude
    values: [claude, codex]
    description: LLM provider override.

defaults:
  workflow_template: examples/runtime-templates/warm_agent_basic.yaml
  execution_spec:
    mode: swarm
    goal: Warm agent task
    workflow:
      template: ""
    policy:
      budget:
        max_iterations: 1
        max_child_runs: 1
        max_wall_clock_secs: 3600
        max_cost_usd_millis: null
      concurrency:
        max_concurrent_candidates: 1
      convergence:
        strategy: threshold
        min_score: 1.0
        max_iterations_without_improvement: 0
      max_candidate_failures_per_iteration: 1
      missing_output_policy: mark_failed
      iteration_failure_policy: fail_execution
    evaluation:
      scoring_type: weighted_metrics
      weights:
        success: 1.0
      pass_threshold: 1.0
      ranking: highest_score
      tie_breaking: success
    variation:
      source: explicit
      candidates_per_iteration: 1
      explicit:
        - overrides: {}
    swarm: true
    supervision: null

compile:
  bindings:
    - input: goal
      target: execution_spec.goal
    - input: prompt
      target: variation.explicit[0].overrides.agent.prompt
    - input: provider
      target: variation.explicit[0].overrides.llm.provider
```

## Dry Run and Execute Response Shape

### `GET /v1/templates`

Should return a compact listing:

```json
{
  "templates": [
    {
      "id": "single-agent-basic",
      "name": "Single Agent",
      "execution_kind": "single_agent",
      "description": "Run one agent once and return the result."
    },
    {
      "id": "warm-agent-basic",
      "name": "Warm Agent",
      "execution_kind": "warm_agent",
      "description": "Start one long-running service-mode agent."
    }
  ]
}
```

### `GET /v1/templates/{id}`

Should return:

- template metadata
- input schema
- defaults that are safe to expose
- optionally a redacted compile summary

### `POST /v1/templates/{id}/dry-run`

Should return a compiled execution preview without creating an execution:

```json
{
  "template": {
    "id": "single-agent-basic",
    "execution_kind": "single_agent"
  },
  "inputs": {
    "goal": "Summarize this repo",
    "prompt": "Read the repo and summarize risks",
    "provider": "claude"
  },
  "compiled": {
    "goal": "Summarize this repo",
    "workflow_template": "examples/runtime-templates/claude_mcp_diagnostic_agent.yaml",
    "mode": "swarm",
    "variation_source": "explicit",
    "candidates_per_iteration": 1,
    "overrides": {
      "agent.prompt": "Read the repo and summarize risks",
      "llm.provider": "claude"
    }
  }
}
```

### `POST /v1/templates/{id}/execute`

Should create a normal `Execution` and return an execution-centric response:

```json
{
  "execution_id": "exec_123",
  "template": {
    "id": "single-agent-basic",
    "execution_kind": "single_agent"
  },
  "status": "pending",
  "goal": "Summarize this repo"
}
```

## Module and File Mapping

Recommended first implementation layout:

- `src/templates/mod.rs`
  - template types and loader
- `src/templates/schema.rs`
  - input field validation
- `src/templates/compile.rs`
  - compile template + inputs into normal `ExecutionSpec`
- `src/templates/api.rs`
  - bridge-facing request/response helpers if useful
- `templates/`
  - checked-in template files

Minimal bridge changes:

- add list/get template routes
- add template dry-run route
- add template execute route that internally compiles to a normal
  `ExecutionSpec` and reuses existing execution-creation machinery

## API Direction

Make the API template-first.

First endpoints:

- `GET /v1/templates`
- `GET /v1/templates/{id}`
- `POST /v1/templates/{id}/dry-run`
- `POST /v1/templates/{id}/execute`

Execution request shape:

```json
{
  "inputs": {
    "goal": "Help with repo tasks",
    "prompt": "Review this codebase and summarize risks",
    "provider": "claude"
  }
}
```

The response should include:

- resolved template metadata
- validated inputs
- compiled execution summary
- resulting `execution_id`

Phase 1 should remain execution-centric at the API level:

- template endpoints compile into and create normal `Execution`s
- `POST /v1/templates/{id}/execute` should return the same execution-oriented
  identifiers and status model used elsewhere in `void-control`
- service-like behavior is a template/runtime concern in phase 1, not a distinct
  persisted API resource

## Compilation Model

The template layer should compile user inputs into the current lower-level
control-plane representation.

Phase 1 compilation targets:

- one-shot execution behavior over the current `ExecutionSpec`
- service-mode execution behavior over the current `ExecutionSpec`

Phase 1 should not require a new `void-box` session-exec primitive.

The compilation layer in `void-control` should:

- load the template file
- validate `inputs`
- apply defaults
- compile into:
  - `ExecutionSpec.goal`
  - `ExecutionSpec.workflow.template`
  - `ExecutionSpec.policy`
  - `ExecutionSpec.evaluation`
  - `ExecutionSpec.variation`
  - `ExecutionSpec.swarm`
  - optional `ExecutionSpec.supervision`
  - runtime override values embedded into variation proposals or equivalent
    existing override plumbing
- preserve a compiled artifact for debug/replay

## Relationship to Existing Execution Specs

The existing raw execution spec should remain supported.

Positioning:

- template API: default path for end users
- raw execution spec: advanced mode for expert users and tests

This keeps backward compatibility while letting the UI and SDKs adopt the
simpler template surface first.

## First Starter Templates

Recommended first template set:

- `single-agent-basic`
  - one-shot prompt/task, return response
- `warm-agent-basic`
  - long-running service-mode agent

Recommended second wave:

- `team-basic`
- `team-review`
- `benchmark-runner-python`
- `benchmark-runner-node`
- `code-review-team`

## Why Template-First

This design is preferred over prompt-first as the primary API because it:

- reduces end-user decision fatigue
- hides orchestration/runtime complexity
- produces repeatable and benchmarkable behavior
- makes UI authoring easier
- gives Python/JS SDKs a cleaner contract

Prompt-first should still be supported, but as an input inside a template, not
as the only product surface.

## Python/JS API Implication

Primary SDKs should live over `void-control`, not `void-box`.

The SDKs should expose product concepts such as:

- list templates
- execute template
- create warm service from template
- inspect execution/service

They should not require end users to construct low-level orchestration specs or
know `void-box` runtime details directly.

## Incremental Rollout

### Phase 1

- file-backed templates
- `GET /v1/templates`
- `GET /v1/templates/{id}`
- `POST /v1/templates/{id}/dry-run`
- `POST /v1/templates/{id}/execute`
- support `single-agent-basic` and `warm-agent-basic`

### Phase 2

- `team-basic`
- compiled artifact persistence and better inspection
- UI wizard over template inputs

### Phase 3

- saved user-defined templates
- warm service pools / prewarmed groups
- benchmark-oriented templates and SDK helpers

## Key Design Constraint

This design intentionally compiles to the runtime behavior that exists today:

- `void-box` run
- `void-box` service-mode run

And to the `void-control` persistence model that exists today:

- `ExecutionSpec`
- `Execution`
- existing bridge inspection/result APIs

It avoids inventing a reusable session execution model until the runtime layer
actually exposes one as a first-class daemon contract.

## Recommendation

Proceed with a file-backed template system in `void-control` as the next API
layer. Keep raw execution specs as advanced mode. Build the first template
compiler over existing one-shot and service-mode `void-box` runs.
