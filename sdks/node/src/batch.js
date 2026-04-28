const TERMINAL_STATUSES = new Set(["Completed", "Failed", "Canceled"]);

export class BatchClient {
  constructor(client, { routeBase }) {
    this._client = client;
    this._routeBase = routeBase;
  }

  async run(spec) {
    const payload = await this._client.postJson(`${this._routeBase}/run`, spec);
    return {
      kind: String(payload.kind),
      runId: String(payload.run_id),
      executionId: String(payload.execution_id),
      compiledPrimitive: String(payload.compiled_primitive),
      status: String(payload.status),
      goal: String(payload.goal)
    };
  }
}

export class BatchRunsClient {
  constructor(client, { routeBase }) {
    this._client = client;
    this._routeBase = routeBase;
  }

  async get(runId) {
    const payload = await this._client.getJson(`${this._routeBase}-runs/${runId}`);
    return {
      kind: String(payload.kind),
      runId: String(payload.run_id),
      execution: {
        executionId: String(payload.execution.execution_id),
        goal: String(payload.execution.goal),
        status: String(payload.execution.status)
      },
      progress: payload.progress ?? {},
      result: {
        bestCandidateId:
          payload.result?.best_candidate_id == null
            ? null
            : String(payload.result.best_candidate_id),
        completedIterations: Number(payload.result?.completed_iterations ?? 0),
        totalCandidateFailures: Number(payload.result?.total_candidate_failures ?? 0)
      },
      candidates: payload.candidates ?? []
    };
  }

  async wait(runId, { pollIntervalMs = 1000 } = {}) {
    while (true) {
      const detail = await this.get(runId);
      if (TERMINAL_STATUSES.has(detail.execution.status)) {
        return detail;
      }
      await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
    }
  }
}
