## Summary

<!-- Brief description of what this PR changes and why. -->

## Type of Change

<!-- Mark the relevant option(s) with an "x" -->

- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change (contract or API)
- [ ] Documentation update
- [ ] Code refactoring
- [ ] Test addition or update
- [ ] Dev UX / tooling

## Related Issues

<!-- Link related issues here using #issue-number -->

Fixes #
Related to #

## Changes

<!-- List the main changes by area (control plane, runtime client, web UI, docs, tests). -->

-
-
-

## Test Plan

<!-- How the changes were validated locally. -->

- [ ] `cargo test --features serde`
- [ ] `cargo clippy --features serde --all-targets -- -D warnings`
- [ ] `cargo fmt --all -- --check`
- [ ] Web UI typecheck (`cd web/void-control-ux && npx tsc -b --noEmit`)
- [ ] Web UI build (`cd web/void-control-ux && npm run build`) — if UI changed
- [ ] Live contract gate against a running void-box daemon — if runtime client changed
- [ ] Manual execution run (swarm / supervision) — if orchestration behavior changed

### Commands run

```bash
# Paste the commands and their summarized output.
```

## Documentation

- [ ] Updated README.md / CLAUDE.md if user-facing behavior changed
- [ ] Updated examples/ if orchestration or runtime template behavior changed
- [ ] Updated spec/ if the contract changed
- [ ] Updated skills/void-control/SKILL.md if the operator workflow changed

## Compatibility

<!-- Does this PR require a specific void-box baseline? Contract version bump?
     Breaking change in persisted run state? Call it out here. -->

## Notes for Reviewers

<!-- Anything reviewers should focus on, known follow-ups, or out-of-scope items. -->
