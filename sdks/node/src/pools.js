import { toPoolRecord } from "./models.js";

export class PoolsClient {
  constructor(client) {
    this._client = client;
  }

  async create(spec) {
    const payload = await this._client.postJson("/v1/pools", spec);
    return toPoolRecord(payload);
  }

  async get(poolId) {
    const payload = await this._client.getJson(`/v1/pools/${poolId}`);
    return toPoolRecord(payload);
  }

  async scale(poolId, request) {
    const payload = await this._client.postJson(`/v1/pools/${poolId}/scale`, request);
    return toPoolRecord(payload);
  }
}
