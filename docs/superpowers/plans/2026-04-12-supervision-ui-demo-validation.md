# Supervision UI Demo Validation Plan

> **For agentic workers:** REQUIRED: Use superpowers:executing-plans or equivalent disciplined execution. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run the checked-in supervision example on a fresh stack, capture an operator-story UI video, and verify the supervision-specific UI path is presentation-ready.

**Architecture:** Reuse the existing `void-box` production Claude runtime, `voidctl` bridge, and Vite UI. Submit the checked-in supervision example through the live stack, then use Playwright to record the operator flow from `Launch Spec` through the supervision graph, inspector, and runtime jump.

**Tech Stack:** `void-box`, `voidctl`, bridge HTTP API, React/Vite UI, Playwright recording scripts, checked-in supervision example assets.

---

## Files

- Use: `examples/supervision-transform-review.yaml`
- Use: `examples/runtime-templates/transform_supervision_worker.yaml`
- Use: `examples/runtime-assets/transform_supervision_worker.py`
- Modify or create if needed: `web/void-control-ux/scripts/record-supervision-demo.mjs`
- Save assets under: `docs/assets/`

## Checklist

- [x] Stop old `void-box` processes
- [x] Stop old `voidctl` processes
- [x] Stop old UI dev servers
- [x] Confirm production `void-box` initramfs path
- [x] Start fresh `void-box` on `127.0.0.1:43100`
- [x] Start fresh `voidctl` bridge on `127.0.0.1:43210`
- [x] Start fresh UI on `127.0.0.1:3000`
- [x] Verify health endpoints for all three services
- [x] Submit `examples/supervision-transform-review.yaml`
- [x] Capture the execution id
- [x] Verify the supervision execution appears in the left rail
- [x] Verify the supervision graph renders
- [x] Verify the right inspector shows supervision-specific state
- [x] Verify `Open Runtime Graph` works
- [x] Record the operator story video with Playwright
- [x] Save recorded video to `docs/assets/`
- [ ] Optionally generate GIF if needed later
- [x] Review the captured demo for obvious visual bugs

## Acceptance Notes

- The demo should show the real supervision UI, not a mock.
- The video should include:
  - `Launch Spec`
  - launching the supervision YAML
  - supervision execution selection in the left rail
  - center supervision graph
  - right-side supervision inspector
  - runtime jump via `Open Runtime Graph`
- The current v1 supervision flow is acceptable if approval remains metric-driven through `metrics.approved`.
- Captured artifact: `docs/assets/void-control-supervision-demo.webm`
- Recorded execution: `exec-1776041810457`
- Runtime jump target captured in the recording: `run-1776041815464`
- Note: this recording was refreshed after fixing the supervision example policy so the captured execution completes successfully and shows supervision finalization in the real UI.
