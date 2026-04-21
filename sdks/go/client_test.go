package voidcontrol

import (
	"bytes"
	"encoding/json"
	"io"
	"net/http"
	"testing"
)

func TestClientExposesTemplateAndExecutionClients(t *testing.T) {
	client := NewClient("http://127.0.0.1:43210")

	if client.BaseURL != "http://127.0.0.1:43210" {
		t.Fatalf("BaseURL = %q", client.BaseURL)
	}
	if client.Templates == nil {
		t.Fatalf("Templates client should be initialized")
	}
	if client.Executions == nil {
		t.Fatalf("Executions client should be initialized")
	}
	if client.Batch == nil {
		t.Fatalf("Batch client should be initialized")
	}
	if client.BatchRuns == nil {
		t.Fatalf("BatchRuns client should be initialized")
	}
	if client.Yolo == nil {
		t.Fatalf("Yolo client should be initialized")
	}
	if client.YoloRuns == nil {
		t.Fatalf("YoloRuns client should be initialized")
	}
	if client.Sandboxes == nil {
		t.Fatalf("Sandboxes client should be initialized")
	}
	if client.Snapshots == nil {
		t.Fatalf("Snapshots client should be initialized")
	}
	if client.Pools == nil {
		t.Fatalf("Pools client should be initialized")
	}
}

func TestTemplateAndExecutionMethods(t *testing.T) {
	responses := []map[string]any{
		{
			"templates": []map[string]any{
				{
					"id":             "benchmark-runner-python",
					"name":           "Benchmark Runner Python",
					"execution_kind": "execution",
					"description":    "Compare multiple Python benchmark candidates in one swarm execution.",
				},
			},
		},
		{
			"template": map[string]any{
				"id":             "benchmark-runner-python",
				"name":           "Benchmark Runner Python",
				"execution_kind": "execution",
				"description":    "Compare multiple Python benchmark candidates in one swarm execution.",
			},
			"inputs": map[string]any{
				"goal":     map[string]any{"type": "string", "required": true},
				"snapshot": map[string]any{"type": "string", "required": false},
			},
			"defaults": map[string]any{
				"workflow_template": "examples/runtime-templates/transform_optimizer_agent.yaml",
			},
			"compile": map[string]any{"bindings": []any{}},
		},
		{
			"template": map[string]any{
				"id":             "benchmark-runner-python",
				"execution_kind": "execution",
			},
			"inputs": map[string]any{
				"goal":     "Compare transform benchmark candidates",
				"provider": "claude",
			},
			"compiled": map[string]any{
				"goal":                     "Compare transform benchmark candidates",
				"workflow_template":        "examples/runtime-templates/transform_optimizer_agent.yaml",
				"mode":                     "swarm",
				"variation_source":         "explicit",
				"candidates_per_iteration": float64(3),
				"candidate_overrides": []map[string]any{
					{"sandbox.env.TRANSFORM_ROLE": "latency-baseline"},
					{"sandbox.env.TRANSFORM_ROLE": "cache-locality"},
					{"sandbox.env.TRANSFORM_ROLE": "max-throughput"},
				},
				"overrides": map[string]any{
					"sandbox.env.TRANSFORM_ROLE": "latency-baseline",
				},
			},
		},
		{
			"execution_id": "exec-benchmark-1",
			"template": map[string]any{
				"id":             "benchmark-runner-python",
				"execution_kind": "execution",
			},
			"status": "Pending",
			"goal":   "Compare transform benchmark candidates",
		},
		{
			"execution": map[string]any{
				"execution_id": "exec-benchmark-1",
				"goal":         "Compare transform benchmark candidates",
				"status":       "Pending",
			},
			"progress":   map[string]any{},
			"result":     map[string]any{"best_candidate_id": nil, "completed_iterations": float64(0), "total_candidate_failures": float64(0)},
			"candidates": []any{},
		},
		{
			"execution": map[string]any{
				"execution_id": "exec-benchmark-1",
				"goal":         "Compare transform benchmark candidates",
				"status":       "Pending",
			},
			"progress":   map[string]any{},
			"result":     map[string]any{"best_candidate_id": nil, "completed_iterations": float64(0), "total_candidate_failures": float64(0)},
			"candidates": []any{},
		},
		{
			"execution": map[string]any{
				"execution_id": "exec-benchmark-1",
				"goal":         "Compare transform benchmark candidates",
				"status":       "Completed",
			},
			"progress":   map[string]any{},
			"result":     map[string]any{"best_candidate_id": "candidate-2", "completed_iterations": float64(1), "total_candidate_failures": float64(0)},
			"candidates": []any{},
		},
	}
	requests := make([]string, 0, len(responses))

	client := NewClient("http://void-control.test")
	client.HTTPClient = &http.Client{
		Transport: roundTripFunc(func(r *http.Request) (*http.Response, error) {
			requests = append(requests, r.Method+" "+r.URL.Path)
			if len(responses) == 0 {
				t.Fatalf("received unexpected request %s %s", r.Method, r.URL.Path)
			}
			body, err := json.Marshal(responses[0])
			if err != nil {
				t.Fatalf("marshal response: %v", err)
			}
			responses = responses[1:]
			return &http.Response{
				StatusCode: http.StatusOK,
				Header:     http.Header{"Content-Type": []string{"application/json"}},
				Body:       io.NopCloser(bytes.NewReader(body)),
				Request:    r,
			}, nil
		}),
	}

	templates, err := client.Templates.List()
	if err != nil {
		t.Fatalf("Templates.List: %v", err)
	}
	template, err := client.Templates.Get("benchmark-runner-python")
	if err != nil {
		t.Fatalf("Templates.Get: %v", err)
	}
	dryRun, err := client.Templates.DryRun("benchmark-runner-python", map[string]any{
		"inputs": map[string]any{
			"goal":     "Compare transform benchmark candidates",
			"provider": "claude",
		},
	})
	if err != nil {
		t.Fatalf("Templates.DryRun: %v", err)
	}
	execution, err := client.Templates.Execute("benchmark-runner-python", map[string]any{
		"inputs": map[string]any{
			"goal":     "Compare transform benchmark candidates",
			"provider": "claude",
		},
	})
	if err != nil {
		t.Fatalf("Templates.Execute: %v", err)
	}
	detail, err := client.Executions.Get("exec-benchmark-1")
	if err != nil {
		t.Fatalf("Executions.Get: %v", err)
	}
	waited, err := client.Executions.Wait("exec-benchmark-1")
	if err != nil {
		t.Fatalf("Executions.Wait: %v", err)
	}

	if templates[0].ID != "benchmark-runner-python" {
		t.Fatalf("templates[0].ID = %q", templates[0].ID)
	}
	if template.ID != "benchmark-runner-python" {
		t.Fatalf("template.ID = %q", template.ID)
	}
	if dryRun.Compiled.CandidatesPerIteration != 3 {
		t.Fatalf("dryRun.Compiled.CandidatesPerIteration = %d", dryRun.Compiled.CandidatesPerIteration)
	}
	if dryRun.Compiled.CandidateOverrides[2]["sandbox.env.TRANSFORM_ROLE"] != "max-throughput" {
		t.Fatalf("unexpected candidate override: %#v", dryRun.Compiled.CandidateOverrides[2])
	}
	if execution.ExecutionID != "exec-benchmark-1" {
		t.Fatalf("execution.ExecutionID = %q", execution.ExecutionID)
	}
	if detail.Execution.Status != "Pending" {
		t.Fatalf("detail.Execution.Status = %q", detail.Execution.Status)
	}
	if waited.Execution.Status != "Completed" {
		t.Fatalf("waited.Execution.Status = %q", waited.Execution.Status)
	}
	if waited.Result.BestCandidateID != "candidate-2" {
		t.Fatalf("waited.Result.BestCandidateID = %q", waited.Result.BestCandidateID)
	}
	if len(requests) != 7 {
		t.Fatalf("len(requests) = %d", len(requests))
	}
}

func TestBatchAndYoloMethods(t *testing.T) {
	responses := []map[string]any{
		{
			"kind":               "batch",
			"run_id":             "exec-batch-1",
			"execution_id":       "exec-batch-1",
			"compiled_primitive": "swarm",
			"status":             "Pending",
			"goal":               "repo-background-work",
		},
		{
			"kind":   "batch",
			"run_id": "exec-batch-1",
			"execution": map[string]any{
				"execution_id": "exec-batch-1",
				"goal":         "repo-background-work",
				"status":       "Pending",
			},
			"progress":   map[string]any{},
			"result":     map[string]any{"best_candidate_id": nil, "completed_iterations": float64(0), "total_candidate_failures": float64(0)},
			"candidates": []any{},
		},
		{
			"kind":   "batch",
			"run_id": "exec-batch-1",
			"execution": map[string]any{
				"execution_id": "exec-batch-1",
				"goal":         "repo-background-work",
				"status":       "Completed",
			},
			"progress":   map[string]any{},
			"result":     map[string]any{"best_candidate_id": "candidate-2", "completed_iterations": float64(1), "total_candidate_failures": float64(0)},
			"candidates": []any{},
		},
		{
			"kind":               "batch",
			"run_id":             "exec-yolo-1",
			"execution_id":       "exec-yolo-1",
			"compiled_primitive": "swarm",
			"status":             "Pending",
			"goal":               "run 1 background jobs",
		},
		{
			"kind":   "batch",
			"run_id": "exec-yolo-1",
			"execution": map[string]any{
				"execution_id": "exec-yolo-1",
				"goal":         "run 1 background jobs",
				"status":       "Completed",
			},
			"progress":   map[string]any{},
			"result":     map[string]any{"best_candidate_id": nil, "completed_iterations": float64(1), "total_candidate_failures": float64(0)},
			"candidates": []any{},
		},
	}
	requests := make([]string, 0, len(responses))

	client := NewClient("http://void-control.test")
	client.HTTPClient = &http.Client{
		Transport: roundTripFunc(func(r *http.Request) (*http.Response, error) {
			requests = append(requests, r.Method+" "+r.URL.Path)
			if len(responses) == 0 {
				t.Fatalf("received unexpected request %s %s", r.Method, r.URL.Path)
			}
			body, err := json.Marshal(responses[0])
			if err != nil {
				t.Fatalf("marshal response: %v", err)
			}
			responses = responses[1:]
			return &http.Response{
				StatusCode: http.StatusOK,
				Header:     http.Header{"Content-Type": []string{"application/json"}},
				Body:       io.NopCloser(bytes.NewReader(body)),
				Request:    r,
			}, nil
		}),
	}

	batchRun, err := client.Batch.Run(map[string]any{
		"api_version": "v1",
		"kind":        "batch",
		"worker": map[string]any{
			"template": "examples/runtime-templates/warm_agent_basic.yaml",
		},
		"jobs": []map[string]any{
			{"prompt": "Fix failing auth tests"},
		},
	})
	if err != nil {
		t.Fatalf("Batch.Run: %v", err)
	}
	batchDetail, err := client.BatchRuns.Get("exec-batch-1")
	if err != nil {
		t.Fatalf("BatchRuns.Get: %v", err)
	}
	waitedBatch, err := client.BatchRuns.Wait("exec-batch-1")
	if err != nil {
		t.Fatalf("BatchRuns.Wait: %v", err)
	}
	yoloRun, err := client.Yolo.Run(map[string]any{
		"api_version": "v1",
		"kind":        "yolo",
		"worker": map[string]any{
			"template": "examples/runtime-templates/warm_agent_basic.yaml",
		},
		"jobs": []map[string]any{
			{"prompt": "Review migration safety"},
		},
	})
	if err != nil {
		t.Fatalf("Yolo.Run: %v", err)
	}
	waitedYolo, err := client.YoloRuns.Wait("exec-yolo-1")
	if err != nil {
		t.Fatalf("YoloRuns.Wait: %v", err)
	}

	if batchRun.Kind != "batch" {
		t.Fatalf("batchRun.Kind = %q", batchRun.Kind)
	}
	if batchRun.RunID != "exec-batch-1" {
		t.Fatalf("batchRun.RunID = %q", batchRun.RunID)
	}
	if batchDetail.Execution.ExecutionID != "exec-batch-1" {
		t.Fatalf("batchDetail.Execution.ExecutionID = %q", batchDetail.Execution.ExecutionID)
	}
	if waitedBatch.Execution.Status != "Completed" {
		t.Fatalf("waitedBatch.Execution.Status = %q", waitedBatch.Execution.Status)
	}
	if yoloRun.RunID != "exec-yolo-1" {
		t.Fatalf("yoloRun.RunID = %q", yoloRun.RunID)
	}
	if waitedYolo.Execution.Status != "Completed" {
		t.Fatalf("waitedYolo.Execution.Status = %q", waitedYolo.Execution.Status)
	}
	if len(requests) != 5 {
		t.Fatalf("len(requests) = %d", len(requests))
	}
}

func TestComputeMethods(t *testing.T) {
	responses := []map[string]any{
		{
			"kind": "sandbox",
			"sandbox": map[string]any{
				"sandbox_id": "sbx-1",
				"state":      "running",
				"image":      "python:3.12-slim",
				"cpus":       float64(2),
				"memory_mb":  float64(2048),
			},
		},
		{
			"kind": "sandbox_list",
			"sandboxes": []map[string]any{
				{
					"sandbox_id": "sbx-1",
					"state":      "running",
					"image":      "python:3.12-slim",
					"cpus":       float64(2),
					"memory_mb":  float64(2048),
				},
			},
		},
		{
			"kind": "sandbox",
			"sandbox": map[string]any{
				"sandbox_id": "sbx-1",
				"state":      "running",
				"image":      "python:3.12-slim",
				"cpus":       float64(2),
				"memory_mb":  float64(2048),
			},
		},
		{
			"kind": "sandbox_exec",
			"result": map[string]any{
				"exit_code": float64(0),
				"stdout":    "hello\n",
				"stderr":    "",
			},
		},
		{
			"kind":       "sandbox_deleted",
			"sandbox_id": "sbx-1",
		},
		{
			"kind": "snapshot",
			"snapshot": map[string]any{
				"snapshot_id":       "snap-1",
				"source_sandbox_id": "sbx-1",
				"distribution": map[string]any{
					"mode":    "cached",
					"targets": []string{"node-a", "node-b"},
				},
			},
		},
		{
			"kind": "snapshot_list",
			"snapshots": []map[string]any{
				{
					"snapshot_id":       "snap-1",
					"source_sandbox_id": "sbx-1",
					"distribution": map[string]any{
						"mode":    "cached",
						"targets": []string{"node-a", "node-b"},
					},
				},
			},
		},
		{
			"kind": "snapshot",
			"snapshot": map[string]any{
				"snapshot_id":       "snap-1",
				"source_sandbox_id": "sbx-1",
				"distribution": map[string]any{
					"mode":    "cached",
					"targets": []string{"node-a", "node-b"},
				},
			},
		},
		{
			"kind":        "snapshot_deleted",
			"snapshot_id": "snap-1",
		},
		{
			"kind": "snapshot",
			"snapshot": map[string]any{
				"snapshot_id":       "snap-1",
				"source_sandbox_id": "sbx-1",
				"distribution": map[string]any{
					"mode":    "copy",
					"targets": []string{"node-a", "node-c"},
				},
			},
		},
		{
			"kind": "pool",
			"pool": map[string]any{
				"pool_id": "pool-1",
				"sandbox_spec": map[string]any{
					"runtime": map[string]any{
						"image":     "python:3.12-slim",
						"cpus":      float64(2),
						"memory_mb": float64(2048),
					},
				},
				"capacity": map[string]any{
					"warm": float64(5),
					"max":  float64(20),
				},
			},
		},
		{
			"kind": "pool",
			"pool": map[string]any{
				"pool_id": "pool-1",
				"sandbox_spec": map[string]any{
					"runtime": map[string]any{
						"image":     "python:3.12-slim",
						"cpus":      float64(2),
						"memory_mb": float64(2048),
					},
				},
				"capacity": map[string]any{
					"warm": float64(5),
					"max":  float64(20),
				},
			},
		},
		{
			"kind": "pool",
			"pool": map[string]any{
				"pool_id": "pool-1",
				"sandbox_spec": map[string]any{
					"runtime": map[string]any{
						"image":     "python:3.12-slim",
						"cpus":      float64(2),
						"memory_mb": float64(2048),
					},
				},
				"capacity": map[string]any{
					"warm": float64(8),
					"max":  float64(24),
				},
			},
		},
	}
	requests := make([]string, 0, len(responses))

	client := NewClient("http://void-control.test")
	client.HTTPClient = &http.Client{
		Transport: roundTripFunc(func(r *http.Request) (*http.Response, error) {
			requests = append(requests, r.Method+" "+r.URL.Path)
			if len(responses) == 0 {
				t.Fatalf("received unexpected request %s %s", r.Method, r.URL.Path)
			}
			body, err := json.Marshal(responses[0])
			if err != nil {
				t.Fatalf("marshal response: %v", err)
			}
			responses = responses[1:]
			return &http.Response{
				StatusCode: http.StatusOK,
				Header:     http.Header{"Content-Type": []string{"application/json"}},
				Body:       io.NopCloser(bytes.NewReader(body)),
				Request:    r,
			}, nil
		}),
	}

	sandbox, err := client.Sandboxes.Create(map[string]any{
		"api_version": "v1",
		"kind":        "sandbox",
		"runtime": map[string]any{
			"image":     "python:3.12-slim",
			"cpus":      2,
			"memory_mb": 2048,
		},
	})
	if err != nil {
		t.Fatalf("Sandboxes.Create: %v", err)
	}
	sandboxes, err := client.Sandboxes.List()
	if err != nil {
		t.Fatalf("Sandboxes.List: %v", err)
	}
	fetchedSandbox, err := client.Sandboxes.Get("sbx-1")
	if err != nil {
		t.Fatalf("Sandboxes.Get: %v", err)
	}
	execResult, err := client.Sandboxes.Exec("sbx-1", map[string]any{
		"kind":    "command",
		"command": []string{"python3", "-c", "print('hello')"},
	})
	if err != nil {
		t.Fatalf("Sandboxes.Exec: %v", err)
	}
	deletedSandbox, err := client.Sandboxes.Delete("sbx-1")
	if err != nil {
		t.Fatalf("Sandboxes.Delete: %v", err)
	}
	snapshot, err := client.Snapshots.Create(map[string]any{
		"api_version": "v1",
		"kind":        "snapshot",
		"source": map[string]any{
			"sandbox_id": "sbx-1",
		},
		"distribution": map[string]any{
			"mode":    "cached",
			"targets": []string{"node-a", "node-b"},
		},
	})
	if err != nil {
		t.Fatalf("Snapshots.Create: %v", err)
	}
	snapshots, err := client.Snapshots.List()
	if err != nil {
		t.Fatalf("Snapshots.List: %v", err)
	}
	fetchedSnapshot, err := client.Snapshots.Get("snap-1")
	if err != nil {
		t.Fatalf("Snapshots.Get: %v", err)
	}
	deletedSnapshot, err := client.Snapshots.Delete("snap-1")
	if err != nil {
		t.Fatalf("Snapshots.Delete: %v", err)
	}
	replicated, err := client.Snapshots.Replicate("snap-1", map[string]any{
		"mode":    "copy",
		"targets": []string{"node-a", "node-c"},
	})
	if err != nil {
		t.Fatalf("Snapshots.Replicate: %v", err)
	}
	pool, err := client.Pools.Create(map[string]any{
		"api_version": "v1",
		"kind":        "sandbox_pool",
		"sandbox_spec": map[string]any{
			"runtime": map[string]any{
				"image":     "python:3.12-slim",
				"cpus":      2,
				"memory_mb": 2048,
			},
		},
		"capacity": map[string]any{
			"warm": 5,
			"max":  20,
		},
	})
	if err != nil {
		t.Fatalf("Pools.Create: %v", err)
	}
	fetchedPool, err := client.Pools.Get("pool-1")
	if err != nil {
		t.Fatalf("Pools.Get: %v", err)
	}
	scaled, err := client.Pools.Scale("pool-1", map[string]any{
		"warm": 8,
		"max":  24,
	})
	if err != nil {
		t.Fatalf("Pools.Scale: %v", err)
	}

	if sandbox.SandboxID != "sbx-1" {
		t.Fatalf("sandbox.SandboxID = %q", sandbox.SandboxID)
	}
	if sandboxes[0].State != "running" {
		t.Fatalf("sandboxes[0].State = %q", sandboxes[0].State)
	}
	if fetchedSandbox.Image != "python:3.12-slim" {
		t.Fatalf("fetchedSandbox.Image = %q", fetchedSandbox.Image)
	}
	if execResult.ExitCode != 0 {
		t.Fatalf("execResult.ExitCode = %d", execResult.ExitCode)
	}
	if deletedSandbox.Kind != "sandbox_deleted" {
		t.Fatalf("deletedSandbox.Kind = %q", deletedSandbox.Kind)
	}
	if deletedSandbox.SandboxID != "sbx-1" {
		t.Fatalf("deletedSandbox.SandboxID = %q", deletedSandbox.SandboxID)
	}
	if snapshot.SnapshotID != "snap-1" {
		t.Fatalf("snapshot.SnapshotID = %q", snapshot.SnapshotID)
	}
	if snapshots[0].SnapshotID != "snap-1" {
		t.Fatalf("snapshots[0].SnapshotID = %q", snapshots[0].SnapshotID)
	}
	if fetchedSnapshot.SourceSandboxID != "sbx-1" {
		t.Fatalf("fetchedSnapshot.SourceSandboxID = %q", fetchedSnapshot.SourceSandboxID)
	}
	if deletedSnapshot.Kind != "snapshot_deleted" {
		t.Fatalf("deletedSnapshot.Kind = %q", deletedSnapshot.Kind)
	}
	if deletedSnapshot.SnapshotID != "snap-1" {
		t.Fatalf("deletedSnapshot.SnapshotID = %q", deletedSnapshot.SnapshotID)
	}
	if replicated.Distribution["mode"] != "copy" {
		t.Fatalf("replicated.Distribution = %#v", replicated.Distribution)
	}
	if pool.PoolID != "pool-1" {
		t.Fatalf("pool.PoolID = %q", pool.PoolID)
	}
	if fetchedPool.Capacity["warm"] != float64(5) {
		t.Fatalf("fetchedPool.Capacity = %#v", fetchedPool.Capacity)
	}
	if scaled.Capacity["warm"] != float64(8) {
		t.Fatalf("scaled.Capacity = %#v", scaled.Capacity)
	}
	if len(requests) != 13 {
		t.Fatalf("len(requests) = %d", len(requests))
	}
}

type roundTripFunc func(*http.Request) (*http.Response, error)

func (fn roundTripFunc) RoundTrip(r *http.Request) (*http.Response, error) {
	return fn(r)
}
