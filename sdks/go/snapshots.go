package voidcontrol

type snapshotListResponse struct {
	Snapshots []SnapshotRecord `json:"snapshots"`
}

type SnapshotsClient struct {
	client *Client
}

func (client *SnapshotsClient) Create(spec map[string]any) (*SnapshotRecord, error) {
	var response struct {
		Snapshot SnapshotRecord `json:"snapshot"`
	}
	if err := client.client.postJSON("/v1/snapshots", spec, &response); err != nil {
		return nil, err
	}
	return &response.Snapshot, nil
}

func (client *SnapshotsClient) Get(snapshotID string) (*SnapshotRecord, error) {
	var response struct {
		Snapshot SnapshotRecord `json:"snapshot"`
	}
	if err := client.client.getJSON("/v1/snapshots/"+snapshotID, &response); err != nil {
		return nil, err
	}
	return &response.Snapshot, nil
}

func (client *SnapshotsClient) List() ([]SnapshotRecord, error) {
	var response snapshotListResponse
	if err := client.client.getJSON("/v1/snapshots", &response); err != nil {
		return nil, err
	}
	return response.Snapshots, nil
}

func (client *SnapshotsClient) Replicate(snapshotID string, request map[string]any) (*SnapshotRecord, error) {
	var response struct {
		Snapshot SnapshotRecord `json:"snapshot"`
	}
	if err := client.client.postJSON("/v1/snapshots/"+snapshotID+"/replicate", request, &response); err != nil {
		return nil, err
	}
	return &response.Snapshot, nil
}

func (client *SnapshotsClient) Delete(snapshotID string) (*SnapshotDeleteResult, error) {
	var response SnapshotDeleteResult
	if err := client.client.deleteJSON("/v1/snapshots/"+snapshotID, &response); err != nil {
		return nil, err
	}
	return &response, nil
}
