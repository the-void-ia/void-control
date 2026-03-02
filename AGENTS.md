# Repository Guidelines

## Project Structure & Module Organization
This repository currently contains architecture and runtime-contract documentation for Void Control.

- `spec/`: Canonical specifications (for example, `spec/void-control-runtime-spec-v0.1.md`).
- `LICENSE`: Project license.

When adding implementation code, keep the same separation of concerns defined in the spec:
- Control-plane orchestration logic should be separate from runtime execution logic.
- Add new specs to `spec/` and version them in the filename (for example, `*-v0.2.md`).

## Build, Test, and Development Commands
Use Cargo for local development and validation:

- `cargo test`: Run core unit tests (no optional JSON compatibility feature).
- `cargo test --features serde`: Run JSON compatibility tests and fixture-based checks.
- `cargo test --features serde runtime::void_box::`: Run live-daemon client contract tests (mocked transport).
- `VOID_BOX_BASE_URL=http://127.0.0.1:3000 cargo test --features serde --test void_box_contract -- --ignored --nocapture`: Run live daemon contract gate tests (tests auto-generate fallback specs under `/tmp`).
- Optional spec overrides for policy behavior checks:
  - `VOID_BOX_TIMEOUT_SPEC_FILE`
  - `VOID_BOX_PARALLEL_SPEC_FILE`
  - `VOID_BOX_RETRY_SPEC_FILE`
  - `VOID_BOX_NO_POLICY_SPEC_FILE`
- `cargo run --example normalize_void_box_run`: Run the typed normalization example.
- `cargo run --bin normalize_fixture -- fixtures/sample.vbrun`: Normalize from local fixture format.

## Coding Style & Naming Conventions
For documentation and future code contributions:

- Use clear, boundary-focused naming aligned with the spec (`Run`, `Stage`, `Attempt`, `Runtime`, `Controller`).
- Keep Markdown headings hierarchical and concise.
- Prefer short sections and bullet lists over long prose blocks.
- Use ASCII unless a symbol is required for technical clarity.

## Testing Guidelines
- Keep contract tests in module `#[cfg(test)]` blocks close to conversion/runtime logic.
- Add fixture-based tests for compatibility behavior under `--features serde`.
- Validate both paths before PRs:
  - `cargo test`
  - `cargo test --features serde`

## Commit & Pull Request Guidelines
Git history is minimal (`Initial commit`), so adopt a consistent imperative style now:

- Commit format: `area: concise action` (example: `spec: clarify cancellation semantics`).
- Keep commits focused to one concern.
- PRs should include:
  - A short problem statement.
  - A summary of what changed.
  - Any spec sections affected (file paths + headings).
  - Follow-up work, if intentionally deferred.
