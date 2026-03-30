# Void MCP Action Tools

## Date: 2026-03-26

## Problem

`void-box` now provisions a working Claude-facing MCP bridge:

- `mcp.json` is written
- `void-mcp` is present in the guest image
- Claude-backed runs can start successfully

But live Claude swarm runs still do not use the MCP collaboration surface.
Claude completes the task using generic tools like `Bash`, `Read`, and `Write`,
while the sidecar ends with `buffered_intents=0`.

The current `void-mcp` tool surface is technically correct but too
transport-oriented:

- `get_context`
- `read_inbox`
- `send_message`

This is weaker than the action-oriented swarm tools used by systems like
`agent-swarm`, where the tool names read like work actions rather than bridge
plumbing.

## Decision

Replace the current `void-mcp` tool surface with action-oriented collaboration
tools.

No backward-compatibility layer is required. Nothing external should depend on
the current tool names.

## Goals

- make the MCP collaboration surface legible to Claude as work actions
- preserve the same canonical sidecar semantics underneath
- prove actual MCP invocation in `void-box` end-to-end tests

## Non-Goals

- changing `void-control` routing semantics
- changing sidecar HTTP endpoints
- changing the generic `void-message` CLI contract
- introducing provider-specific message semantics beyond Claude-facing UX

## Design

### 1. Replace Transport Verbs with Task Verbs

`void-mcp` should expose these tools:

- `read_shared_context`
- `read_peer_messages`
- `broadcast_observation`
- `recommend_to_leader`

These replace:

- `get_context`
- `read_inbox`
- `send_message`

Reason:

- `read_shared_context` is clearer than `get_context`
- `read_peer_messages` is clearer than `read_inbox`
- `broadcast_observation` expresses the main swarm-sharing action directly
- `recommend_to_leader` expresses leader-directed coordination directly

The tool names should read like collaboration work, not sidecar transport.

### 2. Keep Canonical Sidecar Semantics

The new tool names are only a Claude-facing presentation layer.

Under the hood:

- `read_shared_context` maps to `GET /v1/context`
- `read_peer_messages` maps to `GET /v1/inbox`
- `broadcast_observation` maps to `POST /v1/intents`
  - `audience = "broadcast"`
  - `kind = "signal"`
- `recommend_to_leader` maps to `POST /v1/intents`
  - `audience = "leader"`
  - `kind = "proposal"` or `kind = "evaluation"`

The sidecar remains authoritative. Only the Claude-facing tool names change.

### 3. Tool Contracts

#### `read_shared_context()`

Returns the current execution context for the candidate.

Expected content:

- execution identity
- candidate identity
- role
- iteration metadata

#### `read_peer_messages(since?)`

Returns peer-visible inbox content.

Arguments:

- optional `since`

Result:

- same inbox snapshot semantics as the sidecar API

#### `broadcast_observation(summary_text, priority?)`

Sends a concise cross-peer observation.

Arguments:

- `summary_text`
- optional `priority`

Fixed semantics:

- `kind = "signal"`
- `audience = "broadcast"`

#### `recommend_to_leader(summary_text, disposition?, priority?)`

Sends a concise leader-directed recommendation.

Arguments:

- `summary_text`
- optional `disposition`
  - `promote`
  - `refine`
  - `reject`
- optional `priority`

Fixed audience:

- `audience = "leader"`

Kind mapping:

- default to `proposal`
- `evaluation` may be used if the implementation wants to distinguish
  incumbent judgment from proposal shaping

### 4. Descriptions Must Be Claude-Oriented

Tool descriptions should explicitly describe when to use the tool, not just
what endpoint it calls.

Examples:

- `read_shared_context`
  - "Read the shared execution context for this candidate before evaluating the assigned role."
- `read_peer_messages`
  - "Read observations already shared by sibling candidates."
- `broadcast_observation`
  - "Share a concise finding that could help sibling candidates refine or compare their work."
- `recommend_to_leader`
  - "Send a short recommendation to the leader about whether this candidate should be promoted, refined, or rejected."

### 5. Remove Compatibility Aliases

The old names must be removed rather than kept as aliases.

Reason:

- nothing should depend on them
- keeping both surfaces weakens tool salience
- a smaller action-oriented tool set is easier for Claude to choose from

### 6. Strengthen the Claude E2E

The current `void-box` e2e around `void-mcp` discovery is too weak because it
proves discovery but not real use.

Replace or upgrade that test so it proves at least one of:

- Claude telemetry includes actual `mcp__void-mcp__...` tool invocation
- the sidecar buffers at least one intent from a Claude-backed run

Preferred assertion:

- sidecar drain returns one or more intents after the run

That is stronger because it proves the bridge produced canonical collaboration
state rather than only listing tools.

### 7. Example Prompt Guidance

Claude-first swarm examples should refer to collaboration in work terms:

- review shared context
- review peer observations
- share one reusable observation
- send one leader recommendation

They should not refer to:

- sidecar
- MCP config
- hidden skills
- raw protocol steps

## Acceptance Criteria

- `void-mcp` exposes only the new action-oriented tool names
- Claude-backed runs can invoke the tools from the rebuilt production image
- at least one Claude-backed e2e proves canonical sidecar intent emission
- swarm reruns from `void-control` show non-zero collaboration activity

## Follow-Up

After this lands in `void-box`:

1. rebuild the production guest image
2. rerun the Claude-first swarm example from `void-control`
3. confirm:
   - non-zero buffered intents
   - non-empty later inboxes or routed message state
   - visible MCP-backed collaboration behavior
