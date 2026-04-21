import { VoidControlClient } from "../src/index.js";

const baseUrl = process.env.VOID_CONTROL_BASE_URL ?? "http://127.0.0.1:43210";
const templateId = process.env.VOID_CONTROL_TEMPLATE_ID ?? "benchmark-runner-python";

const inputs = {
  goal:
    process.env.VOID_CONTROL_TEMPLATE_GOAL ??
    "Compare transform benchmark candidates",
  provider: process.env.VOID_CONTROL_TEMPLATE_PROVIDER ?? "claude"
};

if (process.env.VOID_CONTROL_TEMPLATE_SNAPSHOT) {
  inputs.snapshot = process.env.VOID_CONTROL_TEMPLATE_SNAPSHOT;
}

const client = new VoidControlClient({ baseUrl });
const execution = await client.templates.execute(templateId, { inputs });
const detail = await client.executions.wait(execution.executionId, {
  pollIntervalMs: 2000
});

console.log(
  JSON.stringify(
    {
      templateId,
      executionId: execution.executionId,
      status: detail.execution.status,
      bestCandidateId: detail.result.bestCandidateId,
      completedIterations: detail.result.completedIterations,
      totalCandidateFailures: detail.result.totalCandidateFailures
    },
    null,
    2
  )
);
