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

## Dev workflow

The browser cannot speak AF_UNIX, so the Vite `/api` proxy targets the
**void-control bridge** (TCP), and the bridge in turn dispatches to the
daemon over whichever transport it was configured with — AF_UNIX by
default, TCP when explicitly opted into. Run order:

1. Start the daemon (default AF_UNIX, no flags needed):
   ```bash
   cargo run --bin voidbox -- serve
   ```
2. Start the bridge (default `127.0.0.1:43210`):
   ```bash
   cargo run --features serde --bin voidctl -- serve
   ```
3. Start the dev server:
   ```bash
   cd web/void-control-ux
   npm run dev -- --host 127.0.0.1 --port 3000
   ```

Defaults:

- If `VITE_VOID_BOX_BASE_URL` is not set, the app uses `/api`, which the
  Vite proxy forwards to the bridge.
- If `VITE_VOID_CONTROL_BASE_URL` is not set, launch/upload uses
  `http://127.0.0.1:43210` (the bridge).
- The bridge listen address is overridable via `VOID_CONTROL_BRIDGE_LISTEN`
  in the bridge process, and the proxy target via
  `VITE_VOID_CONTROL_BRIDGE_TARGET` in the dev server.

Example `.env`:

```bash
VITE_VOID_CONTROL_BASE_URL=http://127.0.0.1:43210
```

There is no `VITE_VOID_BOX_BASE_URL` example: the daemon defaults to
AF_UNIX with mode `0o600` and is unreachable from a browser. Pointing
`VITE_VOID_BOX_BASE_URL` at a daemon URL only makes sense in the rare
deployment shape where the daemon listens on TCP behind a CORS-aware
reverse proxy.

## Cross-uid / TCP daemon

When the bridge and daemon run under different uids (deployment shape), the
bridge consults `VOID_BOX_BASE_URL` for the daemon URL and the daemon
emits a bearer token at `$XDG_CONFIG_HOME/voidbox/daemon-token` (or
honors `VOIDBOX_DAEMON_TOKEN` / `VOIDBOX_DAEMON_TOKEN_FILE`). The bridge
loads the token from the same precedence chain and injects
`Authorization: Bearer …` on every TCP daemon request. No browser-side
configuration changes — the proxy still terminates at the bridge.

## Requirements

- `void-box` daemon running (`/v1/health` reachable through the bridge proxy)
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
