import os
import sys
import unittest

import httpx


SDK_SRC = os.path.join(
    os.path.dirname(__file__),
    "..",
    "src",
)
sys.path.insert(0, os.path.abspath(SDK_SRC))


class ClientScaffoldTest(unittest.TestCase):
    def test_client_exposes_template_and_execution_subclients(self) -> None:
        from void_control import VoidControlClient

        client = VoidControlClient(base_url="http://127.0.0.1:43210")

        self.assertEqual(client.base_url, "http://127.0.0.1:43210")
        self.assertIsNotNone(client.templates)
        self.assertIsNotNone(client.executions)
        self.assertIsNotNone(client.batch)
        self.assertIsNotNone(client.batch_runs)
        self.assertIsNotNone(client.yolo)
        self.assertIsNotNone(client.yolo_runs)
        self.assertIsNotNone(client.sandboxes)
        self.assertIsNotNone(client.snapshots)
        self.assertIsNotNone(client.pools)


class ClientMethodsTest(unittest.IsolatedAsyncioTestCase):
    async def test_template_and_execution_methods(self) -> None:
        from void_control import VoidControlClient

        responses = [
            {
                "templates": [
                    {
                        "id": "benchmark-runner-python",
                        "name": "Benchmark Runner Python",
                        "execution_kind": "execution",
                        "description": "Compare multiple Python benchmark candidates in one swarm execution.",
                    }
                ]
            },
            {
                "template": {
                    "id": "benchmark-runner-python",
                    "name": "Benchmark Runner Python",
                    "execution_kind": "execution",
                    "description": "Compare multiple Python benchmark candidates in one swarm execution.",
                },
                "inputs": {
                    "goal": {"type": "string", "required": True, "description": "Goal"},
                    "snapshot": {"type": "string", "required": False, "description": "Snapshot"},
                },
                "defaults": {
                    "workflow_template": "examples/runtime-templates/transform_optimizer_agent.yaml"
                },
                "compile": {"bindings": []},
            },
            {
                "template": {
                    "id": "benchmark-runner-python",
                    "execution_kind": "execution",
                },
                "inputs": {
                    "goal": "Compare transform benchmark candidates",
                    "provider": "claude",
                },
                "compiled": {
                    "goal": "Compare transform benchmark candidates",
                    "workflow_template": "examples/runtime-templates/transform_optimizer_agent.yaml",
                    "mode": "swarm",
                    "variation_source": "explicit",
                    "candidates_per_iteration": 3,
                    "candidate_overrides": [
                        {"sandbox.env.TRANSFORM_ROLE": "latency-baseline"},
                        {"sandbox.env.TRANSFORM_ROLE": "cache-locality"},
                        {"sandbox.env.TRANSFORM_ROLE": "max-throughput"},
                    ],
                    "overrides": {"sandbox.env.TRANSFORM_ROLE": "latency-baseline"},
                },
            },
            {
                "execution_id": "exec-benchmark-1",
                "template": {
                    "id": "benchmark-runner-python",
                    "execution_kind": "execution",
                },
                "status": "Pending",
                "goal": "Compare transform benchmark candidates",
            },
            {
                "execution": {
                    "execution_id": "exec-benchmark-1",
                    "goal": "Compare transform benchmark candidates",
                    "status": "Pending",
                },
                "progress": {},
                "result": {
                    "best_candidate_id": None,
                    "completed_iterations": 0,
                    "total_candidate_failures": 0,
                },
                "candidates": [],
            },
            {
                "execution": {
                    "execution_id": "exec-benchmark-1",
                    "goal": "Compare transform benchmark candidates",
                    "status": "Pending",
                },
                "progress": {},
                "result": {
                    "best_candidate_id": None,
                    "completed_iterations": 0,
                    "total_candidate_failures": 0,
                },
                "candidates": [],
            },
            {
                "execution": {
                    "execution_id": "exec-benchmark-1",
                    "goal": "Compare transform benchmark candidates",
                    "status": "Completed",
                },
                "progress": {},
                "result": {
                    "best_candidate_id": "candidate-2",
                    "completed_iterations": 1,
                    "total_candidate_failures": 0,
                },
                "candidates": [],
            },
        ]
        requests: list[tuple[str, str, str | None]] = []

        def handler(request: httpx.Request) -> httpx.Response:
            body = request.content.decode() if request.content else None
            requests.append((request.method, request.url.path, body))
            payload = responses.pop(0)
            return httpx.Response(200, json=payload)

        client = VoidControlClient(
            base_url="http://127.0.0.1:43210",
            transport=httpx.MockTransport(handler),
        )

        templates = await client.templates.list()
        template = await client.templates.get("benchmark-runner-python")
        dry_run = await client.templates.dry_run(
            "benchmark-runner-python",
            inputs={
                "goal": "Compare transform benchmark candidates",
                "provider": "claude",
            },
        )
        execution = await client.templates.execute(
            "benchmark-runner-python",
            inputs={
                "goal": "Compare transform benchmark candidates",
                "provider": "claude",
            },
        )
        detail = await client.executions.get("exec-benchmark-1")
        waited = await client.executions.wait(
            "exec-benchmark-1",
            poll_interval=0.0,
        )
        await client.aclose()

        self.assertEqual(templates[0].id, "benchmark-runner-python")
        self.assertEqual(template.id, "benchmark-runner-python")
        self.assertEqual(dry_run.compiled.candidates_per_iteration, 3)
        self.assertEqual(
            dry_run.compiled.candidate_overrides[2]["sandbox.env.TRANSFORM_ROLE"],
            "max-throughput",
        )
        self.assertEqual(execution.execution_id, "exec-benchmark-1")
        self.assertEqual(detail.execution.status, "Pending")
        self.assertEqual(waited.execution.status, "Completed")
        self.assertEqual(waited.result.best_candidate_id, "candidate-2")

        self.assertEqual(requests[0][:2], ("GET", "/v1/templates"))
        self.assertEqual(requests[1][:2], ("GET", "/v1/templates/benchmark-runner-python"))
        self.assertEqual(requests[2][:2], ("POST", "/v1/templates/benchmark-runner-python/dry-run"))
        self.assertEqual(requests[3][:2], ("POST", "/v1/templates/benchmark-runner-python/execute"))
        self.assertEqual(requests[4][:2], ("GET", "/v1/executions/exec-benchmark-1"))

    async def test_batch_and_yolo_methods(self) -> None:
        from void_control import VoidControlClient

        responses = [
            {
                "kind": "batch",
                "run_id": "exec-batch-1",
                "execution_id": "exec-batch-1",
                "compiled_primitive": "swarm",
                "status": "Pending",
                "goal": "repo-background-work",
            },
            {
                "kind": "batch",
                "run_id": "exec-batch-1",
                "execution": {
                    "execution_id": "exec-batch-1",
                    "goal": "repo-background-work",
                    "status": "Pending",
                },
                "progress": {},
                "result": {
                    "best_candidate_id": None,
                    "completed_iterations": 0,
                    "total_candidate_failures": 0,
                },
                "candidates": [],
            },
            {
                "kind": "batch",
                "run_id": "exec-batch-1",
                "execution": {
                    "execution_id": "exec-batch-1",
                    "goal": "repo-background-work",
                    "status": "Completed",
                },
                "progress": {},
                "result": {
                    "best_candidate_id": "candidate-2",
                    "completed_iterations": 1,
                    "total_candidate_failures": 0,
                },
                "candidates": [],
            },
            {
                "kind": "batch",
                "run_id": "exec-yolo-1",
                "execution_id": "exec-yolo-1",
                "compiled_primitive": "swarm",
                "status": "Pending",
                "goal": "run 1 background jobs",
            },
            {
                "kind": "batch",
                "run_id": "exec-yolo-1",
                "execution": {
                    "execution_id": "exec-yolo-1",
                    "goal": "run 1 background jobs",
                    "status": "Completed",
                },
                "progress": {},
                "result": {
                    "best_candidate_id": None,
                    "completed_iterations": 1,
                    "total_candidate_failures": 0,
                },
                "candidates": [],
            },
        ]
        requests: list[tuple[str, str, str | None]] = []

        def handler(request: httpx.Request) -> httpx.Response:
            body = request.content.decode() if request.content else None
            requests.append((request.method, request.url.path, body))
            payload = responses.pop(0)
            return httpx.Response(200, json=payload)

        client = VoidControlClient(
            base_url="http://127.0.0.1:43210",
            transport=httpx.MockTransport(handler),
        )

        batch_run = await client.batch.run(
            {
                "api_version": "v1",
                "kind": "batch",
                "worker": {"template": "examples/runtime-templates/warm_agent_basic.yaml"},
                "jobs": [{"prompt": "Fix failing auth tests"}],
            }
        )
        batch_detail = await client.batch_runs.get("exec-batch-1")
        waited_batch = await client.batch_runs.wait("exec-batch-1", poll_interval=0.0)
        yolo_run = await client.yolo.run(
            {
                "api_version": "v1",
                "kind": "yolo",
                "worker": {"template": "examples/runtime-templates/warm_agent_basic.yaml"},
                "jobs": [{"prompt": "Review migration safety"}],
            }
        )
        waited_yolo = await client.yolo_runs.wait("exec-yolo-1", poll_interval=0.0)
        await client.aclose()

        self.assertEqual(batch_run.kind, "batch")
        self.assertEqual(batch_run.run_id, "exec-batch-1")
        self.assertEqual(batch_detail.execution.execution_id, "exec-batch-1")
        self.assertEqual(waited_batch.execution.status, "Completed")
        self.assertEqual(yolo_run.run_id, "exec-yolo-1")
        self.assertEqual(waited_yolo.execution.status, "Completed")

        self.assertEqual(requests[0][:2], ("POST", "/v1/batch/run"))
        self.assertEqual(requests[1][:2], ("GET", "/v1/batch-runs/exec-batch-1"))
        self.assertEqual(requests[2][:2], ("GET", "/v1/batch-runs/exec-batch-1"))
        self.assertEqual(requests[3][:2], ("POST", "/v1/yolo/run"))
        self.assertEqual(requests[4][:2], ("GET", "/v1/yolo-runs/exec-yolo-1"))

    async def test_compute_methods(self) -> None:
        from void_control import VoidControlClient

        responses = [
            {
                "kind": "sandbox",
                "sandbox": {
                    "sandbox_id": "sbx-1",
                    "state": "running",
                    "image": "python:3.12-slim",
                    "cpus": 2,
                    "memory_mb": 2048,
                },
            },
            {
                "kind": "sandbox_list",
                "sandboxes": [
                    {
                        "sandbox_id": "sbx-1",
                        "state": "running",
                        "image": "python:3.12-slim",
                        "cpus": 2,
                        "memory_mb": 2048,
                    }
                ],
            },
            {
                "kind": "sandbox",
                "sandbox": {
                    "sandbox_id": "sbx-1",
                    "state": "running",
                    "image": "python:3.12-slim",
                    "cpus": 2,
                    "memory_mb": 2048,
                },
            },
            {
                "kind": "sandbox_exec",
                "result": {
                    "exit_code": 0,
                    "stdout": "hello\n",
                    "stderr": "",
                },
            },
            {
                "kind": "sandbox_deleted",
                "sandbox_id": "sbx-1",
            },
            {
                "kind": "snapshot",
                "snapshot": {
                    "snapshot_id": "snap-1",
                    "source_sandbox_id": "sbx-1",
                    "distribution": {
                        "mode": "cached",
                        "targets": ["node-a", "node-b"],
                    },
                },
            },
            {
                "kind": "snapshot_list",
                "snapshots": [
                    {
                        "snapshot_id": "snap-1",
                        "source_sandbox_id": "sbx-1",
                        "distribution": {
                            "mode": "cached",
                            "targets": ["node-a", "node-b"],
                        },
                    }
                ],
            },
            {
                "kind": "snapshot",
                "snapshot": {
                    "snapshot_id": "snap-1",
                    "source_sandbox_id": "sbx-1",
                    "distribution": {
                        "mode": "cached",
                        "targets": ["node-a", "node-b"],
                    },
                },
            },
            {
                "kind": "snapshot",
                "snapshot": {
                    "snapshot_id": "snap-1",
                    "source_sandbox_id": "sbx-1",
                    "distribution": {
                        "mode": "copy",
                        "targets": ["node-a", "node-c"],
                    },
                },
            },
            {
                "kind": "snapshot_deleted",
                "snapshot_id": "snap-1",
            },
            {
                "kind": "pool",
                "pool": {
                    "pool_id": "pool-1",
                    "sandbox_spec": {
                        "runtime": {
                            "image": "python:3.12-slim",
                            "cpus": 2,
                            "memory_mb": 2048,
                        }
                    },
                    "capacity": {
                        "warm": 5,
                        "max": 20,
                    },
                },
            },
            {
                "kind": "pool",
                "pool": {
                    "pool_id": "pool-1",
                    "sandbox_spec": {
                        "runtime": {
                            "image": "python:3.12-slim",
                            "cpus": 2,
                            "memory_mb": 2048,
                        }
                    },
                    "capacity": {
                        "warm": 5,
                        "max": 20,
                    },
                },
            },
            {
                "kind": "pool",
                "pool": {
                    "pool_id": "pool-1",
                    "sandbox_spec": {
                        "runtime": {
                            "image": "python:3.12-slim",
                            "cpus": 2,
                            "memory_mb": 2048,
                        }
                    },
                    "capacity": {
                        "warm": 8,
                        "max": 24,
                    },
                },
            },
        ]
        requests: list[tuple[str, str, str | None]] = []

        def handler(request: httpx.Request) -> httpx.Response:
            body = request.content.decode() if request.content else None
            requests.append((request.method, request.url.path, body))
            payload = responses.pop(0)
            return httpx.Response(200, json=payload)

        client = VoidControlClient(
            base_url="http://127.0.0.1:43210",
            transport=httpx.MockTransport(handler),
        )

        sandbox = await client.sandboxes.create(
            {
                "api_version": "v1",
                "kind": "sandbox",
                "runtime": {
                    "image": "python:3.12-slim",
                    "cpus": 2,
                    "memory_mb": 2048,
                },
            }
        )
        sandboxes = await client.sandboxes.list()
        fetched_sandbox = await client.sandboxes.get("sbx-1")
        exec_result = await client.sandboxes.exec(
            "sbx-1",
            {
                "kind": "command",
                "command": ["python3", "-c", "print('hello')"],
            },
        )
        deleted_sandbox = await client.sandboxes.delete("sbx-1")
        snapshot = await client.snapshots.create(
            {
                "api_version": "v1",
                "kind": "snapshot",
                "source": {"sandbox_id": "sbx-1"},
                "distribution": {
                    "mode": "cached",
                    "targets": ["node-a", "node-b"],
                },
            }
        )
        snapshots = await client.snapshots.list()
        fetched_snapshot = await client.snapshots.get("snap-1")
        replicated = await client.snapshots.replicate(
            "snap-1",
            {
                "mode": "copy",
                "targets": ["node-a", "node-c"],
            },
        )
        deleted_snapshot = await client.snapshots.delete("snap-1")
        pool = await client.pools.create(
            {
                "api_version": "v1",
                "kind": "sandbox_pool",
                "sandbox_spec": {
                    "runtime": {
                        "image": "python:3.12-slim",
                        "cpus": 2,
                        "memory_mb": 2048,
                    }
                },
                "capacity": {"warm": 5, "max": 20},
            }
        )
        fetched_pool = await client.pools.get("pool-1")
        scaled = await client.pools.scale("pool-1", {"warm": 8, "max": 24})
        await client.aclose()

        self.assertEqual(sandbox.sandbox_id, "sbx-1")
        self.assertEqual(sandboxes[0].state, "running")
        self.assertEqual(fetched_sandbox.image, "python:3.12-slim")
        self.assertEqual(exec_result.exit_code, 0)
        self.assertEqual(deleted_sandbox.kind, "sandbox_deleted")
        self.assertEqual(snapshot.snapshot_id, "snap-1")
        self.assertEqual(snapshots[0].snapshot_id, "snap-1")
        self.assertEqual(fetched_snapshot.source_sandbox_id, "sbx-1")
        self.assertEqual(replicated.distribution["mode"], "copy")
        self.assertEqual(deleted_snapshot.kind, "snapshot_deleted")
        self.assertEqual(pool.pool_id, "pool-1")
        self.assertEqual(fetched_pool.capacity["warm"], 5)
        self.assertEqual(scaled.capacity["warm"], 8)

        self.assertEqual(requests[0][:2], ("POST", "/v1/sandboxes"))
        self.assertEqual(requests[1][:2], ("GET", "/v1/sandboxes"))
        self.assertEqual(requests[2][:2], ("GET", "/v1/sandboxes/sbx-1"))
        self.assertEqual(requests[3][:2], ("POST", "/v1/sandboxes/sbx-1/exec"))
        self.assertEqual(requests[4][:2], ("DELETE", "/v1/sandboxes/sbx-1"))
        self.assertEqual(requests[5][:2], ("POST", "/v1/snapshots"))
        self.assertEqual(requests[6][:2], ("GET", "/v1/snapshots"))
        self.assertEqual(requests[7][:2], ("GET", "/v1/snapshots/snap-1"))
        self.assertEqual(requests[8][:2], ("POST", "/v1/snapshots/snap-1/replicate"))
        self.assertEqual(requests[9][:2], ("DELETE", "/v1/snapshots/snap-1"))
        self.assertEqual(requests[10][:2], ("POST", "/v1/pools"))
        self.assertEqual(requests[11][:2], ("GET", "/v1/pools/pool-1"))
        self.assertEqual(requests[12][:2], ("POST", "/v1/pools/pool-1/scale"))

    async def test_compute_methods_raise_bridge_error(self) -> None:
        from void_control import VoidControlClient
        from void_control.models import BridgeError

        responses = [
            (
                404,
                {
                    "message": "sandbox 'sbx-missing' not found",
                    "code": "SANDBOX_NOT_FOUND",
                    "retryable": False,
                },
            ),
            (
                404,
                {
                    "message": "snapshot 'snap-missing' not found",
                    "code": "SNAPSHOT_NOT_FOUND",
                    "retryable": False,
                },
            ),
            (
                503,
                {
                    "message": "pool controller unavailable",
                    "code": "POOL_UNAVAILABLE",
                    "retryable": True,
                },
            ),
        ]

        def handler(request: httpx.Request) -> httpx.Response:
            status, payload = responses.pop(0)
            return httpx.Response(status, json=payload)

        client = VoidControlClient(
            base_url="http://127.0.0.1:43210",
            transport=httpx.MockTransport(handler),
        )

        with self.assertRaises(BridgeError) as sandbox_err:
            await client.sandboxes.get("sbx-missing")
        self.assertEqual(str(sandbox_err.exception), "sandbox 'sbx-missing' not found")
        self.assertEqual(sandbox_err.exception.code, "SANDBOX_NOT_FOUND")
        self.assertFalse(sandbox_err.exception.retryable)

        with self.assertRaises(BridgeError) as snapshot_err:
            await client.snapshots.delete("snap-missing")
        self.assertEqual(str(snapshot_err.exception), "snapshot 'snap-missing' not found")
        self.assertEqual(snapshot_err.exception.code, "SNAPSHOT_NOT_FOUND")
        self.assertFalse(snapshot_err.exception.retryable)

        with self.assertRaises(BridgeError) as pool_err:
            await client.pools.scale("pool-1", {"warm": 8, "max": 24})
        self.assertEqual(str(pool_err.exception), "pool controller unavailable")
        self.assertEqual(pool_err.exception.code, "POOL_UNAVAILABLE")
        self.assertTrue(pool_err.exception.retryable)

        await client.aclose()


if __name__ == "__main__":
    unittest.main()
