# `void-control` Release Process

This repository ships as a GitHub Release.

## `v0.0.1` Baseline

- Repo tag: `v0.0.1`
- Rust crate version: `0.0.1`
- UI package version: `0.0.1`
- Supported `void-box` baseline: `v0.1.1` or an equivalent validated production build

## Release Artifacts

- `voidctl-v<version>-x86_64-unknown-linux-gnu.tar.gz`
- `normalize_fixture-v<version>-x86_64-unknown-linux-gnu.tar.gz`
- `void-control-ux-v<version>.tar.gz`

The UI is shipped as a built artifact from `web/void-control-ux/dist/`. It is not published to npm for `v0.0.1`.

## Required Checks

Before cutting a release tag:

1. Rust CI passes:
   - `cargo test`
   - `cargo test --features serde`
2. UI CI passes:
   - `cd web/void-control-ux && npm ci && npm run build`
3. `void-box` compatibility gate passes against the supported daemon baseline:
   - `scripts/release/check_void_box_compat.sh http://127.0.0.1:43100`

## Tag-Driven Release

Releases are created by pushing a semver tag:

```bash
git tag v0.0.1
git push origin v0.0.1
```

The release workflow will:

- verify that the tag matches `Cargo.toml` and `web/void-control-ux/package.json`
- run Rust and UI build jobs
- package Rust binaries
- package the UI production bundle
- publish a GitHub Release with those assets

## `void-box` Compatibility Gate

`void-control` is released independently from `void-box`, but releases must be validated against a pinned `void-box` version/build.

For `v0.0.1`, use:

- `void-box` `v0.1.1`
- production kernel/initramfs path as documented in [AGENTS.md](../AGENTS.md)

Run compatibility manually:

```bash
scripts/release/check_void_box_compat.sh http://127.0.0.1:43100
```

Or via the dedicated GitHub Actions workflow on a self-hosted runner that can reach a real daemon.
