package voidcontrol

import (
	"bytes"
	"encoding/json"
	"io"
	"net/http"
	"strings"
)

type Client struct {
	BaseURL    string
	HTTPClient *http.Client
	Templates  *TemplatesClient
	Executions *ExecutionsClient
	Batch      *BatchClient
	BatchRuns  *BatchRunsClient
	Yolo       *BatchClient
	YoloRuns   *BatchRunsClient
	Sandboxes  *SandboxesClient
	Snapshots  *SnapshotsClient
	Pools      *PoolsClient
}

func NewClient(baseURL string) *Client {
	client := &Client{
		BaseURL:    strings.TrimRight(baseURL, "/"),
		HTTPClient: http.DefaultClient,
	}
	client.Templates = &TemplatesClient{client: client}
	client.Executions = &ExecutionsClient{client: client}
	client.Batch = &BatchClient{client: client, routeBase: "/v1/batch"}
	client.BatchRuns = &BatchRunsClient{client: client, routeBase: "/v1/batch"}
	client.Yolo = &BatchClient{client: client, routeBase: "/v1/yolo"}
	client.YoloRuns = &BatchRunsClient{client: client, routeBase: "/v1/yolo"}
	client.Sandboxes = &SandboxesClient{client: client}
	client.Snapshots = &SnapshotsClient{client: client}
	client.Pools = &PoolsClient{client: client}
	return client
}

func (client *Client) getJSON(path string, out any) error {
	req, err := http.NewRequest(http.MethodGet, client.BaseURL+path, nil)
	if err != nil {
		return err
	}
	return client.do(req, out)
}

func (client *Client) postJSON(path string, payload any, out any) error {
	body, err := json.Marshal(payload)
	if err != nil {
		return err
	}
	req, err := http.NewRequest(http.MethodPost, client.BaseURL+path, bytes.NewReader(body))
	if err != nil {
		return err
	}
	req.Header.Set("Content-Type", "application/json")
	return client.do(req, out)
}

func (client *Client) deleteJSON(path string, out any) error {
	req, err := http.NewRequest(http.MethodDelete, client.BaseURL+path, nil)
	if err != nil {
		return err
	}
	return client.do(req, out)
}

func (client *Client) do(req *http.Request, out any) error {
	response, err := client.HTTPClient.Do(req)
	if err != nil {
		return err
	}
	defer response.Body.Close()

	body, err := io.ReadAll(response.Body)
	if err != nil {
		return err
	}
	if response.StatusCode >= 400 {
		var bridgeErr BridgeError
		if err := json.Unmarshal(body, &bridgeErr); err != nil {
			return err
		}
		return &bridgeErr
	}
	return json.Unmarshal(body, out)
}
