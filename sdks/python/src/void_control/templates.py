from __future__ import annotations

from typing import Any

from .models import (
    TemplateDetail,
    TemplateDryRunResult,
    TemplateExecutionResult,
    TemplateSummary,
)

class TemplatesClient:
    def __init__(self, client: object) -> None:
        self._client = client

    async def list(self) -> list[TemplateSummary]:
        payload = await self._client.get_json("/v1/templates")
        return [
            TemplateSummary.from_json(dict(item))
            for item in payload.get("templates", [])
        ]

    async def get(self, template_id: str) -> TemplateDetail:
        payload = await self._client.get_json(f"/v1/templates/{template_id}")
        return TemplateDetail.from_json(payload)

    async def dry_run(
        self,
        template_id: str,
        *,
        inputs: dict[str, Any],
    ) -> TemplateDryRunResult:
        payload = await self._client.post_json(
            f"/v1/templates/{template_id}/dry-run",
            {"inputs": inputs},
        )
        return TemplateDryRunResult.from_json(payload)

    async def execute(
        self,
        template_id: str,
        *,
        inputs: dict[str, Any],
    ) -> TemplateExecutionResult:
        payload = await self._client.post_json(
            f"/v1/templates/{template_id}/execute",
            {"inputs": inputs},
        )
        return TemplateExecutionResult.from_json(payload)
