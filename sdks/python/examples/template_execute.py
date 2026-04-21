from __future__ import annotations

import asyncio
import json
import os

from void_control import VoidControlClient


async def main() -> None:
    base_url = os.environ.get("VOID_CONTROL_BASE_URL", "http://127.0.0.1:43210")
    template_id = os.environ.get("VOID_CONTROL_TEMPLATE_ID", "benchmark-runner-python")
    inputs = {
        "goal": os.environ.get(
            "VOID_CONTROL_TEMPLATE_GOAL",
            "Compare transform benchmark candidates",
        ),
        "provider": os.environ.get("VOID_CONTROL_TEMPLATE_PROVIDER", "claude"),
    }
    snapshot = os.environ.get("VOID_CONTROL_TEMPLATE_SNAPSHOT")
    if snapshot:
        inputs["snapshot"] = snapshot

    async with VoidControlClient(base_url=base_url) as client:
        execution = await client.templates.execute(template_id, inputs=inputs)
        detail = await client.executions.wait(execution.execution_id, poll_interval=2.0)

    print(
        json.dumps(
            {
                "template_id": template_id,
                "execution_id": execution.execution_id,
                "status": detail.execution.status,
                "best_candidate_id": detail.result.best_candidate_id,
                "completed_iterations": detail.result.completed_iterations,
                "total_candidate_failures": detail.result.total_candidate_failures,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    asyncio.run(main())
