import { toSandboxExecResult, toSandboxRecord } from "./models.js";

export class SandboxesClient {
  constructor(client) {
    this._client = client;
  }

  async create(spec) {
    const payload = await this._client.postJson("/v1/sandboxes", spec);
    return toSandboxRecord(payload);
  }

  async get(sandboxId) {
    const payload = await this._client.getJson(`/v1/sandboxes/${sandboxId}`);
    return toSandboxRecord(payload);
  }

  async list() {
    const payload = await this._client.getJson("/v1/sandboxes");
    return (payload.sandboxes ?? []).map((item) => toSandboxRecord({ sandbox: item }));
  }

  async exec(sandboxId, request) {
    const payload = await this._client.postJson(`/v1/sandboxes/${sandboxId}/exec`, request);
    return toSandboxExecResult(payload);
  }

  async stop(sandboxId) {
    const payload = await this._client.postJson(`/v1/sandboxes/${sandboxId}/stop`, {});
    return toSandboxRecord(payload);
  }

  async delete(sandboxId) {
    return this._client.deleteJson(`/v1/sandboxes/${sandboxId}`);
  }
}
