from __future__ import annotations

import asyncio
import json
import os

from void_control import VoidControlClient


async def main() -> None:
    base_url = os.environ.get("VOID_CONTROL_BASE_URL", "http://127.0.0.1:43210")

    spec = {
        "api_version": "v1",
        "kind": "sandbox",
        "runtime": {
            "image": os.environ.get("VOID_CONTROL_SANDBOX_IMAGE", "python:3.12-slim"),
            "cpus": int(os.environ.get("VOID_CONTROL_SANDBOX_CPUS", "2")),
            "memory_mb": int(os.environ.get("VOID_CONTROL_SANDBOX_MEMORY_MB", "2048")),
        },
    }

    async with VoidControlClient(base_url=base_url) as client:
        sandbox = await client.sandboxes.create(spec)

    print(
        json.dumps(
            {
                "sandbox_id": sandbox.sandbox_id,
                "state": sandbox.state,
                "image": sandbox.image,
                "cpus": sandbox.cpus,
                "memory_mb": sandbox.memory_mb,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    asyncio.run(main())
