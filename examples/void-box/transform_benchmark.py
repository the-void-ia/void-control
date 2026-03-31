#!/usr/bin/env python3

from __future__ import annotations

import json
import math
import os
import pathlib
import time
from dataclasses import dataclass
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parent
FIXTURE_DIR = ROOT / "fixtures" / "transform_02"
DEFAULT_OUTPUT = pathlib.Path("/workspace/output.json")


@dataclass(frozen=True)
class CandidateConfig:
    strategy: str
    parallelism: int
    batch_size: int
    cache_mode: str
    validation_mode: str
    prefetch: str
    cpu_budget: int
    role: str


class TransformValidationError(Exception):
    pass


def env_int(name: str, default: int) -> int:
    raw = os.environ.get(name, str(default)).strip()
    try:
        value = int(raw)
    except ValueError as exc:
        raise SystemExit(f"{name} must be an integer, got {raw!r}") from exc
    return value


def candidate_config() -> CandidateConfig:
    return CandidateConfig(
        strategy=os.environ.get("TRANSFORM_STRATEGY", "baseline").strip() or "baseline",
        parallelism=max(1, env_int("TRANSFORM_PARALLELISM", 2)),
        batch_size=max(1, env_int("TRANSFORM_BATCH_SIZE", 16)),
        cache_mode=os.environ.get("TRANSFORM_CACHE_MODE", "disabled").strip() or "disabled",
        validation_mode=os.environ.get("TRANSFORM_VALIDATION_MODE", "balanced").strip()
        or "balanced",
        prefetch=os.environ.get("TRANSFORM_PREFETCH", "disabled").strip() or "disabled",
        cpu_budget=max(1, min(100, env_int("TRANSFORM_CPU_BUDGET", 70))),
        role=os.environ.get("TRANSFORM_ROLE", "benchmark").strip() or "benchmark",
    )


def output_path() -> pathlib.Path:
    return pathlib.Path(os.environ.get("OUTPUT_FILE", str(DEFAULT_OUTPUT)))


def load_fixture_records() -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for path in sorted(FIXTURE_DIR.glob("batch_*.jsonl")):
        for line in path.read_text().splitlines():
            records.append(json.loads(line))
    if not records:
        raise SystemExit(f"no fixture records found under {FIXTURE_DIR}")
    return records


def validate_payload_shape(record: dict[str, Any], config: CandidateConfig) -> dict[str, Any]:
    payload = record.get("payload")
    if not isinstance(payload, dict):
        raise TransformValidationError("payload must be an object")
    account = payload.get("account")
    if not isinstance(account, str) or not account:
        raise TransformValidationError("account must be a non-empty string")
    region = payload.get("region")
    if not isinstance(region, str) or not region:
        raise TransformValidationError("region must be a non-empty string")
    amount = payload.get("amount")
    if not isinstance(amount, (int, float)) or amount <= 0:
        if (
            config.validation_mode == "strict"
            and isinstance(amount, str)
            and amount.replace(".", "", 1).isdigit()
        ):
            payload = dict(payload)
            payload["amount"] = float(amount)
        else:
            raise TransformValidationError("amount must be a positive number")
    items = payload.get("items")
    if isinstance(items, dict) and config.validation_mode == "strict":
        ordered = [items[key] for key in sorted(items)]
        payload = dict(payload)
        payload["items"] = ordered
        items = ordered
    if not isinstance(items, list) or not items:
        raise TransformValidationError("items must be a non-empty list")
    if any(not isinstance(item, int) for item in items):
        raise TransformValidationError("items must contain only integers")
    if record.get("kind") == "refund":
        reason = payload.get("reason")
        if not isinstance(reason, str) or not reason:
            raise TransformValidationError("refunds require a reason")
    return payload


def work_units(record: dict[str, Any], payload: dict[str, Any], config: CandidateConfig) -> int:
    items = payload["items"]
    raw_size = len(json.dumps(record, sort_keys=True))
    units = 9000 + len(items) * 1400 + raw_size * 30
    strategy = config.strategy
    if strategy == "vectorized-parse":
        units = int(units * 0.72)
    elif strategy == "batch-fusion":
        units = int(units * 0.80)
    elif strategy == "cache-aware":
        units = int(units * 0.82)
    elif strategy == "conservative-validation":
        units = int(units * 1.25)
    elif strategy == "speculative-prefetch":
        units = int(units * 0.92)
    elif strategy == "low-cpu":
        units = int(units * 0.88)
    elif strategy == "high-throughput":
        units = int(units * 0.66)
    units = int(units / max(1, min(config.parallelism, 8)) * 2)
    return max(units, 3500)


def perform_cpu_work(units: int, seed: str) -> int:
    token = seed.encode("utf-8") or b"x"
    acc = 0x345678
    for idx in range(units):
        acc = ((acc * 1103515245) + token[idx % len(token)] + 12345 + idx) & 0xFFFFFFFF
    return acc


def percentile_ms(samples: list[float], quantile: float) -> float:
    ordered = sorted(samples)
    pos = max(0, min(len(ordered) - 1, math.ceil(quantile * len(ordered)) - 1))
    return ordered[pos] * 1000.0


def process_records(records: list[dict[str, Any]], config: CandidateConfig) -> dict[str, Any]:
    latencies: list[float] = []
    failures = 0
    cpu_start = time.process_time()
    wall_start = time.perf_counter()
    cache: dict[tuple[str, str, str], int] = {}

    for index, record in enumerate(records):
        start = time.perf_counter()
        try:
            payload = validate_payload_shape(record, config)
            cache_key = (
                str(payload["account"]),
                str(payload["region"]),
                str(record.get("kind", "order")),
            )
            units = work_units(record, payload, config)
            if config.strategy == "cache-aware" and cache_key in cache:
                units = max(150, int(units * 0.28))
            checksum = perform_cpu_work(units, str(cache_key))
            if config.strategy == "cache-aware":
                cache[cache_key] = checksum
            if config.strategy == "speculative-prefetch" and index + 1 < len(records):
                next_record = records[index + 1]
                next_payload = next_record.get("payload")
                if isinstance(next_payload, dict):
                    perform_cpu_work(1500, json.dumps(next_payload, sort_keys=True))
            if config.strategy == "low-cpu":
                time.sleep(0.0035)
        except TransformValidationError:
            failures += 1
            if config.strategy == "low-cpu":
                time.sleep(0.0025)
        latencies.append(time.perf_counter() - start)

    wall_elapsed = max(time.perf_counter() - wall_start, 1e-9)
    cpu_elapsed = max(time.process_time() - cpu_start, 0.0)
    cpu_pct = min(100.0, max(0.0, (cpu_elapsed / wall_elapsed) * 100.0))
    return {
        "latency_p99_ms": round(percentile_ms(latencies, 0.99), 3),
        "error_rate": round(failures / len(records), 6),
        "cpu_pct": round(cpu_pct, 3),
    }


def build_summary(config: CandidateConfig, metrics: dict[str, float]) -> str:
    return (
        f"{config.role} role: {config.strategy} strategy"
        f" + cache_mode={config.cache_mode}"
        f" + batch_size={config.batch_size}"
        f" -> latency_p99_ms={metrics['latency_p99_ms']:.3f},"
        f" error_rate={metrics['error_rate']:.6f}, cpu_pct={metrics['cpu_pct']:.3f}"
    )


def main() -> None:
    config = candidate_config()
    records = load_fixture_records()
    metrics = process_records(records, config)
    result = {
        "status": "success",
        "summary": build_summary(config, metrics),
        "metrics": metrics,
        "artifacts": [],
    }
    destination = output_path()
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
