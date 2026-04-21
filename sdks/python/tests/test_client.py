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


if __name__ == "__main__":
    unittest.main()
