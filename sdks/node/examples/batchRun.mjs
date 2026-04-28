import { VoidControlClient } from "../src/index.js";

const baseUrl = process.env.VOID_CONTROL_BASE_URL ?? "http://127.0.0.1:43210";
const route = process.env.VOID_CONTROL_BATCH_ROUTE ?? "batch";

const spec = {
  api_version: "v1",
  kind: route,
  worker: {
    template: "examples/runtime-templates/warm_agent_basic.yaml"
  },
  jobs: [
    {
      prompt:
        process.env.VOID_CONTROL_BATCH_PROMPT_ONE ?? "Fix failing auth tests"
    },
    {
      prompt:
        process.env.VOID_CONTROL_BATCH_PROMPT_TWO ?? "Improve retry logging"
    }
  ]
};

const client = new VoidControlClient({ baseUrl });
const runner = route === "yolo" ? client.yolo : client.batch;
const runs = route === "yolo" ? client.yoloRuns : client.batchRuns;
const started = await runner.run(spec);
const detail = await runs.wait(started.runId, { pollIntervalMs: 2000 });

console.log(
  JSON.stringify(
    {
      route,
      runId: started.runId,
      kind: started.kind,
      status: detail.execution.status,
      executionId: detail.execution.executionId
    },
    null,
    2
  )
);
