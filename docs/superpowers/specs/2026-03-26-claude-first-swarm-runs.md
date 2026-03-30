# Claude-First Swarm Runs

## Date: 2026-03-26

## Problem

`void-control` can run swarm iterations against production `void-box`
instances, and `void-box` now provides:

- sidecar transport
- injected messaging skill
- in-guest `void-message` CLI

But live swarm runs with Claude-backed agents still do not behave as
messaging-aware swarms. Execution succeeds, yet the agent frequently emits
zero intents, and inbox snapshots remain empty.

This document defines the correct model for **Claude-first swarm runs**:

- swarm execution should work immediately with Claude
- runtime collaboration should be reliable, not prompt-fragile
- Claude-specific optimizations must preserve the canonical sidecar semantics

## Scope

This spec is about **Claude-backed swarm runs first**.

It does not redefine the generic transport model or remove the generic
`void-message` path for non-Claude agents.

## Live Findings

Production swarm runs established the following:

1. Transport is not the blocker.
   - `void-box` starts the sidecar
   - `void-box` injects the `void-messaging` skill
   - `void-box` provisions `void-message`
   - `void-control` launches, inspects, and drains correctly

2. Prompt-only collaboration is unreliable for Claude.
   - asking Claude to read a skill file and call raw HTTP triggered refusal
   - switching to `void-message` CLI improved the model shape, but still did
     not reliably produce intents
   - live runs still showed `buffered_intents=0`

3. Swarm execution and swarm messaging are different milestones.
   - candidate execution works
   - evaluation and scoring work
   - messaging-aware swarm is still not achieved

## Architectural Position

### 1. Swarm Is a Coordination Strategy

For Claude-first runs, `swarm` remains a coordination strategy implemented by
`void-control`, not a provider/runtime primitive.

`void-control` owns:

- iteration boundaries
- candidate generation
- leader/broadcast routing
- signal extraction
- planning bias

`void-box` owns:

- execution transport
- sidecar
- guest runtime integration
- provider-specific bridges

### 2. Canonical Semantics Stay in the Sidecar

Even for Claude-first runs, the sidecar remains authoritative for:

- context
- inbox
- intent submission
- transport-level observability

Claude-specific integrations must map onto the sidecar intent model. They must
not invent a second collaboration protocol.

## Claude-First Design

### 1. Generic Path vs Claude Path

Two paths must coexist:

#### Generic Path

For arbitrary OCI agents:

- sidecar
- `void-message` CLI
- `void-messaging` skill

This path remains the universal fallback.

#### Claude Path

For Claude-backed swarm runs:

- sidecar
- provider bridge
- first-class Claude-callable tools
- optional channel push for inbound delivery

The Claude path is preferred for reliability.

### 2. Claude Must Not Rely on Skill Compliance Alone

For Claude-backed swarm runs, it is insufficient to rely on:

- “read this skill”
- “run this CLI”
- “please remember to send `leader` and `broadcast` intents”

That model is advisory, and live evidence shows it is too weak.

Therefore, Claude-first swarm runs MUST expose collaboration through a
first-class bridge surface.

### 3. Required Claude Tools

Claude-first runs should expose the following provider bridge tools:

- `get_context()`
- `read_inbox(since?)`
- `send_message(kind, audience, summary_text, priority?)`

The implementation may use:

- direct sidecar HTTP
- or `void-message` internally

But that is hidden from Claude.

Claude should interact with collaboration as a tool capability, not as shell
or file-reading instructions.

### 4. Optional Channel Push

If Claude channels are available, `void-box` MAY use them to push inbound
messages into an already-running session.

This is an optimization layer for inbox delivery.

Rules:

- channel push is optional
- sidecar remains the canonical transport
- channel-delivered messages must correspond to the same inbox semantics as
  sidecar-delivered messages

### 5. Pull vs Channel Policy

Claude-first runs should distinguish between **canonical correctness** and
**delivery optimization**:

- tools + pull provide the canonical base path
- channels provide optional low-latency push delivery

#### Pull Is the Default

Claude-first runs SHOULD use pull by default when:

- execution is batch or iteration-bounded
- inbox consumption only needs to happen at iteration boundaries
- deterministic replay is more important than low-latency interaction
- the same run shape must also work for non-Claude agents

In these cases, the required tools are sufficient:

- `get_context()`
- `read_inbox(since?)`
- `send_message(...)`

#### Channels Are an Optimization

Claude-first runs MAY enable channels when:

- the Claude session remains alive for a meaningful duration
- new messages should arrive while the same run is still active
- collaboration latency matters
- polling would be too delayed or too awkward

Channels therefore improve delivery UX, but do not replace the tool/pull path.

#### Policy by Coordination Strategy

##### Swarm

For swarm coordination:

- pull SHOULD be the baseline
- channels MAY be added later for long-running/live swarm interaction

Reason:

- swarm tolerates delayed communication better
- much of swarm planning still happens at iteration boundaries

##### Supervision

For supervision coordination:

- pull is sufficient for batch supervision
- channels are strongly preferred for live supervision

Reason:

- supervision is more centralized and time-sensitive
- supervisor guidance may need to reach a running worker before the next
  iteration boundary

So if one coordination strategy gets channel support first, it SHOULD be
supervision before swarm.

### 6. `void-message` Still Matters

`void-message` is not removed.

It remains:

- the universal guest-facing interface
- the shared fallback for non-Claude agents
- a possible implementation detail under Claude-specific tools

So the correct layering is:

```text
sidecar semantics
  ├── generic agents -> void-message CLI
  └── Claude agents -> provider bridge tools/channels
```

## Why the Current Model Fails

The observed Claude failures are expected under the current shape:

- skills are prompt-based playbooks, not hard capabilities
- “read a hidden file and call external URLs” resembles prompt injection
- even a CLI remains optional if only requested by prompt text

Therefore:

- skill-only collaboration is acceptable for generic agents
- it is not sufficient as the primary Claude swarm path

## Swarm Run Requirements

A Claude-first swarm run is conformant only if all of the following hold:

1. Candidate execution succeeds under production `void-box`
2. Claude has explicit collaboration tools or equivalent bridge surface
3. At least one candidate can:
   - inspect context
   - inspect inbox
   - emit canonical intents
4. `void-control` drains and routes those intents
5. later candidates can observe materialized inbox entries or equivalent pushed
   delivery

If execution completes but no intents are ever emitted, the run is only an
evaluation swarm, not a messaging-aware swarm.

## Supervision Note

This document is Claude-first and swarm-first, but the same bridge model also
supports future supervision strategies.

The main difference is delivery urgency:

- swarm can often remain pull-first
- supervision becomes channel-favored earlier because live guidance is more
  valuable there

This does not change the canonical semantic model:

- sidecar remains authoritative
- tools remain the primary Claude interaction surface
- channels remain an optional delivery optimization layer

## Provider Compatibility Model

### Claude

Preferred integration:

- first-class collaboration tools
- optional channel push
- sidecar-backed transport

### Codex

Likely acceptable integration:

- `void-message` CLI as a first-class command/tool

### `pi` / `pi-mono`

Likely acceptable integration:

- `void-message` CLI
- or a lightweight wrapper tool

### OpenClaw

Likely acceptable integration:

- `void-message` CLI as universal fallback
- optional native OpenClaw bridge later

## Required Follow-Up Work

### `void-box`

- add a Claude collaboration bridge that exposes messaging as tools
- optionally add channel push for inbound inbox updates
- keep `void-message` as the generic path
- preserve sidecar as canonical semantics

### `void-control`

- treat Claude-first swarm as bridge-preferred, not skill-preferred
- keep swarm coordination semantics unchanged
- verify message-aware swarm runs against the bridge path

## Acceptance Criteria

A Claude-first swarm run is accepted when:

- a production `void-box` run injects the collaboration bridge
- Claude uses collaboration through first-class tools
- sidecar buffers non-zero intents
- `void-control` drains and routes them
- later candidates observe non-empty collaboration state
- swarm planning can react to real communication, not only metrics
