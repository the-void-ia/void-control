import { toSnapshotDeleteResult, toSnapshotRecord } from "./models.js";

export class SnapshotsClient {
  constructor(client) {
    this._client = client;
  }

  async create(spec) {
    const payload = await this._client.postJson("/v1/snapshots", spec);
    return toSnapshotRecord(payload);
  }

  async get(snapshotId) {
    const payload = await this._client.getJson(`/v1/snapshots/${snapshotId}`);
    return toSnapshotRecord(payload);
  }

  async list() {
    const payload = await this._client.getJson("/v1/snapshots");
    return (payload.snapshots ?? []).map((item) => toSnapshotRecord({ snapshot: item }));
  }

  async replicate(snapshotId, request) {
    const payload = await this._client.postJson(
      `/v1/snapshots/${snapshotId}/replicate`,
      request
    );
    return toSnapshotRecord(payload);
  }

  async delete(snapshotId) {
    const payload = await this._client.deleteJson(`/v1/snapshots/${snapshotId}`);
    return toSnapshotDeleteResult(payload);
  }
}
