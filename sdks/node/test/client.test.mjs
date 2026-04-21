import test from "node:test";
import assert from "node:assert/strict";

import { VoidControlClient } from "../src/index.js";

test("client exposes template and execution subclients", () => {
  const client = new VoidControlClient({ baseUrl: "http://127.0.0.1:43210" });

  assert.equal(client.baseUrl, "http://127.0.0.1:43210");
  assert.ok(client.templates);
  assert.ok(client.executions);
  assert.ok(client.batch);
  assert.ok(client.batchRuns);
  assert.ok(client.yolo);
  assert.ok(client.yoloRuns);
});

test("template and execution methods use the bridge API", async () => {
  const responses = [
    {
      templates: [
        {
          id: "benchmark-runner-python",
          name: "Benchmark Runner Python",
          execution_kind: "execution",
          description: "Compare multiple Python benchmark candidates in one swarm execution."
        }
      ]
    },
    {
      template: {
        id: "benchmark-runner-python",
        name: "Benchmark Runner Python",
        execution_kind: "execution",
        description: "Compare multiple Python benchmark candidates in one swarm execution."
      },
      inputs: {
        goal: { type: "string", required: true, description: "Goal" },
        snapshot: { type: "string", required: false, description: "Snapshot" }
      },
      defaults: {
        workflow_template: "examples/runtime-templates/transform_optimizer_agent.yaml"
      },
      compile: { bindings: [] }
    },
    {
      template: {
        id: "benchmark-runner-python",
        execution_kind: "execution"
      },
      inputs: {
        goal: "Compare transform benchmark candidates",
        provider: "claude"
      },
      compiled: {
        goal: "Compare transform benchmark candidates",
        workflow_template: "examples/runtime-templates/transform_optimizer_agent.yaml",
        mode: "swarm",
        variation_source: "explicit",
        candidates_per_iteration: 3,
        candidate_overrides: [
          { "sandbox.env.TRANSFORM_ROLE": "latency-baseline" },
          { "sandbox.env.TRANSFORM_ROLE": "cache-locality" },
          { "sandbox.env.TRANSFORM_ROLE": "max-throughput" }
        ],
        overrides: { "sandbox.env.TRANSFORM_ROLE": "latency-baseline" }
      }
    },
    {
      execution_id: "exec-benchmark-1",
      template: {
        id: "benchmark-runner-python",
        execution_kind: "execution"
      },
      status: "Pending",
      goal: "Compare transform benchmark candidates"
    },
    {
      execution: {
        execution_id: "exec-benchmark-1",
        goal: "Compare transform benchmark candidates",
        status: "Pending"
      },
      progress: {},
      result: {
        best_candidate_id: null,
        completed_iterations: 0,
        total_candidate_failures: 0
      },
      candidates: []
    },
    {
      execution: {
        execution_id: "exec-benchmark-1",
        goal: "Compare transform benchmark candidates",
        status: "Pending"
      },
      progress: {},
      result: {
        best_candidate_id: null,
        completed_iterations: 0,
        total_candidate_failures: 0
      },
      candidates: []
    },
    {
      execution: {
        execution_id: "exec-benchmark-1",
        goal: "Compare transform benchmark candidates",
        status: "Completed"
      },
      progress: {},
      result: {
        best_candidate_id: "candidate-2",
        completed_iterations: 1,
        total_candidate_failures: 0
      },
      candidates: []
    }
  ];
  const requests = [];
  const fetchImpl = async (url, init = {}) => {
    const body = init.body ?? null;
    requests.push({
      method: init.method ?? "GET",
      path: new URL(url).pathname,
      body
    });
    const payload = responses.shift();
    return new Response(JSON.stringify(payload), {
      status: 200,
      headers: { "content-type": "application/json" }
    });
  };

  const client = new VoidControlClient({
    baseUrl: "http://127.0.0.1:43210",
    fetchImpl
  });

  const templates = await client.templates.list();
  const template = await client.templates.get("benchmark-runner-python");
  const dryRun = await client.templates.dryRun("benchmark-runner-python", {
    inputs: {
      goal: "Compare transform benchmark candidates",
      provider: "claude"
    }
  });
  const execution = await client.templates.execute("benchmark-runner-python", {
    inputs: {
      goal: "Compare transform benchmark candidates",
      provider: "claude"
    }
  });
  const detail = await client.executions.get("exec-benchmark-1");
  const waited = await client.executions.wait("exec-benchmark-1", {
    pollIntervalMs: 0
  });

  assert.equal(templates[0].id, "benchmark-runner-python");
  assert.equal(template.id, "benchmark-runner-python");
  assert.equal(dryRun.compiled.candidatesPerIteration, 3);
  assert.equal(
    dryRun.compiled.candidateOverrides[2]["sandbox.env.TRANSFORM_ROLE"],
    "max-throughput"
  );
  assert.equal(execution.executionId, "exec-benchmark-1");
  assert.equal(detail.execution.status, "Pending");
  assert.equal(waited.execution.status, "Completed");
  assert.equal(waited.result.bestCandidateId, "candidate-2");

  assert.deepEqual(requests[0], {
    method: "GET",
    path: "/v1/templates",
    body: null
  });
  assert.deepEqual(requests[1], {
    method: "GET",
    path: "/v1/templates/benchmark-runner-python",
    body: null
  });
  assert.equal(requests[2].method, "POST");
  assert.equal(requests[2].path, "/v1/templates/benchmark-runner-python/dry-run");
  assert.equal(requests[3].path, "/v1/templates/benchmark-runner-python/execute");
  assert.equal(requests[4].path, "/v1/executions/exec-benchmark-1");
});

test("batch and yolo methods use the bridge API", async () => {
  const responses = [
    {
      kind: "batch",
      run_id: "exec-batch-1",
      execution_id: "exec-batch-1",
      compiled_primitive: "swarm",
      status: "Pending",
      goal: "repo-background-work"
    },
    {
      kind: "batch",
      run_id: "exec-batch-1",
      execution: {
        execution_id: "exec-batch-1",
        goal: "repo-background-work",
        status: "Pending"
      },
      progress: {},
      result: {
        best_candidate_id: null,
        completed_iterations: 0,
        total_candidate_failures: 0
      },
      candidates: []
    },
    {
      kind: "batch",
      run_id: "exec-batch-1",
      execution: {
        execution_id: "exec-batch-1",
        goal: "repo-background-work",
        status: "Completed"
      },
      progress: {},
      result: {
        best_candidate_id: "candidate-2",
        completed_iterations: 1,
        total_candidate_failures: 0
      },
      candidates: []
    },
    {
      kind: "batch",
      run_id: "exec-yolo-1",
      execution_id: "exec-yolo-1",
      compiled_primitive: "swarm",
      status: "Pending",
      goal: "run 1 background jobs"
    },
    {
      kind: "batch",
      run_id: "exec-yolo-1",
      execution: {
        execution_id: "exec-yolo-1",
        goal: "run 1 background jobs",
        status: "Completed"
      },
      progress: {},
      result: {
        best_candidate_id: null,
        completed_iterations: 1,
        total_candidate_failures: 0
      },
      candidates: []
    }
  ];
  const requests = [];
  const fetchImpl = async (url, init = {}) => {
    const body = init.body ?? null;
    requests.push({
      method: init.method ?? "GET",
      path: new URL(url).pathname,
      body
    });
    const payload = responses.shift();
    return new Response(JSON.stringify(payload), {
      status: 200,
      headers: { "content-type": "application/json" }
    });
  };

  const client = new VoidControlClient({
    baseUrl: "http://127.0.0.1:43210",
    fetchImpl
  });

  const batchRun = await client.batch.run({
    api_version: "v1",
    kind: "batch",
    worker: { template: "examples/runtime-templates/warm_agent_basic.yaml" },
    jobs: [{ prompt: "Fix failing auth tests" }]
  });
  const batchDetail = await client.batchRuns.get("exec-batch-1");
  const waitedBatch = await client.batchRuns.wait("exec-batch-1", {
    pollIntervalMs: 0
  });
  const yoloRun = await client.yolo.run({
    api_version: "v1",
    kind: "yolo",
    worker: { template: "examples/runtime-templates/warm_agent_basic.yaml" },
    jobs: [{ prompt: "Review migration safety" }]
  });
  const waitedYolo = await client.yoloRuns.wait("exec-yolo-1", {
    pollIntervalMs: 0
  });

  assert.equal(batchRun.kind, "batch");
  assert.equal(batchRun.runId, "exec-batch-1");
  assert.equal(batchDetail.execution.executionId, "exec-batch-1");
  assert.equal(waitedBatch.execution.status, "Completed");
  assert.equal(yoloRun.runId, "exec-yolo-1");
  assert.equal(waitedYolo.execution.status, "Completed");

  assert.equal(requests[0].path, "/v1/batch/run");
  assert.equal(requests[1].path, "/v1/batch-runs/exec-batch-1");
  assert.equal(requests[2].path, "/v1/batch-runs/exec-batch-1");
  assert.equal(requests[3].path, "/v1/yolo/run");
  assert.equal(requests[4].path, "/v1/yolo-runs/exec-yolo-1");
});
