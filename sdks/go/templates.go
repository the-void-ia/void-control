package voidcontrol

type templateListResponse struct {
	Templates []TemplateSummary `json:"templates"`
}

type rawTemplateDetail struct {
	Template struct {
		ID            string `json:"id"`
		Name          string `json:"name"`
		ExecutionKind string `json:"execution_kind"`
		Description   string `json:"description"`
	} `json:"template"`
	Inputs   map[string]any `json:"inputs"`
	Defaults struct {
		WorkflowTemplate string `json:"workflow_template"`
	} `json:"defaults"`
	Compile struct {
		Bindings []map[string]any `json:"bindings"`
	} `json:"compile"`
}

type TemplatesClient struct {
	client *Client
}

func (client *TemplatesClient) List() ([]TemplateSummary, error) {
	var response templateListResponse
	if err := client.client.getJSON("/v1/templates", &response); err != nil {
		return nil, err
	}
	return response.Templates, nil
}

func (client *TemplatesClient) Get(templateID string) (*TemplateDetail, error) {
	var raw rawTemplateDetail
	if err := client.client.getJSON("/v1/templates/"+templateID, &raw); err != nil {
		return nil, err
	}
	return &TemplateDetail{
		ID:               raw.Template.ID,
		Name:             raw.Template.Name,
		ExecutionKind:    raw.Template.ExecutionKind,
		Description:      raw.Template.Description,
		Inputs:           raw.Inputs,
		WorkflowTemplate: raw.Defaults.WorkflowTemplate,
		Bindings:         raw.Compile.Bindings,
	}, nil
}

func (client *TemplatesClient) DryRun(templateID string, request map[string]any) (*TemplateDryRunResult, error) {
	var response TemplateDryRunResult
	if err := client.client.postJSON("/v1/templates/"+templateID+"/dry-run", request, &response); err != nil {
		return nil, err
	}
	return &response, nil
}

func (client *TemplatesClient) Execute(templateID string, request map[string]any) (*TemplateExecutionResult, error) {
	var response TemplateExecutionResult
	if err := client.client.postJSON("/v1/templates/"+templateID+"/execute", request, &response); err != nil {
		return nil, err
	}
	return &response, nil
}
