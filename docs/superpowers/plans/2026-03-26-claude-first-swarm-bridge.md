# Claude-First Swarm Bridge Plan

## Goal

Make Claude-backed swarm runs reliably messaging-aware by exposing collaboration
as a first-class Claude bridge surface in `void-box`, while preserving the
sidecar as the canonical transport and `void-message` as the generic fallback.

This plan assumes:

- sidecar transport is already implemented in `void-box`
- `void-message` CLI already exists in `void-box`
- `void-control` already has `MessageDeliveryAdapter`, `HttpSidecarAdapter`,
  merge-and-dedup, and execution-service integration

The missing piece is reliable Claude agent interaction.

## Problem Statement

Live runs proved:

- production swarm execution works
- sidecar provisioning works
- skill injection works
- CLI provisioning works

But Claude still emits zero intents in many runs because prompt+skill guidance
is not a reliable collaboration surface.

We need:

- Claude collaboration as explicit callable capabilities
- sidecar semantics preserved underneath
- minimal change to `void-control` coordination semantics

## Architecture

### Canonical semantics

Remain unchanged:

- `GET /v1/context`
- `GET /v1/inbox`
- `POST /v1/intents`

### Claude bridge surface

`void-box` should expose Claude-callable collaboration tools:

- `get_context()`
- `read_inbox(since?)`
- `send_message(kind, audience, summary_text, priority?)`

Bridge implementation may internally use:

- direct sidecar HTTP
- or `void-message`

Claude does not see the transport details.

### Delivery policy

- tools + pull are required
- channels are optional follow-up
- swarm remains pull-first
- supervision may prefer channels earlier

## Work Split

### `void-box`

Owns:

- Claude bridge/tool surface
- tool-to-sidecar mapping
- optional channel integration
- runtime provisioning for Claude-backed runs

### `void-control`

Owns:

- swarm coordination semantics
- routing / inbox materialization
- signal extraction
- validation that real intents are emitted and routed

## Tasks

### Task 1: Define Claude collaboration tool contract in `void-box`

Add an internal contract for three Claude-facing capabilities:

- `get_context`
- `read_inbox`
- `send_message`

Requirements:

- maps one-to-one onto canonical sidecar semantics
- supports the existing message-box kinds/audiences
- rejects invalid calls before transport submission

Deliverable:

- documented bridge contract
- tests for argument validation

### Task 2: Implement bridge handlers over the sidecar

Implement tool handlers in `void-box` that:

- fetch context from sidecar
- fetch inbox from sidecar
- submit intents to sidecar

Implementation choice:

- either direct sidecar HTTP
- or `void-message` subprocess invocation

Recommendation:

- use direct internal calls where practical for lower overhead
- keep `void-message` as the generic guest path

Deliverable:

- bridge handlers returning typed results
- unit tests for successful mapping to sidecar semantics

### Task 3: Provision bridge only for Claude-backed runs

For Claude-backed agent runs:

- register collaboration tools with the Claude runtime
- keep skill injection optional or informational only
- do not require prompt text to discover the transport

For non-Claude runs:

- keep `void-message` + skill path unchanged

Deliverable:

- runtime gating by provider
- provider-specific provisioning tests

### Task 4: Keep `void-message` and skill as generic fallback

Do not remove:

- `void-message`
- `void-messaging` skill

Instead:

- rewrite the skill to describe the CLI clearly
- position it as the generic fallback path
- ensure Claude-first runs do not depend on it for correctness

Deliverable:

- updated skill text policy
- tests ensuring generic runs still work

### Task 5: Add live Claude bridge validation in `void-box`

Add integration coverage proving that a Claude-backed run can:

- access context
- read inbox
- send an intent

This should not rely on the prompt “remembering” to call a CLI.

Deliverable:

- ignored live daemon test or equivalent bridge integration test
- assertion that at least one intent is buffered

### Task 6: Add end-to-end validation in `void-control`

Using the existing adapter and message-box pipeline, add validation that:

- a real Claude-backed swarm candidate emits at least one intent
- `void-control` drains those intents
- routing produces non-empty inbox entries for later candidates

Deliverable:

- live ignored contract test in `void-control`
- execution fixture proving non-empty collaboration state

### Task 7: Validate a Claude-first swarm run end to end

Success criteria for the full system:

- swarm execution launches successfully
- at least one candidate emits canonical intents
- `void-control` drains and routes them
- a later candidate observes non-empty inbox state
- the run is no longer only an evaluation swarm

Deliverable:

- one successful documented swarm execution against production `void-box`

## Follow-Up (Not in this plan)

### Optional channels

Once tools + pull are working:

- add Claude channel push for inbound delivery
- prioritize supervision use cases before swarm

### Search validation

Only after Claude-first swarm is messaging-aware:

- run the search example
- compare search behavior with and without real collaboration signals

## Acceptance Criteria

This plan is complete when:

1. Claude-backed runs have first-class collaboration tools
2. sidecar remains the canonical transport
3. `void-message` remains the generic fallback
4. at least one live swarm run emits non-zero intents
5. `void-control` materializes non-empty collaboration state for later candidates
