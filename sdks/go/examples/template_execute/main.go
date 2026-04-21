package main

import (
	"encoding/json"
	"fmt"
	"os"

	voidcontrol "github.com/the-void-ia/void-control/sdks/go"
)

func main() {
	baseURL := getenvDefault("VOID_CONTROL_BASE_URL", "http://127.0.0.1:43210")
	templateID := getenvDefault("VOID_CONTROL_TEMPLATE_ID", "benchmark-runner-python")
	inputs := map[string]any{
		"goal":     getenvDefault("VOID_CONTROL_TEMPLATE_GOAL", "Compare transform benchmark candidates"),
		"provider": getenvDefault("VOID_CONTROL_TEMPLATE_PROVIDER", "claude"),
	}
	if snapshot := os.Getenv("VOID_CONTROL_TEMPLATE_SNAPSHOT"); snapshot != "" {
		inputs["snapshot"] = snapshot
	}

	client := voidcontrol.NewClient(baseURL)
	execution, err := client.Templates.Execute(templateID, map[string]any{"inputs": inputs})
	if err != nil {
		panic(err)
	}
	detail, err := client.Executions.Wait(execution.ExecutionID)
	if err != nil {
		panic(err)
	}

	output, err := json.MarshalIndent(map[string]any{
		"template_id":              templateID,
		"execution_id":             execution.ExecutionID,
		"status":                   detail.Execution.Status,
		"best_candidate_id":        detail.Result.BestCandidateID,
		"completed_iterations":     detail.Result.CompletedIterations,
		"total_candidate_failures": detail.Result.TotalCandidateFailures,
	}, "", "  ")
	if err != nil {
		panic(err)
	}
	fmt.Println(string(output))
}

func getenvDefault(key string, fallback string) string {
	value := os.Getenv(key)
	if value == "" {
		return fallback
	}
	return value
}
