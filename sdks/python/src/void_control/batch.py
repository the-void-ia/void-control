from __future__ import annotations

import asyncio
from typing import Any

from .models import BatchRunDetail, BatchRunResult


class BatchClient:
    def __init__(self, client: object, *, route_base: str) -> None:
        self._client = client
        self._route_base = route_base

    async def run(self, spec: dict[str, Any]) -> BatchRunResult:
        payload = await self._client.post_json(f"{self._route_base}/run", spec)
        return BatchRunResult.from_json(payload)


class BatchRunsClient:
    def __init__(self, client: object, *, route_base: str) -> None:
        self._client = client
        self._route_base = route_base

    async def get(self, run_id: str) -> BatchRunDetail:
        payload = await self._client.get_json(f"{self._route_base}-runs/{run_id}")
        return BatchRunDetail.from_json(payload)

    async def wait(self, run_id: str, *, poll_interval: float = 1.0) -> BatchRunDetail:
        while True:
            detail = await self.get(run_id)
            if detail.execution.status in {"Completed", "Failed", "Canceled"}:
                return detail
            await asyncio.sleep(poll_interval)
