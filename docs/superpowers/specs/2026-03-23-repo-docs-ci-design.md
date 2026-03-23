# Void-Control Docs And CI Design

## Goal

Improve `void-control`'s contributor-facing repository documentation and local/CI validation so the repo is easier to navigate, easier to maintain, and less dependent on tribal knowledge.

## Scope

This design covers:

- rewriting `AGENTS.md` into a stronger repository guide
- adding `docs/architecture.md` for `void-control`
- tightening `README.md` to link contributor and architecture docs
- adding repository-managed pre-commit hooks
- expanding GitHub CI coverage for Rust and UI validation

This design does not cover:

- cross-platform CI matrices
- MSRV policy
- security audit jobs
- release workflow redesign
- changes to `void-box`

## Current Problems

### Documentation gaps

- `AGENTS.md` is short and mostly procedural; it does not explain the current Rust module layout or the orchestration/runtime boundary in enough detail.
- There is no `docs/architecture.md`, so contributors have to infer architecture from specs and source files.
- `README.md` is useful for quick start but not for contributor orientation.

### Validation gaps

- CI currently runs only `cargo test`, `cargo test --features serde`, and the UI build.
- Formatting, clippy, and docs are not enforced in CI.
- There is no checked-in pre-commit configuration for local validation consistency.

## Design

### 1. `AGENTS.md`

Rewrite `AGENTS.md` as the repo-local contributor guide for agents and humans.

Sections:

- project purpose and control-plane/runtime boundary
- repository layout with key directories
- module map for `src/contract`, `src/runtime`, `src/orchestration`, `src/bridge`, and `web/void-control-ux`
- required local validation commands
- guidance for UI work and browser-based inspection
- testing expectations
- commit and PR expectations

Tone should remain concise and operational, but more informative than the current file.

### 2. `docs/architecture.md`

Add a contributor-focused architecture document for `void-control`.

Sections:

- overview and system boundary
- main components and responsibilities
- component diagram in ASCII
- core data flows:
  - execution submission
  - planning and iteration
  - candidate dispatch
  - artifact collection and reduction
  - signal-reactive planning path
- persistence and replay responsibilities
- source file map for quick navigation

This document should describe the code as implemented today and avoid speculative future architecture beyond brief notes.

### 3. `README.md`

Improve contributor orientation without turning the README into a full architecture doc.

Changes:

- add links to `docs/architecture.md`, `AGENTS.md`, and release-process docs
- add a short "Development" section with the main validation commands
- keep quick-start instructions concise

### 4. Pre-commit

Add a repository-managed `.pre-commit-config.yaml`.

Hooks:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo test --features serde`
- `npm run build` in `web/void-control-ux`

Rationale:

- these are already meaningful repo checks
- they align with local development and CI
- they do not introduce speculative tooling not already used by the repo

### 5. CI

Expand `.github/workflows/ci.yml` into separate jobs for clearer failure modes.

Jobs:

- `fmt`
- `clippy`
- `rust-test`
- `rust-test-serde`
- `rust-doc`
- `ui-build`

Details:

- use stable Rust
- keep current Ubuntu-only baseline
- enable `RUSTDOCFLAGS=-D warnings` for docs
- keep the existing compatibility workflow separate

## Trade-offs

### Why not copy `void-box` CI exactly

`void-box` has a broader platform matrix and stronger runtime-specific constraints. `void-control` does not yet need the same level of CI breadth, and copying it directly would add cost and noise without clear benefit.

### Why include `cargo test` and `cargo test --features serde` in pre-commit

They are heavier than formatting checks, but this repo is still small enough that the stronger local gate is practical. The goal is to catch breakage before push, not optimize for very fast hooks.

## Success Criteria

- a new contributor can find the architecture and main module boundaries quickly
- local validation commands are documented once and consistent across docs, hooks, and CI
- CI failures clearly identify whether the issue is formatting, linting, docs, Rust tests, serde tests, or UI build
- the repo has a checked-in pre-commit configuration contributors can install locally
