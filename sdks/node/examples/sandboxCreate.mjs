import { VoidControlClient } from "../src/index.js";

const baseUrl = process.env.VOID_CONTROL_BASE_URL ?? "http://127.0.0.1:43210";

const spec = {
  api_version: "v1",
  kind: "sandbox",
  runtime: {
    image: process.env.VOID_CONTROL_SANDBOX_IMAGE ?? "python:3.12-slim",
    cpus: Number(process.env.VOID_CONTROL_SANDBOX_CPUS ?? "2"),
    memory_mb: Number(process.env.VOID_CONTROL_SANDBOX_MEMORY_MB ?? "2048")
  }
};

const client = new VoidControlClient({ baseUrl });
const sandbox = await client.sandboxes.create(spec);

console.log(
  JSON.stringify(
    {
      sandboxId: sandbox.sandboxId,
      state: sandbox.state,
      image: sandbox.image,
      cpus: sandbox.cpus,
      memoryMb: sandbox.memoryMb
    },
    null,
    2
  )
);
