from __future__ import annotations

from typing import Any

import httpx

from .batch import BatchClient, BatchRunsClient
from .executions import ExecutionsClient
from .templates import TemplatesClient
from .models import BridgeError


class VoidControlClient:
    def __init__(
        self,
        base_url: str,
        *,
        transport: httpx.AsyncBaseTransport | None = None,
        timeout: float = 30.0,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self._http = httpx.AsyncClient(
            base_url=self.base_url,
            transport=transport,
            timeout=timeout,
        )
        self.templates = TemplatesClient(self)
        self.executions = ExecutionsClient(self)
        self.batch = BatchClient(self, route_base="/v1/batch")
        self.batch_runs = BatchRunsClient(self, route_base="/v1/batch")
        self.yolo = BatchClient(self, route_base="/v1/yolo")
        self.yolo_runs = BatchRunsClient(self, route_base="/v1/yolo")

    async def aclose(self) -> None:
        await self._http.aclose()

    async def __aenter__(self) -> "VoidControlClient":
        return self

    async def __aexit__(self, exc_type: Any, exc: Any, tb: Any) -> None:
        await self.aclose()

    async def get_json(self, path: str) -> dict[str, Any]:
        response = await self._http.get(path)
        return await self._decode_response(response)

    async def post_json(self, path: str, payload: dict[str, Any]) -> dict[str, Any]:
        response = await self._http.post(path, json=payload)
        return await self._decode_response(response)

    async def _decode_response(self, response: httpx.Response) -> dict[str, Any]:
        data = response.json()
        if response.status_code >= 400:
            raise BridgeError(
                message=str(data.get("message", f"bridge returned HTTP {response.status_code}")),
                code=None if data.get("code") is None else str(data.get("code")),
                retryable=data.get("retryable"),
            )
        return dict(data)
