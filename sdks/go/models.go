package voidcontrol

type BridgeError struct {
	Message   string `json:"message"`
	Code      string `json:"code"`
	Retryable bool   `json:"retryable"`
}

func (err *BridgeError) Error() string {
	return err.Message
}

type TemplateSummary struct {
	ID            string `json:"id"`
	Name          string `json:"name"`
	ExecutionKind string `json:"execution_kind"`
	Description   string `json:"description"`
}

type TemplateDetail struct {
	ID               string
	Name             string
	ExecutionKind    string
	Description      string
	Inputs           map[string]any
	WorkflowTemplate string
	Bindings         []map[string]any
}

type CompiledPreview struct {
	Goal                   string              `json:"goal"`
	WorkflowTemplate       string              `json:"workflow_template"`
	Mode                   string              `json:"mode"`
	VariationSource        string              `json:"variation_source"`
	CandidatesPerIteration int                 `json:"candidates_per_iteration"`
	CandidateOverrides     []map[string]string `json:"candidate_overrides"`
	Overrides              map[string]string   `json:"overrides"`
}

type TemplateDryRunResult struct {
	Template struct {
		ID            string `json:"id"`
		ExecutionKind string `json:"execution_kind"`
	} `json:"template"`
	Inputs   map[string]any  `json:"inputs"`
	Compiled CompiledPreview `json:"compiled"`
}

type TemplateExecutionResult struct {
	ExecutionID string `json:"execution_id"`
	Template    struct {
		ID            string `json:"id"`
		ExecutionKind string `json:"execution_kind"`
	} `json:"template"`
	Status string `json:"status"`
	Goal   string `json:"goal"`
}

type ExecutionRecord struct {
	ExecutionID string `json:"execution_id"`
	Goal        string `json:"goal"`
	Status      string `json:"status"`
}

type ExecutionResult struct {
	BestCandidateID        string `json:"best_candidate_id"`
	CompletedIterations    int    `json:"completed_iterations"`
	TotalCandidateFailures int    `json:"total_candidate_failures"`
}

type ExecutionDetail struct {
	Execution  ExecutionRecord `json:"execution"`
	Progress   map[string]any  `json:"progress"`
	Result     ExecutionResult `json:"result"`
	Candidates []any           `json:"candidates"`
}

type BatchRunResult struct {
	Kind              string `json:"kind"`
	RunID             string `json:"run_id"`
	ExecutionID       string `json:"execution_id"`
	CompiledPrimitive string `json:"compiled_primitive"`
	Status            string `json:"status"`
	Goal              string `json:"goal"`
}

type BatchRunDetail struct {
	Kind       string          `json:"kind"`
	RunID      string          `json:"run_id"`
	Execution  ExecutionRecord `json:"execution"`
	Progress   map[string]any  `json:"progress"`
	Result     ExecutionResult `json:"result"`
	Candidates []any           `json:"candidates"`
}

type SandboxRecord struct {
	SandboxID string `json:"sandbox_id"`
	State     string `json:"state"`
	Image     string `json:"image"`
	CPUs      int    `json:"cpus"`
	MemoryMB  int    `json:"memory_mb"`
}

type SandboxExecResult struct {
	ExitCode int    `json:"exit_code"`
	Stdout   string `json:"stdout"`
	Stderr   string `json:"stderr"`
}

type SnapshotRecord struct {
	SnapshotID      string         `json:"snapshot_id"`
	SourceSandboxID string         `json:"source_sandbox_id"`
	Distribution    map[string]any `json:"distribution"`
}

type PoolRecord struct {
	PoolID      string         `json:"pool_id"`
	SandboxSpec map[string]any `json:"sandbox_spec"`
	Capacity    map[string]any `json:"capacity"`
}
