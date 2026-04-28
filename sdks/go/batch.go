package voidcontrol

import "time"

type BatchClient struct {
	client    *Client
	routeBase string
}

type BatchRunsClient struct {
	client    *Client
	routeBase string
}

func (client *BatchClient) Run(spec map[string]any) (*BatchRunResult, error) {
	var response BatchRunResult
	if err := client.client.postJSON(client.routeBase+"/run", spec, &response); err != nil {
		return nil, err
	}
	return &response, nil
}

func (client *BatchRunsClient) Get(runID string) (*BatchRunDetail, error) {
	var response BatchRunDetail
	if err := client.client.getJSON(client.routeBase+"-runs/"+runID, &response); err != nil {
		return nil, err
	}
	return &response, nil
}

func (client *BatchRunsClient) Wait(runID string) (*BatchRunDetail, error) {
	for {
		detail, err := client.Get(runID)
		if err != nil {
			return nil, err
		}
		if detail.Execution.Status == "Completed" || detail.Execution.Status == "Failed" || detail.Execution.Status == "Canceled" {
			return detail, nil
		}
		time.Sleep(10 * time.Millisecond)
	}
}
