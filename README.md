# void-control

Orchestration layer for `void-box` runtime execution.

![void-control hero](docs/assets/void-control-hero.png)

## Demo

<video src="docs/assets/void-control-demo.mp4" controls muted playsinline width="100%"></video>

If the embedded player does not render in your Markdown viewer, use the direct file link: [void-control demo video](docs/assets/void-control-demo.mp4).

## Release

- First public release target: `v0.0.1`
- Release artifacts are published through GitHub Releases
- Supported `void-box` baseline for `v0.0.1`: `void-box` `v0.1.1` or an equivalent validated production build
- Release process and compatibility gate details: [docs/release-process.md](docs/release-process.md)

## What It Is

`void-control` is the control-plane side of the stack:

- Launches and manages runs on `void-box`.
- Tracks run/stage/event lifecycle.
- Provides terminal-first and graph-first operator UX.
- Enforces runtime contract compatibility with `void-box`.

## Project Components

- `spec/`: Runtime and orchestration contracts.
- `src/`: Rust orchestration client/runtime normalization logic.
- `tests/`: Contract and compatibility tests.
- `web/void-control-ux/`: React operator dashboard (graph + inspector).

## Quick Start

### 1) Start `void-box` daemon

```bash
cargo run --bin voidbox -- serve --listen 127.0.0.1:43100
```

### 2) Run `void-control` tests

```bash
cargo test
cargo test --features serde
```

### 3) Run live daemon contract gate

```bash
VOID_BOX_BASE_URL=http://127.0.0.1:43100 \
cargo test --features serde --test void_box_contract -- --ignored --nocapture
```

### 4) Start graph dashboard

```bash
cd web/void-control-ux
npm install
VITE_VOID_BOX_BASE_URL=http://127.0.0.1:43100 npm run dev
```

### 5) Launch from YAML editor/upload (bridge)

Run bridge mode in another terminal:

```bash
cargo run --features serde --bin voidctl -- serve
```

Then start UI with bridge URL:

```bash
cd web/void-control-ux
VITE_VOID_BOX_BASE_URL=http://127.0.0.1:43100 \
VITE_VOID_CONTROL_BASE_URL=http://127.0.0.1:43210 \
npm run dev
```

## Terminal Console

```bash
cargo run --features serde --bin voidctl
```

## Notes

- Dashboard uses daemon APIs (`/v1/runs`, `/v1/runs/{id}/events`, `/v1/runs/{id}/stages`, `/v1/runs/{id}/telemetry`).
- `+ Launch Box` supports:
  - editor/upload launch through bridge (`POST /v1/launch`)
  - path-only fallback launch (`POST /v1/runs`) when no spec text is provided.
