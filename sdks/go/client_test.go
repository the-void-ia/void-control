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

type roundTripFunc func(*http.Request) (*http.Response, error)

func (fn roundTripFunc) RoundTrip(r *http.Request) (*http.Response, error) {
	return fn(r)
}
