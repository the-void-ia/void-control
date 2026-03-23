# Repo Docs And CI Hardening Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve `void-control` contributor documentation, architecture documentation, pre-commit checks, and CI coverage without changing runtime behavior.

**Architecture:** This change strengthens repository metadata around the current codebase rather than introducing new product behavior. The work is split into documentation updates, local pre-commit automation, and CI workflow hardening so each area stays focused and independently reviewable.

**Tech Stack:** Markdown, GitHub Actions, pre-commit, Rust/Cargo, Node/Vite

---

## File Map

- Create: `docs/architecture.md`
- Create: `.pre-commit-config.yaml`
- Create: `docs/superpowers/specs/2026-03-23-repo-docs-ci-design.md`
- Create: `docs/superpowers/plans/2026-03-23-repo-docs-ci-hardening.md`
- Modify: `AGENTS.md`
- Modify: `README.md`
- Modify: `.github/workflows/ci.yml`

## Chunk 1: Documentation

### Task 1: Rewrite `AGENTS.md`

**Files:**
- Modify: `AGENTS.md`

- [ ] **Step 1: Rewrite the repository guide**

Update `AGENTS.md` so it explains:
- system boundary between `void-control` and `void-box`
- repo/module layout
- recommended commands
- UI workflow expectations
- testing and PR expectations

- [ ] **Step 2: Review for consistency**

Check that commands, file paths, and expectations match the current repo.

### Task 2: Add `docs/architecture.md`

**Files:**
- Create: `docs/architecture.md`

- [ ] **Step 1: Write the architecture document**

Document the implemented architecture with:
- component overview
- data flows
- persistence and replay notes
- source file map

- [ ] **Step 2: Review for accuracy**

Verify the doc matches current module names and responsibilities.

### Task 3: Improve `README.md`

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add contributor-oriented links and development commands**

Keep quick start concise, but add:
- architecture link
- contributor guide link
- validation command list

- [ ] **Step 2: Review for duplication**

Ensure README stays high-level and delegates deeper detail to `AGENTS.md` and `docs/architecture.md`.

## Chunk 2: Local Validation

### Task 4: Add pre-commit config

**Files:**
- Create: `.pre-commit-config.yaml`

- [ ] **Step 1: Add repo-managed hooks**

Include local hooks for:
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo test --features serde`
- `npm run build` in `web/void-control-ux`

- [ ] **Step 2: Document hook usage**

Reference installation and usage from `README.md` or `AGENTS.md`.

## Chunk 3: CI

### Task 5: Expand GitHub CI workflow

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Split CI into focused jobs**

Add distinct jobs for:
- formatting
- clippy
- Rust tests
- serde tests
- docs
- UI build

- [ ] **Step 2: Keep workflow aligned with local checks**

Ensure the commands used in CI match the documented local validation flow where practical.

## Chunk 4: Verification

### Task 6: Run validation

**Files:**
- No code changes

- [ ] **Step 1: Validate Rust formatting**

Run: `cargo fmt --all -- --check`

- [ ] **Step 2: Validate clippy**

Run: `cargo clippy --all-targets --all-features -- -D warnings`

- [ ] **Step 3: Validate Rust tests**

Run: `cargo test`

- [ ] **Step 4: Validate serde tests**

Run: `cargo test --features serde`

- [ ] **Step 5: Validate UI build**

Run: `npm run build`
Working directory: `web/void-control-ux`

- [ ] **Step 6: Inspect final diff**

Run: `git diff --stat`

