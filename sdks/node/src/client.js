import { BatchClient, BatchRunsClient } from "./batch.js";
import { ExecutionsClient } from "./executions.js";
import { PoolsClient } from "./pools.js";
import { SandboxesClient } from "./sandboxes.js";
import { SnapshotsClient } from "./snapshots.js";
import { TemplatesClient } from "./templates.js";
import { BridgeError } from "./models.js";

export class VoidControlClient {
  constructor({ baseUrl, fetchImpl = fetch } = {}) {
    this.baseUrl = String(baseUrl ?? "").replace(/\/+$/, "");
    this._fetch = fetchImpl;
    this.templates = new TemplatesClient(this);
    this.executions = new ExecutionsClient(this);
    this.batch = new BatchClient(this, { routeBase: "/v1/batch" });
    this.batchRuns = new BatchRunsClient(this, { routeBase: "/v1/batch" });
    this.yolo = new BatchClient(this, { routeBase: "/v1/yolo" });
    this.yoloRuns = new BatchRunsClient(this, { routeBase: "/v1/yolo" });
    this.sandboxes = new SandboxesClient(this);
    this.snapshots = new SnapshotsClient(this);
    this.pools = new PoolsClient(this);
  }

  async getJson(path) {
    const response = await this._fetch(`${this.baseUrl}${path}`, {
      method: "GET"
    });
    return this.#decodeResponse(response);
  }

  async postJson(path, payload) {
    const response = await this._fetch(`${this.baseUrl}${path}`, {
      method: "POST",
      headers: {
        "content-type": "application/json"
      },
      body: JSON.stringify(payload)
    });
    return this.#decodeResponse(response);
  }

  async deleteJson(path) {
    const response = await this._fetch(`${this.baseUrl}${path}`, {
      method: "DELETE"
    });
    return this.#decodeResponse(response);
  }

  async #decodeResponse(response) {
    const payload = await response.json();
    if (!response.ok) {
      throw new BridgeError(payload.message ?? `bridge returned HTTP ${response.status}`, {
        code: payload.code ?? null,
        retryable: payload.retryable ?? null
      });
    }
    return payload;
  }
}
