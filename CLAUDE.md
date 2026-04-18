# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

For general development guidelines — architecture, module map, local
commands, testing, environment variables, and commit/PR conventions —
see @AGENTS.md.

## Claude-specific guidance

- Before implementing non-trivial changes, propose a plan and explain
  the tradeoffs. Wait for alignment before editing.
- Preserve the control-plane / runtime boundary described in @AGENTS.md.
  Runtime-transport and VM-isolation concerns belong to `void-box`, not
  here. When in doubt, prefer a normalization layer in `src/contract/`
  over leaking runtime details into orchestration.
- Prefer LSP operations (`goToDefinition`, `findReferences`, `hover`)
  over Grep/Glob for Rust code navigation. Fall back to Grep/Glob only
  for comments, config files, and non-Rust code.
- For UI work in `web/void-control-ux`, use browser automation (MCP or
  Playwright) for DOM, layout, resize, console, and network validation.
  Screenshots are fallback only.
- Every test target is currently gated on `--features serde`, so
  `cargo test --features serde` is the one validation path — plain
  `cargo test` runs zero tests.
- Be terse: skip end-of-turn summaries.
