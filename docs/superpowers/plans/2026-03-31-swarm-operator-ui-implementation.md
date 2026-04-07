# Swarm Operator UI Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a distinct swarm operator experience to the dashboard so swarm executions default to an iteration-first orchestration view while preserving the existing runtime graph as a separate runtime inspection surface.

**Architecture:** Keep the current runtime graph, logs, and node inspector intact as the `Runtime` view. Add a swarm-specific derivation layer that converts bridge/runtime payloads into iteration, candidate, score, and health summaries, then render those summaries through new `Swarm` UI components and swarm-aware rows in the runs list. Leave backend contracts unchanged for this pass and derive swarm state from existing payloads wherever possible.

**Tech Stack:** React 18, TypeScript, Zustand, TanStack Query, existing Vite build, existing dashboard CSS, bridge/runtime HTTP APIs.

---

## Scope Check

This plan stays inside the current UI app:

- no bridge API changes
- no Rust orchestration changes
- no launch flow redesign

The work splits cleanly into:

1. client-side swarm data modeling
2. runs-list swarm summaries
3. execution-detail view split into `Runtime`, `Swarm`, and `Timeline`
4. swarm timeline and strategy presentation

## File Map

### Existing files to modify

- Modify: `web/void-control-ux/src/App.tsx`
  - select the right execution view
  - add detail tabs and selected swarm iteration state
  - keep the runtime graph path intact
- Modify: `web/void-control-ux/src/components/RunsList.tsx`
  - add swarm-aware row summaries and health chips
- Modify: `web/void-control-ux/src/components/EventTimeline.tsx`
  - support a swarm-oriented event summary mode in addition to the existing generic telemetry view
- Modify: `web/void-control-ux/src/lib/types.ts`
  - extend front-end types for bridge execution payloads and derived swarm view models
- Modify: `web/void-control-ux/src/lib/api.ts`
  - add execution-oriented bridge fetch helpers without disturbing current runtime helpers
- Modify: `web/void-control-ux/src/styles.css`
  - add tab shell, iteration strip, candidate board, score table, and swarm chip styling

### New files to create

- Create: `web/void-control-ux/src/lib/orchestration.ts`
  - derive swarm summaries from bridge execution payloads and event streams
- Create: `web/void-control-ux/src/components/SwarmOverview.tsx`
  - render the swarm header, iteration strip, candidate board, and summary cards
- Create: `web/void-control-ux/src/components/SwarmStrategyPanel.tsx`
  - render ranking, metrics, winner, and rejection/exclusion reasons

### Existing files to review while implementing

- Review: `docs/superpowers/specs/2026-03-31-swarm-operator-ui-design.md`
- Review: `web/void-control-ux/src/components/RunGraph.tsx`
- Review: `web/void-control-ux/src/components/NodeInspector.tsx`
- Review: `web/void-control-ux/src/store/ui.ts`

## Delivery Strategy

Implement in this order:

1. add bridge execution types and a pure derivation layer
2. add the runs-list swarm summary treatment
3. add the execution detail tab shell
4. build the swarm view components
5. adapt the timeline to show swarm transitions
6. run build validation and lightweight UI verification

## Chunk 1: Add Swarm Data Modeling

### Task 1: Add failing compile references for execution payload support

**Files:**
- Modify: `web/void-control-ux/src/lib/types.ts`
- Modify: `web/void-control-ux/src/lib/api.ts`

- [ ] **Step 1: Add temporary call sites in `App.tsx` that reference future bridge execution helpers**

Add references shaped like:

```ts
getExecution(executionId)
getExecutionEvents(executionId)
```

Expected: TypeScript build fails because the helper functions and types do not exist yet.

- [ ] **Step 2: Run the UI build to verify the missing bridge helpers fail**

Run:

```bash
cd web/void-control-ux
npm run build
```

Expected: TypeScript error mentioning missing execution helpers or execution payload types.

- [ ] **Step 3: Add execution payload types**

Add front-end types for the bridge payloads the swarm UI will consume. At minimum define:

```ts
export interface ExecutionInspection {
  execution_id: string;
  status: string;
  goal?: string;
  completed_iterations?: number;
  candidate_dispatch_count?: number;
  candidate_output_count?: number;
  failed_candidate_count?: number;
  result_best_candidate_id?: string | null;
  current_iteration?: number | null;
}

export interface ExecutionEvent {
  event_id?: string;
  event_type: string;
  timestamp?: string;
  payload?: Record<string, unknown> | null;
}
```

Keep the types permissive enough to match current bridge payload variability.

- [ ] **Step 4: Add bridge fetch helpers**

Implement minimal helpers in `web/void-control-ux/src/lib/api.ts`:

```ts
export async function getExecution(executionId: string): Promise<ExecutionInspection> {}
export async function getExecutionEvents(executionId: string): Promise<ExecutionEvent[]> {}
export async function getExecutions(): Promise<ExecutionInspection[]> {}
```

Use `controlBaseUrl` and existing `requestJsonAt`.

- [ ] **Step 5: Re-run the UI build**

Run:

```bash
cd web/void-control-ux
npm run build
```

Expected: the next failure moves to missing swarm derivation logic or UI references rather than missing bridge helpers.

- [ ] **Step 6: Commit**

```bash
git add web/void-control-ux/src/lib/types.ts web/void-control-ux/src/lib/api.ts web/void-control-ux/src/App.tsx
git commit -m "ui: add bridge execution client types"
```

### Task 2: Build a pure swarm derivation layer

**Files:**
- Create: `web/void-control-ux/src/lib/orchestration.ts`
- Modify: `web/void-control-ux/src/lib/types.ts`

- [ ] **Step 1: Write the failing derivation scaffold**

Create exported functions shaped like:

```ts
export function deriveSwarmExecutionSummary(...) {}
export function deriveIterationSummaries(...) {}
export function deriveCandidateCards(...) {}
export function classifyExecutionHealth(...) {}
```

Use placeholder returns that make TypeScript compile but leave obvious `TODO` branches.

- [ ] **Step 2: Wire one temporary call from `App.tsx` or `RunsList.tsx`**

Expected: build fails because the placeholder outputs do not satisfy the caller shape yet.

- [ ] **Step 3: Implement the minimal derived view-models**

Add explicit UI-facing types such as:

```ts
export interface SwarmIterationSummary {
  iteration: number;
  running: number;
  outputReady: number;
  scored: number;
  failed: number;
}

export interface SwarmCandidateCard {
  candidateId: string;
  state: 'running' | 'output_ready' | 'scored' | 'failed' | 'best' | 'promoted' | 'rejected';
  score?: number | null;
  metrics: Array<{ label: string; value: string }>;
  reason?: string | null;
}
```

Derive from existing execution fields first, then supplement with events when needed. Do not require new backend fields in this pass.

- [ ] **Step 4: Add runtime-vs-orchestration classification**

Provide derivation helpers that distinguish:
- execution health chips
- candidate state chips
- runtime issue hints versus scoring/reduction outcomes

- [ ] **Step 5: Run the UI build**

Run:

```bash
cd web/void-control-ux
npm run build
```

Expected: PASS or the next failure points to missing rendering components only.

- [ ] **Step 6: Commit**

```bash
git add web/void-control-ux/src/lib/orchestration.ts web/void-control-ux/src/lib/types.ts web/void-control-ux/src/App.tsx web/void-control-ux/src/components/RunsList.tsx
git commit -m "ui: derive swarm execution summaries"
```

## Chunk 2: Add Swarm-Aware Runs List

### Task 3: Render compact swarm summaries in the runs list

**Files:**
- Modify: `web/void-control-ux/src/components/RunsList.tsx`
- Modify: `web/void-control-ux/src/styles.css`

- [ ] **Step 1: Replace the current run-row-only markup with a row component that can render extra swarm metadata**

Keep generic runtime rows simple. For swarm rows add:
- iteration progress
- candidate counters
- best-candidate chip
- degradation chips

- [ ] **Step 2: Add a failing visual placeholder state**

Make the UI render placeholder labels like `iter ?`, `outputs ?`, or `winner ?` until the row receives the real derived data.

- [ ] **Step 3: Connect real derived swarm row summaries**

Use `deriveSwarmExecutionSummary(...)` to populate:

```ts
running / outputReady / scored / failed
currentIteration / completedIterations
bestCandidateId
healthChips
```

Keep the rendering resilient when a row is not a swarm execution.

- [ ] **Step 4: Add styles for compact summary chips**

Add CSS for:
- row sublines
- tiny metric counters
- warning/degradation chips
- best-candidate chip

- [ ] **Step 5: Run build validation**

Run:

```bash
cd web/void-control-ux
npm run build
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add web/void-control-ux/src/components/RunsList.tsx web/void-control-ux/src/styles.css web/void-control-ux/src/App.tsx
git commit -m "ui: add swarm run summaries"
```

## Chunk 3: Split Execution Detail Into Runtime, Swarm, and Timeline

### Task 4: Add tab state and preserve the current runtime graph

**Files:**
- Modify: `web/void-control-ux/src/App.tsx`
- Modify: `web/void-control-ux/src/store/ui.ts`
- Modify: `web/void-control-ux/src/styles.css`

- [ ] **Step 1: Add a failing tab shell with static tabs**

Render tabs:

```tsx
Runtime | Swarm | Timeline
```

Default to `Swarm` when the selected item is a swarm execution and to `Runtime` otherwise.

- [ ] **Step 2: Run build validation to catch any missing state or prop wiring**

Run:

```bash
cd web/void-control-ux
npm run build
```

Expected: build failure or lint-like type failure around tab state.

- [ ] **Step 3: Add persisted UI state for selected detail tab and selected swarm iteration**

Extend the UI store with fields like:

```ts
selectedDetailTab: 'runtime' | 'swarm' | 'timeline';
selectedSwarmIterationByExecution: Record<string, number | undefined>;
```

- [ ] **Step 4: Move the current graph/log/inspector layout under the `Runtime` tab**

Do not rewrite `RunGraph` or `NodeInspector` yet. Just preserve the existing path and mount it only when the runtime tab is active.

- [ ] **Step 5: Add tab styling**

Create a tab bar that visually distinguishes the active layer without looking like the existing filter pills.

- [ ] **Step 6: Run build validation**

Run:

```bash
cd web/void-control-ux
npm run build
```

Expected: PASS, with the runtime tab matching current behavior when selected.

- [ ] **Step 7: Commit**

```bash
git add web/void-control-ux/src/App.tsx web/void-control-ux/src/store/ui.ts web/void-control-ux/src/styles.css
git commit -m "ui: split execution detail views"
```

## Chunk 4: Implement The Swarm View

### Task 5: Build the swarm overview surface

**Files:**
- Create: `web/void-control-ux/src/components/SwarmOverview.tsx`
- Modify: `web/void-control-ux/src/App.tsx`
- Modify: `web/void-control-ux/src/styles.css`

- [ ] **Step 1: Write the component skeleton with mocked props**

The component should accept:

```ts
executionSummary
iterations
candidates
selectedIteration
onSelectIteration
```

Render empty states for:
- no swarm data
- no candidates yet
- iteration selected but awaiting outputs

- [ ] **Step 2: Mount the skeleton under the `Swarm` tab**

Expected: build passes but the component shows placeholders.

- [ ] **Step 3: Implement the execution header and health chips**

Render:
- execution status
- goal
- current iteration
- best candidate
- health/degradation chips

- [ ] **Step 4: Implement the iteration strip**

Each iteration card should show:
- iteration number
- running count
- output-ready count
- scored count
- failed count

Selection should drive both the candidate board and strategy panel.

- [ ] **Step 5: Implement the candidate board**

Each card should show:
- candidate id
- explicit state
- score when available
- key metric values when available
- failure or exclusion reason when present

Do not use a graph here. Use a card grid with strong state signaling.

- [ ] **Step 6: Run build validation**

Run:

```bash
cd web/void-control-ux
npm run build
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add web/void-control-ux/src/components/SwarmOverview.tsx web/void-control-ux/src/App.tsx web/void-control-ux/src/styles.css web/void-control-ux/src/lib/orchestration.ts
git commit -m "ui: add swarm overview view"
```

### Task 6: Build the strategy panel inside the swarm view

**Files:**
- Create: `web/void-control-ux/src/components/SwarmStrategyPanel.tsx`
- Modify: `web/void-control-ux/src/components/SwarmOverview.tsx`
- Modify: `web/void-control-ux/src/styles.css`

- [ ] **Step 1: Add the failing strategy panel mount**

Mount a placeholder ranking panel beneath or beside the candidate board.

- [ ] **Step 2: Implement ranking and winner rendering**

Render:
- ordered candidates
- score values
- highlighted winner
- excluded candidates with explicit reasons

- [ ] **Step 3: Implement metric-breakdown rows**

Show the metrics that influenced ranking, for example:
- latency
- error rate
- CPU

If only partial data exists, show explicit placeholders instead of silently omitting fields.

- [ ] **Step 4: Surface promotion and rejection reasoning**

Show whether the candidate was:
- selected as best
- rejected after scoring
- excluded for missing output
- failed before scoring

- [ ] **Step 5: Run build validation**

Run:

```bash
cd web/void-control-ux
npm run build
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add web/void-control-ux/src/components/SwarmStrategyPanel.tsx web/void-control-ux/src/components/SwarmOverview.tsx web/void-control-ux/src/styles.css web/void-control-ux/src/lib/orchestration.ts
git commit -m "ui: add swarm strategy panel"
```

## Chunk 5: Make Timeline Swarm-Aware

### Task 7: Add a swarm-oriented timeline mode

**Files:**
- Modify: `web/void-control-ux/src/components/EventTimeline.tsx`
- Modify: `web/void-control-ux/src/App.tsx`
- Modify: `web/void-control-ux/src/styles.css`

- [ ] **Step 1: Add a timeline mode prop**

Support:

```ts
mode?: 'runtime' | 'swarm'
```

- [ ] **Step 2: Add a failing swarm event summary section**

Display placeholder sections for:
- dispatch
- output ready
- scoring
- winner selected

- [ ] **Step 3: Implement real swarm transition summaries**

Use the derived orchestration data to summarize:
- when candidates were dispatched
- when outputs were collected
- when an iteration completed
- when a winner was selected

Keep the existing telemetry chart for runtime mode unchanged.

- [ ] **Step 4: Run build validation**

Run:

```bash
cd web/void-control-ux
npm run build
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add web/void-control-ux/src/components/EventTimeline.tsx web/void-control-ux/src/App.tsx web/void-control-ux/src/styles.css web/void-control-ux/src/lib/orchestration.ts
git commit -m "ui: add swarm timeline mode"
```

## Chunk 6: Verify And Document

### Task 8: Run validation and lightweight operator checks

**Files:**
- Review only unless small fixes are needed:
  - `web/void-control-ux/src/App.tsx`
  - `web/void-control-ux/src/components/*.tsx`
  - `web/void-control-ux/src/lib/*.ts`
  - `web/void-control-ux/src/styles.css`

- [ ] **Step 1: Run the full UI build**

Run:

```bash
cd web/void-control-ux
npm run build
```

Expected: PASS.

- [ ] **Step 2: Run the Rust frontend gate if the repo expects all tests before merge**

Run:

```bash
cargo test --features serde
```

Expected: PASS, or no regressions caused by the UI-only change.

- [ ] **Step 3: Perform lightweight manual inspection**

Run:

```bash
cd web/void-control-ux
npm run dev -- --host 127.0.0.1 --port 3000
```

Check manually in a lightweight Chrome session:
- swarm execution defaults to `Swarm`
- runtime execution defaults to `Runtime`
- `Runtime` still shows the current graph and inspector
- `Swarm` shows iteration cards, candidate cards, and strategy
- `Timeline` shows swarm transitions without reusing the runtime graph model

- [ ] **Step 4: Update docs only if the operator workflow materially changed**

If needed, update:
- `AGENTS.md`
- `docs/architecture.md`
- `README.md`

Keep this scoped to actual workflow changes, not speculative UI notes.

- [ ] **Step 5: Commit**

```bash
git add web/void-control-ux docs/ AGENTS.md README.md
git commit -m "ui: add swarm operator views"
```
