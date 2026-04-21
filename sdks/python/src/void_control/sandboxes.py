from __future__ import annotations

from typing import Any

from .models import SandboxExecResult, SandboxRecord


class SandboxesClient:
    def __init__(self, client: object) -> None:
        self._client = client

    async def create(self, spec: dict[str, Any]) -> SandboxRecord:
        payload = await self._client.post_json("/v1/sandboxes", spec)
        return SandboxRecord.from_json(payload)

    async def get(self, sandbox_id: str) -> SandboxRecord:
        payload = await self._client.get_json(f"/v1/sandboxes/{sandbox_id}")
        return SandboxRecord.from_json(payload)

    async def list(self) -> list[SandboxRecord]:
        payload = await self._client.get_json("/v1/sandboxes")
        sandboxes = payload.get("sandboxes", [])
        return [SandboxRecord.from_json({"sandbox": item}) for item in sandboxes]

    async def exec(
        self,
        sandbox_id: str,
        request: dict[str, Any],
    ) -> SandboxExecResult:
        payload = await self._client.post_json(f"/v1/sandboxes/{sandbox_id}/exec", request)
        return SandboxExecResult.from_json(payload)

    async def stop(self, sandbox_id: str) -> SandboxRecord:
        payload = await self._client.post_json(f"/v1/sandboxes/{sandbox_id}/stop", {})
        return SandboxRecord.from_json(payload)

    async def delete(self, sandbox_id: str) -> dict[str, Any]:
        return await self._client.delete_json(f"/v1/sandboxes/{sandbox_id}")
