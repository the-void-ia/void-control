# void-control-ux

Graph-first operator dashboard for `void-control` and `void-box`.

## Stack

- React + TypeScript + Vite
- Sigma + Graphology (execution graph renderer)
- TanStack Query (polling and cache)
- Zustand (selection/pinning UI state)

## Run

```bash
cd web/void-control-ux
npm install
npm run dev -- --host 127.0.0.1 --port 3000
```

Open: `http://127.0.0.1:3000`

Default local env behavior:

- If `VITE_VOID_BOX_BASE_URL` is not set, the app uses `/api` (Vite proxy mode).
- If `VITE_VOID_CONTROL_BASE_URL` is not set, launch/upload uses `http://127.0.0.1:43210`.

Example `.env`:

```bash
VITE_VOID_BOX_BASE_URL=http://127.0.0.1:43100
VITE_VOID_CONTROL_BASE_URL=http://127.0.0.1:43210
```

## Launch Bridge (for YAML editor/upload)

The launch modal can upload/persist spec text via `voidctl serve` bridge mode.

Start bridge:

```bash
cargo run --features serde --bin voidctl -- serve
```

Run UI pointing to bridge:

```bash
VITE_VOID_BOX_BASE_URL=http://127.0.0.1:43100 \
VITE_VOID_CONTROL_BASE_URL=http://127.0.0.1:43210 \
npm run dev -- --host 127.0.0.1 --port 3000
```

## Requirements

- `void-box` daemon running (`/v1/health` reachable)
- Runtime contract endpoints available:
  - `GET /v1/runs?state=active|terminal`
  - `GET /v1/runs/{id}`
  - `GET /v1/runs/{id}/events`
  - `GET /v1/runs/{id}/stages`
  - `GET /v1/runs/{id}/telemetry`
- `POST /v1/runs`
- `POST /v1/launch` (bridge endpoint)
  - `POST /v1/runs/{id}/cancel`

## Current UX (MVP+)

- Runs list (active + terminal) with test-run filter
- Sigma execution graph with stage selection and dependency highlighting
- Node inspector (state/timing/dependencies/metrics/recent events)
- Launch Run modal (`+ Launch Box`) with:
  - spec path + run id
  - YAML/JSON upload
  - inline validation
- Run logs panel
- Cancel run action
- Live polling refresh
