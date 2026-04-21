from __future__ import annotations

from typing import Any

from .models import PoolRecord


class PoolsClient:
    def __init__(self, client: object) -> None:
        self._client = client

    async def create(self, spec: dict[str, Any]) -> PoolRecord:
        payload = await self._client.post_json("/v1/pools", spec)
        return PoolRecord.from_json(payload)

    async def get(self, pool_id: str) -> PoolRecord:
        payload = await self._client.get_json(f"/v1/pools/{pool_id}")
        return PoolRecord.from_json(payload)

    async def scale(self, pool_id: str, request: dict[str, Any]) -> PoolRecord:
        payload = await self._client.post_json(f"/v1/pools/{pool_id}/scale", request)
        return PoolRecord.from_json(payload)
