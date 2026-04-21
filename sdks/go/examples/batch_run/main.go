package main

import (
	"encoding/json"
	"fmt"
	"os"

	voidcontrol "github.com/the-void-ia/void-control/sdks/go"
)

func main() {
	baseURL := getenvDefault("VOID_CONTROL_BASE_URL", "http://127.0.0.1:43210")
	route := getenvDefault("VOID_CONTROL_BATCH_ROUTE", "batch")
	spec := map[string]any{
		"api_version": "v1",
		"kind":        route,
		"worker": map[string]any{
			"template": "examples/runtime-templates/warm_agent_basic.yaml",
		},
		"jobs": []map[string]any{
			{
				"prompt": getenvDefault("VOID_CONTROL_BATCH_PROMPT_ONE", "Fix failing auth tests"),
			},
			{
				"prompt": getenvDefault("VOID_CONTROL_BATCH_PROMPT_TWO", "Improve retry logging"),
			},
		},
	}

	client := voidcontrol.NewClient(baseURL)
	runner := client.Batch
	runs := client.BatchRuns
	if route == "yolo" {
		runner = client.Yolo
		runs = client.YoloRuns
	}

	started, err := runner.Run(spec)
	if err != nil {
		panic(err)
	}
	detail, err := runs.Wait(started.RunID)
	if err != nil {
		panic(err)
	}

	output, err := json.MarshalIndent(map[string]any{
		"route":        route,
		"run_id":       started.RunID,
		"kind":         started.Kind,
		"status":       detail.Execution.Status,
		"execution_id": detail.Execution.ExecutionID,
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
