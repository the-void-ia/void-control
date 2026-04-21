from __future__ import annotations

from typing import Any

from .models import SnapshotDeleteResult, SnapshotRecord


class SnapshotsClient:
    def __init__(self, client: object) -> None:
        self._client = client

    async def create(self, spec: dict[str, Any]) -> SnapshotRecord:
        payload = await self._client.post_json("/v1/snapshots", spec)
        return SnapshotRecord.from_json(payload)

    async def get(self, snapshot_id: str) -> SnapshotRecord:
        payload = await self._client.get_json(f"/v1/snapshots/{snapshot_id}")
        return SnapshotRecord.from_json(payload)

    async def list(self) -> list[SnapshotRecord]:
        payload = await self._client.get_json("/v1/snapshots")
        snapshots = payload.get("snapshots", [])
        return [SnapshotRecord.from_json({"snapshot": item}) for item in snapshots]

    async def replicate(
        self,
        snapshot_id: str,
        request: dict[str, Any],
    ) -> SnapshotRecord:
        payload = await self._client.post_json(
            f"/v1/snapshots/{snapshot_id}/replicate",
            request,
        )
        return SnapshotRecord.from_json(payload)

    async def delete(self, snapshot_id: str) -> SnapshotDeleteResult:
        payload = await self._client.delete_json(f"/v1/snapshots/{snapshot_id}")
        return SnapshotDeleteResult.from_json(payload)
