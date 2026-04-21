package voidcontrol

type PoolsClient struct {
	client *Client
}

func (client *PoolsClient) Create(spec map[string]any) (*PoolRecord, error) {
	var response struct {
		Pool PoolRecord `json:"pool"`
	}
	if err := client.client.postJSON("/v1/pools", spec, &response); err != nil {
		return nil, err
	}
	return &response.Pool, nil
}

func (client *PoolsClient) Get(poolID string) (*PoolRecord, error) {
	var response struct {
		Pool PoolRecord `json:"pool"`
	}
	if err := client.client.getJSON("/v1/pools/"+poolID, &response); err != nil {
		return nil, err
	}
	return &response.Pool, nil
}

func (client *PoolsClient) Scale(poolID string, request map[string]any) (*PoolRecord, error) {
	var response struct {
		Pool PoolRecord `json:"pool"`
	}
	if err := client.client.postJSON("/v1/pools/"+poolID+"/scale", request, &response); err != nil {
		return nil, err
	}
	return &response.Pool, nil
}
