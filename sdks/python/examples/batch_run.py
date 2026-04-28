from __future__ import annotations

import asyncio
import json
import os

from void_control import VoidControlClient


async def main() -> None:
    base_url = os.environ.get("VOID_CONTROL_BASE_URL", "http://127.0.0.1:43210")
    route = os.environ.get("VOID_CONTROL_BATCH_ROUTE", "batch")
    prompt_one = os.environ.get(
        "VOID_CONTROL_BATCH_PROMPT_ONE",
        "Fix failing auth tests",
    )
    prompt_two = os.environ.get(
        "VOID_CONTROL_BATCH_PROMPT_TWO",
        "Improve retry logging",
    )
    spec = {
        "api_version": "v1",
        "kind": route,
        "worker": {
            "template": "examples/runtime-templates/warm_agent_basic.yaml",
        },
        "jobs": [
            {"prompt": prompt_one},
            {"prompt": prompt_two},
        ],
    }

    async with VoidControlClient(base_url=base_url) as client:
        runner = client.yolo if route == "yolo" else client.batch
        runs = client.yolo_runs if route == "yolo" else client.batch_runs
        started = await runner.run(spec)
        detail = await runs.wait(started.run_id, poll_interval=2.0)

    print(
        json.dumps(
            {
                "route": route,
                "run_id": started.run_id,
                "kind": started.kind,
                "status": detail.execution.status,
                "execution_id": detail.execution.execution_id,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    asyncio.run(main())
