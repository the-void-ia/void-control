package voidcontrol

type sandboxListResponse struct {
	Sandboxes []SandboxRecord `json:"sandboxes"`
}

type sandboxExecResponse struct {
	Result SandboxExecResult `json:"result"`
}

type SandboxesClient struct {
	client *Client
}

func (client *SandboxesClient) Create(spec map[string]any) (*SandboxRecord, error) {
	var response struct {
		Sandbox SandboxRecord `json:"sandbox"`
	}
	if err := client.client.postJSON("/v1/sandboxes", spec, &response); err != nil {
		return nil, err
	}
	return &response.Sandbox, nil
}

func (client *SandboxesClient) Get(sandboxID string) (*SandboxRecord, error) {
	var response struct {
		Sandbox SandboxRecord `json:"sandbox"`
	}
	if err := client.client.getJSON("/v1/sandboxes/"+sandboxID, &response); err != nil {
		return nil, err
	}
	return &response.Sandbox, nil
}

func (client *SandboxesClient) List() ([]SandboxRecord, error) {
	var response sandboxListResponse
	if err := client.client.getJSON("/v1/sandboxes", &response); err != nil {
		return nil, err
	}
	return response.Sandboxes, nil
}

func (client *SandboxesClient) Exec(sandboxID string, request map[string]any) (*SandboxExecResult, error) {
	var response sandboxExecResponse
	if err := client.client.postJSON("/v1/sandboxes/"+sandboxID+"/exec", request, &response); err != nil {
		return nil, err
	}
	return &response.Result, nil
}

func (client *SandboxesClient) Stop(sandboxID string) (*SandboxRecord, error) {
	var response struct {
		Sandbox SandboxRecord `json:"sandbox"`
	}
	if err := client.client.postJSON("/v1/sandboxes/"+sandboxID+"/stop", map[string]any{}, &response); err != nil {
		return nil, err
	}
	return &response.Sandbox, nil
}

func (client *SandboxesClient) Delete(sandboxID string) (*SandboxDeleteResult, error) {
	var response SandboxDeleteResult
	if err := client.client.deleteJSON("/v1/sandboxes/"+sandboxID, &response); err != nil {
		return nil, err
	}
	return &response, nil
}
