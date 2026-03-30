# CLI Messaging Skill Design

## Date: 2026-03-25

## Problem

The current messaging skill is only documentation. It tells the agent to
manually call sidecar HTTP endpoints with `curl`. In practice, that is too
weak:

- agents may read the skill but never emit intents
- agents may format requests incorrectly
- prompts become noisy and repetitive
- provider-specific bridges such as Claude channels remain disconnected from
  the generic OCI path

We need a more reliable generic agent interface for runtime messaging.

## Goal

Replace "raw HTTP from prompt" with a provisioned in-guest CLI that the skill
documents and the agent can call directly.

The canonical agent-facing surface becomes:

```bash
void-message context
void-message inbox
void-message inbox --since 3
void-message send --kind signal --audience broadcast --summary "cache misses dominate p99"
void-message send --kind proposal --audience leader --summary "promote cache-aware variant"
```

## Non-Goals

- changing message-box semantics in `void-control`
- making Claude channels the canonical protocol
- requiring provider-specific runtimes for generic OCI agents
- introducing semantic signal extraction inside `void-box`

## Design

### 1. Canonical Transport Stays the Same

The sidecar HTTP API remains the canonical transport owned by `void-box`:

- `GET /v1/context`
- `GET /v1/inbox`
- `POST /v1/intents`
- `GET /v1/health`

The CLI is only an in-guest wrapper over that transport.

### 2. Canonical Agent Surface Becomes a CLI

`void-box` provisions a small helper command in the guest filesystem:

```bash
void-message context
void-message inbox [--since N]
void-message send \
  --kind proposal|signal|evaluation \
  --audience leader|broadcast \
  --summary "..."
  [--priority high|normal|low]
```

The CLI is responsible for:

- resolving the sidecar base URL
- issuing HTTP requests
- shaping valid JSON payloads
- applying expected headers such as `Idempotency-Key`
- returning non-zero exit status on failure

### 3. The Skill Explains the CLI, Not HTTP

The injected `void-messaging` skill must stop telling the agent to hand-write
`curl` calls. Instead it should explain:

- when to inspect context
- when to read inbox
- when to send `broadcast` vs `leader`
- the meaning of `proposal`, `signal`, and `evaluation`
- the usage of the `void-message` CLI

Example skill excerpt:

```md
# Collaboration Protocol

Use the `void-message` CLI for collaboration.

## Read context
`void-message context`

## Read inbox
`void-message inbox`
`void-message inbox --since 3`

## Send an intent
`void-message send --kind signal --audience broadcast --summary "cache misses dominate p99"`
`void-message send --kind proposal --audience leader --summary "promote cache-aware variant"`
```

### 4. Provider Bridges Are an Optional Higher Layer

Provider-specific integrations still exist, but they must sit above the same
canonical sidecar semantics.

Examples:

- Claude Code channel bridge
- MCP-based bridges
- custom provider-native messaging adapters

These are optimizations, not the source of truth.

Rules:

- generic OCI agents must work with only `sidecar + CLI + skill`
- provider bridges may translate provider-native actions into the same
  sidecar intent model
- provider bridges must not redefine message semantics

### 5. Claude Channels

Claude channels are treated as a provider-specific bridge, not as the base
protocol.

That means:

- the sidecar remains authoritative
- the channel bridge may push inbox updates into Claude
- the channel bridge may convert Claude-native collaboration actions into
  sidecar intents
- the generic CLI path still exists for the same run shape

So the layering is:

```text
sidecar semantics
  ├── generic path: void-message CLI
  └── Claude optimization: channel/MCP bridge
```

Claude channels therefore become an ergonomic acceleration layer, not a second
semantic system.

### 6. Launch-Time Provisioning

For messaging-enabled runs, `void-box` should provision all three:

- sidecar transport
- `void-message` CLI
- `void-messaging` skill that documents the CLI

This replaces the current weaker model of:

- sidecar transport
- skill-only guidance

### 7. Failure Model

If the sidecar is unavailable:

- `void-message` returns a non-zero exit status
- the agent may continue its main task
- no intent is recorded unless the CLI reports success

If a provider bridge is unavailable:

- the generic CLI path remains valid
- the run is still conformant

## Rationale

This gives us a stable progression:

1. sidecar transport
2. CLI wrapper
3. skill documentation for the CLI
4. optional provider-native bridges

That is better than "skill only" because it turns messaging from advisory
instructions into an operational interface.

## Required Follow-Up Changes

### `void-box`

- provision `void-message` into the guest
- inject sidecar connection info required by the CLI
- change `messaging_skill_content(...)` to document the CLI
- add tests proving:
  - CLI can read context
  - CLI can read inbox
  - CLI can send intents
  - injected skill references the CLI rather than raw HTTP

### `void-control`

- stop assuming raw-HTTP messaging skills
- update examples to use the CLI-oriented skill
- keep transport and message-box semantics unchanged

## Acceptance Criteria

- a messaging-enabled generic OCI agent can collaborate without hand-written
  HTTP requests
- the injected skill references `void-message`, not `curl`
- Claude-channel integration remains optional
- provider-specific bridges and generic OCI agents emit the same canonical
  intent shape into the sidecar
