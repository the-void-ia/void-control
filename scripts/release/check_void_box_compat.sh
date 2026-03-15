#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${1:-${VOID_BOX_BASE_URL:-http://127.0.0.1:43100}}"

echo "[void-control] checking daemon health at ${BASE_URL}"
curl -fsS "${BASE_URL}/v1/health" >/dev/null

echo "[void-control] running live daemon contract suite against ${BASE_URL}"
VOID_BOX_BASE_URL="${BASE_URL}" cargo test --features serde --test void_box_contract -- --ignored --nocapture
