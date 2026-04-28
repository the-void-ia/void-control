import { toExecutionDetail } from "./models.js";

const TERMINAL_STATUSES = new Set(["Completed", "Failed", "Canceled"]);

export class ExecutionsClient {
  constructor(client) {
    this._client = client;
  }

  async get(executionId) {
    const payload = await this._client.getJson(`/v1/executions/${executionId}`);
    return toExecutionDetail(payload);
  }

  async wait(executionId, { pollIntervalMs = 1000 } = {}) {
    while (true) {
      const detail = await this.get(executionId);
      if (TERMINAL_STATUSES.has(detail.execution.status)) {
        return detail;
      }
      await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
    }
  }
}
