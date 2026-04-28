from __future__ import annotations

import asyncio

from .models import ExecutionDetail


TERMINAL_STATUSES = {"Completed", "Failed", "Canceled"}


class ExecutionsClient:
    def __init__(self, client: object) -> None:
        self._client = client

    async def get(self, execution_id: str) -> ExecutionDetail:
        payload = await self._client.get_json(f"/v1/executions/{execution_id}")
        return ExecutionDetail.from_json(payload)

    async def wait(
        self,
        execution_id: str,
        *,
        poll_interval: float = 1.0,
    ) -> ExecutionDetail:
        while True:
            detail = await self.get(execution_id)
            if detail.execution.status in TERMINAL_STATUSES:
                return detail
            await asyncio.sleep(poll_interval)
