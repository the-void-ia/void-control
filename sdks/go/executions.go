package voidcontrol

import "time"

type ExecutionsClient struct {
	client *Client
}

func (client *ExecutionsClient) Get(executionID string) (*ExecutionDetail, error) {
	var response ExecutionDetail
	if err := client.client.getJSON("/v1/executions/"+executionID, &response); err != nil {
		return nil, err
	}
	return &response, nil
}

func (client *ExecutionsClient) Wait(executionID string) (*ExecutionDetail, error) {
	for {
		detail, err := client.Get(executionID)
		if err != nil {
			return nil, err
		}
		if detail.Execution.Status == "Completed" || detail.Execution.Status == "Failed" || detail.Execution.Status == "Canceled" {
			return detail, nil
		}
		time.Sleep(10 * time.Millisecond)
	}
}
