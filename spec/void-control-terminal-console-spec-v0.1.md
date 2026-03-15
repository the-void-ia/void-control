# Void Control Terminal Console Specification

## Version: v0.1

## Scope
Define a terminal-first user experience (no web UI) for operating
`void-control` + `void-box` runs with interactive commands and live event/log
feedback.

This spec complements `void-control-ux-visualization-spec-v0.1.md`.

---

## 1. Product Flavor (No UI)

Single binary experience:
- interactive console (REPL style)
- command-based orchestration
- live stream of run events/logs
- recovery/resume after disconnect

Target users:
- developers
- infra/operators
- CI/debug workflows

---

## 2. Console Command Set (v0.1)

Required commands:
- `/run <spec_file> [--run-id <id>] [--policy <preset|json>]`
- `/status <run_id>`
- `/events <run_id> [--from <event_id>]`
- `/logs <run_id> [--follow]`
- `/cancel <run_id> [--reason <text>]`
- `/list [--state active|terminal]`
- `/resume <run_id>`
- `/help`
- `/exit`

Optional quality-of-life:
- `/watch <run_id>` (status + events tail combined)
- `/policy presets`

---

## 3. Runtime/Controller Contract Mapping

Console actions map directly to run-level APIs:
- start: `POST /v1/runs`
- inspect: `GET /v1/runs/{id}`
- events: `GET /v1/runs/{id}/events?from_event_id=...`
- cancel: `POST /v1/runs/{id}/cancel`
- list: `GET /v1/runs?state=...`

No stage-level orchestration in console logic.

---

## 4. Session & Persistence Rules

Local session file (recommended path):
- `~/.void-control/session.json`

Persist:
- last selected run
- last seen event id per run
- active watch mode settings
- recent command history

On console restart:
1. reload session file
2. offer `/resume <run_id>` for prior active runs
3. continue stream from `last_seen_event_id`

---

## 5. Output Model (Terminal UX)

## 5.1 Status line
- run_id, attempt_id, state, active_stage_count, active_microvm_count

## 5.2 Event line format
`[timestamp][seq][event_type][run_id] message`

## 5.3 Log chunk format
`[timestamp][run_id][stdout|stderr] <chunk>`

Use ANSI colors by state:
- running blue
- succeeded green
- failed red
- canceled gray

---

## 6. Error Handling UX

All non-2xx responses must render:
- `code`
- `message`
- `retryable`

Examples:
- `NOT_FOUND` -> suggest `/list`
- `ALREADY_TERMINAL` -> suggest `/status` or `/events`
- `INVALID_POLICY` -> suggest `/policy presets`

---

## 7. Acceptance Criteria

1. User can start a run and see state transition to terminal in console.
2. User can disconnect/restart console and resume events via `from_event_id`.
3. Repeated cancel on terminal run returns stable terminal response.
4. Structured errors are shown without losing interactive session.
5. `list --state active` and `list --state terminal` work for reconciliation.

---

## 8. Non-Goals (v0.1)

- No web frontend.
- No multi-node cluster dashboard.
- No stage-level control commands (runtime internal concern).

