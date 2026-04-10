#!/usr/bin/env python3

from __future__ import annotations

import json
import os
import pathlib
import subprocess
import sys


DEFAULT_OUTPUT = pathlib.Path("/workspace/output.json")
BENCHMARK = pathlib.Path("/workspace/runtime-assets/transform_benchmark.py")


def output_path() -> pathlib.Path:
    return pathlib.Path(os.environ.get("OUTPUT_FILE", str(DEFAULT_OUTPUT)))


def env_float(name: str, default: float) -> float:
    raw = os.environ.get(name, str(default)).strip()
    try:
        return float(raw)
    except ValueError as exc:
        raise SystemExit(f"{name} must be a float, got {raw!r}") from exc


def mark_approval(result: dict[str, object]) -> dict[str, object]:
    metrics = result.get("metrics")
    if not isinstance(metrics, dict):
        raise SystemExit("benchmark output is missing metrics")

    latency = float(metrics.get("latency_p99_ms", 0.0))
    error_rate = float(metrics.get("error_rate", 1.0))
    cpu_pct = float(metrics.get("cpu_pct", 100.0))
    latency_limit = env_float("SUPERVISION_MAX_LATENCY_P99_MS", 10.0)
    error_limit = env_float("SUPERVISION_MAX_ERROR_RATE", 0.34)
    cpu_limit = env_float("SUPERVISION_MAX_CPU_PCT", 100.0)

    approved = (
        result.get("status") == "success"
        and latency <= latency_limit
        and error_rate <= error_limit
        and cpu_pct <= cpu_limit
    )
    metrics["approved"] = 1.0 if approved else 0.0
    result["summary"] = (
        f"{result.get('summary', '').strip()} | "
        f"supervision approval={'approved' if approved else 'revision_requested'}"
    ).strip()
    return result


def main() -> None:
    output = output_path()
    output.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run([sys.executable, str(BENCHMARK)], check=True)
    result = json.loads(output.read_text())
    result = mark_approval(result)
    output.write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
