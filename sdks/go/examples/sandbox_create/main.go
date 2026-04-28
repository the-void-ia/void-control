package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strconv"

	voidcontrol "github.com/the-void-ia/void-control/sdks/go"
)

func main() {
	baseURL := getenvDefault("VOID_CONTROL_BASE_URL", "http://127.0.0.1:43210")
	image := getenvDefault("VOID_CONTROL_SANDBOX_IMAGE", "python:3.12-slim")
	cpus := getenvIntDefault("VOID_CONTROL_SANDBOX_CPUS", 2)
	memoryMB := getenvIntDefault("VOID_CONTROL_SANDBOX_MEMORY_MB", 2048)

	client := voidcontrol.NewClient(baseURL)
	sandbox, err := client.Sandboxes.Create(map[string]any{
		"api_version": "v1",
		"kind":        "sandbox",
		"runtime": map[string]any{
			"image":     image,
			"cpus":      cpus,
			"memory_mb": memoryMB,
		},
	})
	if err != nil {
		panic(err)
	}

	output, err := json.MarshalIndent(map[string]any{
		"sandbox_id": sandbox.SandboxID,
		"state":      sandbox.State,
		"image":      sandbox.Image,
		"cpus":       sandbox.CPUs,
		"memory_mb":  sandbox.MemoryMB,
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

func getenvIntDefault(key string, fallback int) int {
	value := os.Getenv(key)
	if value == "" {
		return fallback
	}
	parsed, err := strconv.Atoi(value)
	if err != nil {
		return fallback
	}
	return parsed
}
