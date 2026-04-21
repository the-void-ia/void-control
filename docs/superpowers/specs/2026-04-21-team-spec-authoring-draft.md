# Team Spec Authoring Draft

## Date

2026-04-21

## Status

Draft

## Problem

`void-control` currently asks users to author low-level orchestration concepts
directly:

- `mode`
- `policy`
- `evaluation`
- `variation`
- `swarm`
- `supervision`

That is too much control-plane surface area for common multi-agent use cases.

External systems show a more usable pattern:

- CrewAI
  - `Agent`, `Task`, `Crew`
- OpenAI Swarm
  - agents, messages, handoffs
- LangGraph
  - explicit workflow graph
- agent-swarm
  - lead/worker agents, task lifecycle, advanced workflow engine

The common lesson is:

- simple authoring concepts first
- orchestration strategy second
- low-level execution details hidden by default

## Design Principle

The current `swarm` and `supervision` models should remain execution
primitives, not primary authoring concepts.

Users should author:

- `AgentSpec`
- `TaskSpec`
- `TeamSpec`

`void-control` should compile that into the current orchestration engine.

That means:

- the simple authoring model becomes the public product surface
- `ExecutionSpec` becomes an advanced/internal representation
- `swarm` and `supervision` remain part of the runtime planner, but are usually
  selected by compilation rules rather than written directly by users

## Primary Objects

### `AgentSpec`

Represents one named agent role.

Suggested shape:

```yaml
name: researcher
role: Researcher
goal: Find information about Rust performance
template: single-agent-basic
tools:
  - docs_search
  - web_search
runtime:
  provider: claude
```

Minimal required fields:

- `name`
- `role`
- `goal`

Optional fields:

- `template`
- `tools`
- `runtime`
- `memory`
- `sandbox`

### `TaskSpec`

Represents a unit of work for one or more agents.

Suggested shape:

```yaml
name: write_blog_post
description: Write a blog post about Rust performance tradeoffs
agent: researcher
inputs:
  topic: Rust performance
depends_on: []
expected_output: article_draft
```

Minimal required fields:

- `name`
- `description`

Optional fields:

- `agent`
- `inputs`
- `depends_on`
- `expected_output`
- `reviewed_by`

### `TeamSpec`

Represents a group of agents plus tasks and a collaboration process.

Suggested shape:

```yaml
api_version: v1
kind: team

metadata:
  name: rust-article-team

agents:
  - name: researcher
    role: Researcher
    goal: Find information about Rust performance
  - name: writer
    role: Writer
    goal: Write a high-quality article

tasks:
  - name: research
    description: Gather evidence about Rust performance tradeoffs
    agent: researcher
  - name: write
    description: Write the article using the research findings
    agent: writer
    depends_on: [research]

process:
  type: sequential

inputs:
  topic: Rust performance
```

## Authoring Modes

Recommended user-facing authoring modes:

### `agent`

Single-agent execution.

Maps to:

- a degenerate single-candidate `swarm` execution in phase 1

### `batch` / `yolo`

Remote background execution with one or more offloaded workers.

Intended meaning:

- low-friction remote execution
- user can keep working locally while remote work continues
- little or no inter-agent coordination
- optimized for background throughput rather than collaboration

Maps to:

- a simple parallel `swarm`-style execution plan
- minimal or no message passing
- simple result collection

Canonical term:

- `batch`

Accepted alias:

- `yolo`

Normalization rule:

- if the user authors `kind: yolo`, the parser normalizes it to `batch`

### `team`

Multiple agents plus tasks and a collaboration process.

Maps to:

- `swarm`
- or `supervision`

depending on `process.type`

### `workflow`

Explicit graph / DAG mode.

Maps to:

- a future graph-capable internal plan

This should be added later as an advanced mode.

## `BatchSpec` (`YoloSpec` alias)

`BatchSpec` is intentionally different from `TeamSpec`.

It does not model rich collaboration. It models offloaded background work.

Suggested shape:

```yaml
api_version: v1
kind: batch

metadata:
  name: repo-background-work

worker:
  template: coder-agent
  provider: claude

mode:
  parallelism: 4
  background: true
  interaction: none

jobs:
  - name: fix-auth-tests
    prompt: Fix failing auth tests
  - name: improve-logging
    prompt: Improve logging around retries
  - name: review-db-migrations
    prompt: Review migration safety
```

Minimal required fields:

- `worker`
- `jobs`

Optional fields:

- `mode.parallelism`
- `mode.background`
- `mode.interaction`
- `metadata`

## `TeamSpec.process`

This is the key field that determines how the simple model compiles into the
current orchestration primitives.

Recommended phase 1 values:

- `sequential`
- `parallel`
- `lead_worker`

Later values:

- `handoff`
- `graph`

## Compile Rules

### `process.type = sequential`

Intended user meaning:

- run tasks in order
- later tasks depend on earlier outputs
- collaboration is structured, not emergent

Compile shape:

- execution primitive:
  - constrained `swarm`
- defaults:
  - one active task stage at a time
  - explicit candidate/task mapping
  - conservative failure policy
  - simple success-oriented evaluation

Why not `supervision` by default:

- sequential task pipelines do not necessarily require a supervising reviewer
- they are often just ordered execution

### `process.type = parallel`

Intended user meaning:

- multiple agents work at the same time
- same or related tasks may be compared or merged
- result selection/reduction matters

Compile shape:

- execution primitive:
  - `swarm`
- defaults:
  - explicit multi-candidate variation
  - parallel candidate dispatch
  - evaluation/reduction enabled

This is the most direct mapping to the current `swarm` primitive.

### `process.type = lead_worker`

Intended user meaning:

- one lead agent plans or reviews
- worker agents execute subtasks
- lead synthesizes or approves the result

Compile shape:

- execution primitive:
  - `supervision`
- defaults:
  - supervisor role from one designated agent
  - worker roles mapped from remaining agents
  - review policy enabled

This is the most direct mapping to the current `supervision` primitive.

## Suggested Compile Table

| `TeamSpec.process.type` | Internal primitive | Main behavior |
|---|---|---|
| `sequential` | constrained `swarm` | ordered task execution |
| `parallel` | `swarm` | concurrent candidate/team execution |
| `lead_worker` | `supervision` | lead plans/reviews, workers execute |

Later:

| `TeamSpec.process.type` | Internal primitive | Main behavior |
|---|---|---|
| `handoff` | swarm-like planner | emergent agent handoffs |
| `graph` | workflow plan | explicit node/edge execution |

## Relationship to Existing ExecutionSpec

The simple authoring model should compile into the existing engine.

That means:

- users usually never write raw `mode`, `policy`, `evaluation`, or `variation`
- `void-control` compiler produces those fields
- raw `ExecutionSpec` remains available as:
  - advanced mode
  - debugging output
  - expert/operator escape hatch

Recommended output flow:

1. user writes `TeamSpec`
2. compiler selects primitive and fills defaults
3. compiler emits normal `ExecutionSpec`
4. execution service runs that spec unchanged

## Example Compilation: `parallel`

User input:

```yaml
api_version: v1
kind: team

agents:
  - name: researcher_a
    role: Researcher
    goal: Find strong evidence
  - name: researcher_b
    role: Researcher
    goal: Find contrarian evidence
  - name: writer
    role: Writer
    goal: Produce final article

tasks:
  - name: investigate
    description: Research Rust performance tradeoffs

process:
  type: parallel
```

Compiler output intent:

- `mode: swarm`
- explicit variation entries for parallel agent/task strategies
- evaluation enabled to compare outputs
- result reduction picks the best candidate or best merged outcome

## Example Compilation: `lead_worker`

User input:

```yaml
api_version: v1
kind: team

agents:
  - name: lead
    role: Lead Editor
    goal: Coordinate and approve
  - name: researcher
    role: Researcher
    goal: Gather evidence
  - name: writer
    role: Writer
    goal: Draft the article

tasks:
  - name: article
    description: Produce final article on Rust performance

process:
  type: lead_worker
  lead: lead
```

Compiler output intent:

- `mode: supervision`
- `supervision.supervisor_role = "Lead Editor"`
- worker roles mapped to candidate runs
- review policy defaults enabled

## API Examples

The simple authoring model should be reflected directly in the SDKs.

### Python

```python
from void_control import VoidControlClient

team = {
    "api_version": "v1",
    "kind": "team",
    "agents": [
        {
            "name": "researcher",
            "role": "Researcher",
            "goal": "Find information about Rust performance",
        },
        {
            "name": "writer",
            "role": "Writer",
            "goal": "Write a clear final article",
        },
    ],
    "tasks": [
        {
            "name": "research",
            "description": "Gather evidence about Rust performance tradeoffs",
            "agent": "researcher",
        },
        {
            "name": "write",
            "description": "Write the article from the research findings",
            "agent": "writer",
            "depends_on": ["research"],
        },
    ],
    "process": {
        "type": "sequential",
    },
}

client = VoidControlClient(base_url="http://127.0.0.1:43210")
run = await client.teams.run(team)
result = await client.team_runs.wait(run.run_id)
print(result)
```

#### Python `batch`

```python
from void_control import VoidControlClient

batch = {
    "api_version": "v1",
    "kind": "batch",
    "worker": {
        "template": "coder-agent",
        "provider": "claude",
    },
    "mode": {
        "parallelism": 3,
        "background": True,
        "interaction": "none",
    },
    "jobs": [
        {"name": "auth", "prompt": "Fix failing auth tests"},
        {"name": "logging", "prompt": "Improve retry logging"},
        {"name": "migrations", "prompt": "Review DB migration safety"},
    ],
}

client = VoidControlClient(base_url="http://127.0.0.1:43210")
run = await client.batch.run(batch)
result = await client.batch_runs.wait(run.run_id)
print(result)
```

Alias form should also be accepted:

```python
run = await client.yolo.run(batch)
```

### Node.js

```js
import { VoidControlClient } from "@the-void-ia/void-control";

const team = {
  api_version: "v1",
  kind: "team",
  agents: [
    {
      name: "researcher",
      role: "Researcher",
      goal: "Find information about Rust performance"
    },
    {
      name: "writer",
      role: "Writer",
      goal: "Write a clear final article"
    }
  ],
  tasks: [
    {
      name: "research",
      description: "Gather evidence about Rust performance tradeoffs",
      agent: "researcher"
    },
    {
      name: "write",
      description: "Write the article from the research findings",
      agent: "writer",
      depends_on: ["research"]
    }
  ],
  process: {
    type: "sequential"
  }
};

const client = new VoidControlClient({ baseUrl: "http://127.0.0.1:43210" });
const run = await client.teams.run(team);
const result = await client.teamRuns.wait(run.runId);
console.log(result);
```

#### Node.js `batch`

```js
import { VoidControlClient } from "@the-void-ia/void-control";

const batch = {
  api_version: "v1",
  kind: "batch",
  worker: {
    template: "coder-agent",
    provider: "claude"
  },
  mode: {
    parallelism: 3,
    background: true,
    interaction: "none"
  },
  jobs: [
    { name: "auth", prompt: "Fix failing auth tests" },
    { name: "logging", prompt: "Improve retry logging" },
    { name: "migrations", prompt: "Review DB migration safety" }
  ]
};

const client = new VoidControlClient({ baseUrl: "http://127.0.0.1:43210" });
const run = await client.batch.run(batch);
const result = await client.batchRuns.wait(run.runId);
console.log(result);
```

Alias form should also be accepted:

```js
const run = await client.yolo.run(batch);
```

### Go

```go
client := voidcontrol.NewClient("http://127.0.0.1:43210")

team := map[string]any{
	"api_version": "v1",
	"kind":        "team",
	"agents": []map[string]any{
		{
			"name": "researcher",
			"role": "Researcher",
			"goal": "Find information about Rust performance",
		},
		{
			"name": "writer",
			"role": "Writer",
			"goal": "Write a clear final article",
		},
	},
	"tasks": []map[string]any{
		{
			"name":        "research",
			"description": "Gather evidence about Rust performance tradeoffs",
			"agent":       "researcher",
		},
		{
			"name":        "write",
			"description": "Write the article from the research findings",
			"agent":       "writer",
			"depends_on":  []string{"research"},
		},
	},
	"process": map[string]any{
		"type": "sequential",
	},
}

run, _ := client.Teams.Run(team)
result, _ := client.TeamRuns.Wait(run.RunID)
fmt.Println(result)
```

#### Go `batch`

```go
batch := map[string]any{
	"api_version": "v1",
	"kind":        "batch",
	"worker": map[string]any{
		"template": "coder-agent",
		"provider": "claude",
	},
	"mode": map[string]any{
		"parallelism": 3,
		"background":  true,
		"interaction": "none",
	},
	"jobs": []map[string]any{
		{"name": "auth", "prompt": "Fix failing auth tests"},
		{"name": "logging", "prompt": "Improve retry logging"},
		{"name": "migrations", "prompt": "Review DB migration safety"},
	},
}

run, _ := client.Batch.Run(batch)
result, _ := client.BatchRuns.Wait(run.RunID)
fmt.Println(result)
```

Alias form should also be accepted:

```go
run, _ := client.Yolo.Run(batch)
```

## HTTP API Examples

Suggested simple routes:

- `POST /v1/teams/dry-run`
- `POST /v1/teams/run`
- `GET /v1/team-runs/{id}`
- `POST /v1/batch/run`
- `GET /v1/batch-runs/{id}`

Accepted route aliases:

- `POST /v1/yolo/run`
- `GET /v1/yolo-runs/{id}`

### Run Request

```json
{
  "api_version": "v1",
  "kind": "team",
  "agents": [
    {
      "name": "researcher",
      "role": "Researcher",
      "goal": "Find information about Rust performance"
    },
    {
      "name": "writer",
      "role": "Writer",
      "goal": "Write a clear final article"
    }
  ],
  "tasks": [
    {
      "name": "research",
      "description": "Gather evidence about Rust performance tradeoffs",
      "agent": "researcher"
    },
    {
      "name": "write",
      "description": "Write the article from the research findings",
      "agent": "writer",
      "depends_on": ["research"]
    }
  ],
  "process": {
    "type": "sequential"
  }
}
```

### Run Response

```json
{
  "run_id": "team-run-123",
  "status": "Pending",
  "compiled_primitive": "swarm"
}
```

### Inspect Response

```json
{
  "run_id": "team-run-123",
  "status": "Running",
  "compiled_primitive": "swarm",
  "execution_id": "exec-123",
  "progress": {
    "completed_tasks": 1,
    "total_tasks": 2
  }
}
```

### `batch` Run Request

```json
{
  "api_version": "v1",
  "kind": "batch",
  "worker": {
    "template": "coder-agent",
    "provider": "claude"
  },
  "mode": {
    "parallelism": 3,
    "background": true,
    "interaction": "none"
  },
  "jobs": [
    {
      "name": "auth",
      "prompt": "Fix failing auth tests"
    },
    {
      "name": "logging",
      "prompt": "Improve retry logging"
    },
    {
      "name": "migrations",
      "prompt": "Review DB migration safety"
    }
  ]
}
```

### `batch` Run Response

```json
{
  "run_id": "batch-run-123",
  "status": "Pending",
  "compiled_primitive": "swarm"
}
```

## Spec Examples

### YAML `TeamSpec`

```yaml
api_version: v1
kind: team

metadata:
  name: rust-article-team

agents:
  - name: researcher
    role: Researcher
    goal: Find information about Rust performance
  - name: writer
    role: Writer
    goal: Write a clear final article

tasks:
  - name: research
    description: Gather evidence about Rust performance tradeoffs
    agent: researcher
  - name: write
    description: Write the article from the research findings
    agent: writer
    depends_on: [research]

process:
  type: sequential
```

### JSON `TeamSpec`

```json
{
  "api_version": "v1",
  "kind": "team",
  "metadata": {
    "name": "rust-article-team"
  },
  "agents": [
    {
      "name": "researcher",
      "role": "Researcher",
      "goal": "Find information about Rust performance"
    },
    {
      "name": "writer",
      "role": "Writer",
      "goal": "Write a clear final article"
    }
  ],
  "tasks": [
    {
      "name": "research",
      "description": "Gather evidence about Rust performance tradeoffs",
      "agent": "researcher"
    },
    {
      "name": "write",
      "description": "Write the article from the research findings",
      "agent": "writer",
      "depends_on": ["research"]
    }
  ],
  "process": {
    "type": "sequential"
  }
}
```

### YAML `BatchSpec`

```yaml
api_version: v1
kind: batch

metadata:
  name: repo-background-work

worker:
  template: coder-agent
  provider: claude

mode:
  parallelism: 3
  background: true
  interaction: none

jobs:
  - name: auth
    prompt: Fix failing auth tests
  - name: logging
    prompt: Improve retry logging
  - name: migrations
    prompt: Review DB migration safety
```

### JSON `BatchSpec`

```json
{
  "api_version": "v1",
  "kind": "batch",
  "metadata": {
    "name": "repo-background-work"
  },
  "worker": {
    "template": "coder-agent",
    "provider": "claude"
  },
  "mode": {
    "parallelism": 3,
    "background": true,
    "interaction": "none"
  },
  "jobs": [
    {
      "name": "auth",
      "prompt": "Fix failing auth tests"
    },
    {
      "name": "logging",
      "prompt": "Improve retry logging"
    },
    {
      "name": "migrations",
      "prompt": "Review DB migration safety"
    }
  ]
}
```

## Advanced Mode

The current `ExecutionSpec` should remain supported as advanced mode.

That preserves:

- orchestration power
- debugging visibility
- backward compatibility

But it should no longer be the default way new users author agent teams.

## Recommended Rollout

### Phase 1

- define `AgentSpec`
- define `TaskSpec`
- define `TeamSpec`
- define `BatchSpec`
- support `sequential`, `parallel`, and `lead_worker`
- compile into existing `swarm` and `supervision` primitives

### Phase 2

- add richer task dependencies
- add handoff-style process mode
- add better result merge semantics

### Phase 3

- add `WorkflowSpec`
- add explicit graph authoring

## Recommendation

The current internal orchestration primitives should stay.

What should change is the authoring surface:

- `swarm` and `supervision` become engine-level concepts
- `TeamSpec` becomes the collaborative user-facing concept
- `BatchSpec` becomes the remote offload/background concept
- `yolo` remains an accepted alias and product term

That gives:

- better usability
- continuity with current engine investments
- a clean path toward teams, workflows, and compute APIs without exposing raw
  orchestration fields to every user
