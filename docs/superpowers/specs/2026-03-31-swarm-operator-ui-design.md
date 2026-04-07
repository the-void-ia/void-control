# Swarm Operator UI Design

## Goal

Preserve the graph-first operator experience from the original dashboard while
making swarm executions legible as orchestration strategies rather than forcing
them into the same runtime semantics or into a separate card-driven product
mode.

The UI should keep the execution graph as the primary/default view. For swarm
executions, the graph should represent parallel candidate executions and their
iteration relationships instead of low-level runtime stage flow.

## Core Correction

The previous direction treated `Swarm` as a separate primary view. That is the
wrong model.

`Swarm` is not a different product surface. It is an orchestration strategy
applied to an execution. Operators still need the graph-first experience from
the original demo. What changes is the meaning of the nodes and edges.

Correct mental model:

- one default graph-first execution surface
- graph semantics depend on execution type
- inspector and secondary panels explain scoring, failure, and promotion
- timeline remains forensic, not primary

## Problem

The current UI problems are not just styling issues:

- swarm was split into a separate primary view and lost the original graph-first
  feel
- orchestration decisions became a card wall instead of a graph of parallel
  candidate executions
- runtime and orchestration were separated too strongly, making the original
  demo view effectively disappear
- operators lost the immediate visual answer to:
  - what is running in parallel?
  - which candidate won?
  - how do iterations relate?

This breaks the continuity with the existing dashboard and weakens the operator
experience.

## Scope

This pass covers:

- keeping graph-first execution detail as the default surface
- making swarm executions render as orchestration graphs
- preserving node/edge inspection as the core interaction model
- moving scoring and decision explanation into inspector-side content or a
  secondary tab

This pass does not cover:

- replacing the graph renderer
- redesigning launch flows
- changing bridge or daemon contracts unless required to supply missing graph
  data

## User Priorities

The operator still needs:

1. graph-first visibility of what is running in parallel
2. clear iteration progress and scoring decisions
3. failure and partial-output visibility without losing the original execution
   view

These goals should be achieved without abandoning the node-and-line experience
from the demo.

## Information Architecture

### Default Execution View

The default execution page remains graph-first.

For runtime executions:

- nodes represent runtime stages / boxes
- edges represent runtime flow and dependencies

For swarm executions:

- nodes represent candidate executions
- edges represent orchestration relationships, such as sibling parallelism,
  reduction, promotion, or iteration progression
- iteration grouping should be visible in the layout

The graph remains the main screen in both cases.

### Secondary Surfaces

Keep secondary surfaces for deeper analysis:

- `Timeline`
- `Strategy` or `Decision`

These are supporting views, not replacements for the graph.

## Swarm Graph Design

### Node Semantics

For swarm executions, nodes should represent candidates, not runtime stages.

Each candidate node should communicate:

- candidate id
- iteration number
- state:
  - queued
  - running
  - output ready
  - scored
  - failed
  - best
- key metrics when available

The graph should make parallel candidate execution immediately visible.

### Edge Semantics

Edges should communicate orchestration structure, not runtime transport.

Examples:

- parent execution to iteration cluster
- iteration cluster to candidate nodes
- winner / promoted candidate highlighted into the next iteration
- rejected or terminal candidates visually de-emphasized

The graph should answer:

- which candidates were launched in parallel?
- which candidate advanced or won?
- how did the execution move from one iteration to the next?

### Iteration Layout

Swarm should use a layered or clustered graph layout, not a service mesh look.

Recommended structure:

- one visual lane or cluster per iteration
- sibling candidates aligned together within an iteration
- winner path highlighted across iterations

This keeps the graph compatible with the demo’s node-and-line interaction while
making swarm semantics explicit.

## Inspector Design

The inspector remains central.

When a swarm candidate node is selected, the inspector should show:

- candidate state
- runtime handle / run id
- metrics
- whether output is available
- score or decision status
- failure reason when relevant

The inspector should also explain:

- selected as best
- promoted
- rejected
- excluded for missing output
- failed before scoring

This is where decision explanation belongs first, rather than in a separate
primary layout.

## Strategy / Decision Surface

Scoring and decision analysis still matter, but should not replace the graph.

Use a secondary `Decision` or `Strategy` tab/panel for:

- winner summary
- metric breakdown
- ranking details
- convergence context
- promotion/rejection rationale

This answers the “why” after the graph answers the “what happened in parallel”.

## Timeline

The timeline remains the forensic view.

It should emphasize:

- candidate dispatch
- output readiness
- scoring
- iteration completion
- winner selection

But it should remain secondary to the graph.

## Runs List

The runs list should remain compact, but it should reflect execution type.

For swarm executions, each row should show:

- execution status
- iteration progress
- best candidate when known
- a concise health/degradation indicator

This is enough to identify interesting executions before opening the graph.

## Failure Presentation

Failure display should still separate:

- runtime causes
- orchestration consequences

But failure detail should appear inside the node inspector and related panels,
not by replacing the graph.

## Implementation Direction

Recommended delivery order:

1. restore graph-first execution detail as the primary/default surface
2. make swarm executions render as candidate graphs instead of card grids
3. adapt node inspector for swarm candidate semantics
4. move scoring/decision explanation into the inspector and a secondary
   strategy tab
5. keep timeline as a secondary forensic view

## Design Principle

Do not build a separate “swarm application” inside the dashboard.

Instead:

- keep the original node-and-line experience
- change the graph semantics for swarm executions
- use inspector and secondary analysis surfaces to explain strategy decisions

That preserves the strength of the original demo while making orchestration
intelligible.
