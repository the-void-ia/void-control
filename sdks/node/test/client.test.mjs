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
  assert.ok(client.sandboxes);
  assert.ok(client.snapshots);
  assert.ok(client.pools);
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

test("compute methods use the bridge API", async () => {
  const responses = [
    {
      kind: "sandbox",
      sandbox: {
        sandbox_id: "sbx-1",
        state: "running",
        image: "python:3.12-slim",
        cpus: 2,
        memory_mb: 2048
      }
    },
    {
      kind: "sandbox_list",
      sandboxes: [
        {
          sandbox_id: "sbx-1",
          state: "running",
          image: "python:3.12-slim",
          cpus: 2,
          memory_mb: 2048
        }
      ]
    },
    {
      kind: "sandbox",
      sandbox: {
        sandbox_id: "sbx-1",
        state: "running",
        image: "python:3.12-slim",
        cpus: 2,
        memory_mb: 2048
      }
    },
    {
      kind: "sandbox_exec",
      result: {
        exit_code: 0,
        stdout: "hello\n",
        stderr: ""
      }
    },
    {
      kind: "sandbox_deleted",
      sandbox_id: "sbx-1"
    },
    {
      kind: "snapshot",
      snapshot: {
        snapshot_id: "snap-1",
        source_sandbox_id: "sbx-1",
        distribution: {
          mode: "cached",
          targets: ["node-a", "node-b"]
        }
      }
    },
    {
      kind: "snapshot_list",
      snapshots: [
        {
          snapshot_id: "snap-1",
          source_sandbox_id: "sbx-1",
          distribution: {
            mode: "cached",
            targets: ["node-a", "node-b"]
          }
        }
      ]
    },
    {
      kind: "snapshot",
      snapshot: {
        snapshot_id: "snap-1",
        source_sandbox_id: "sbx-1",
        distribution: {
          mode: "cached",
          targets: ["node-a", "node-b"]
        }
      }
    },
    {
      kind: "snapshot_deleted",
      snapshot_id: "snap-1"
    },
    {
      kind: "snapshot",
      snapshot: {
        snapshot_id: "snap-1",
        source_sandbox_id: "sbx-1",
        distribution: {
          mode: "copy",
          targets: ["node-a", "node-c"]
        }
      }
    },
    {
      kind: "pool",
      pool: {
        pool_id: "pool-1",
        sandbox_spec: {
          runtime: {
            image: "python:3.12-slim",
            cpus: 2,
            memory_mb: 2048
          }
        },
        capacity: {
          warm: 5,
          max: 20
        }
      }
    },
    {
      kind: "pool",
      pool: {
        pool_id: "pool-1",
        sandbox_spec: {
          runtime: {
            image: "python:3.12-slim",
            cpus: 2,
            memory_mb: 2048
          }
        },
        capacity: {
          warm: 5,
          max: 20
        }
      }
    },
    {
      kind: "pool",
      pool: {
        pool_id: "pool-1",
        sandbox_spec: {
          runtime: {
            image: "python:3.12-slim",
            cpus: 2,
            memory_mb: 2048
          }
        },
        capacity: {
          warm: 8,
          max: 24
        }
      }
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

  const sandbox = await client.sandboxes.create({
    api_version: "v1",
    kind: "sandbox",
    runtime: {
      image: "python:3.12-slim",
      cpus: 2,
      memory_mb: 2048
    }
  });
  const sandboxes = await client.sandboxes.list();
  const fetchedSandbox = await client.sandboxes.get("sbx-1");
  const execResult = await client.sandboxes.exec("sbx-1", {
    kind: "command",
    command: ["python3", "-c", "print('hello')"]
  });
  const deletedSandbox = await client.sandboxes.delete("sbx-1");
  const snapshot = await client.snapshots.create({
    api_version: "v1",
    kind: "snapshot",
    source: { sandbox_id: "sbx-1" },
    distribution: {
      mode: "cached",
      targets: ["node-a", "node-b"]
    }
  });
  const snapshots = await client.snapshots.list();
  const fetchedSnapshot = await client.snapshots.get("snap-1");
  const deletedSnapshot = await client.snapshots.delete("snap-1");
  const replicated = await client.snapshots.replicate("snap-1", {
    mode: "copy",
    targets: ["node-a", "node-c"]
  });
  const pool = await client.pools.create({
    api_version: "v1",
    kind: "sandbox_pool",
    sandbox_spec: {
      runtime: {
        image: "python:3.12-slim",
        cpus: 2,
        memory_mb: 2048
      }
    },
    capacity: {
      warm: 5,
      max: 20
    }
  });
  const fetchedPool = await client.pools.get("pool-1");
  const scaled = await client.pools.scale("pool-1", {
    warm: 8,
    max: 24
  });

  assert.equal(sandbox.sandboxId, "sbx-1");
  assert.equal(sandboxes[0].state, "running");
  assert.equal(fetchedSandbox.image, "python:3.12-slim");
  assert.equal(execResult.exitCode, 0);
  assert.equal(deletedSandbox.kind, "sandbox_deleted");
  assert.equal(deletedSandbox.sandboxId, "sbx-1");
  assert.equal(snapshot.snapshotId, "snap-1");
  assert.equal(snapshots[0].snapshotId, "snap-1");
  assert.equal(fetchedSnapshot.sourceSandboxId, "sbx-1");
  assert.equal(deletedSnapshot.kind, "snapshot_deleted");
  assert.equal(deletedSnapshot.snapshotId, "snap-1");
  assert.equal(replicated.distribution.mode, "copy");
  assert.equal(pool.poolId, "pool-1");
  assert.equal(fetchedPool.capacity.warm, 5);
  assert.equal(scaled.capacity.warm, 8);

  assert.equal(requests[0].path, "/v1/sandboxes");
  assert.equal(requests[1].path, "/v1/sandboxes");
  assert.equal(requests[2].path, "/v1/sandboxes/sbx-1");
  assert.equal(requests[3].path, "/v1/sandboxes/sbx-1/exec");
  assert.equal(requests[4].path, "/v1/sandboxes/sbx-1");
  assert.equal(requests[5].path, "/v1/snapshots");
  assert.equal(requests[6].path, "/v1/snapshots");
  assert.equal(requests[7].path, "/v1/snapshots/snap-1");
  assert.equal(requests[8].path, "/v1/snapshots/snap-1");
  assert.equal(requests[9].path, "/v1/snapshots/snap-1/replicate");
  assert.equal(requests[10].path, "/v1/pools");
  assert.equal(requests[11].path, "/v1/pools/pool-1");
  assert.equal(requests[12].path, "/v1/pools/pool-1/scale");
});
