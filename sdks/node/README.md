# void-control Node SDK

Async-first Node client for the `void-control` bridge API.

## Quick start

```js
import { VoidControlClient } from "./src/index.js";

const client = new VoidControlClient({ baseUrl: "http://127.0.0.1:43210" });
```

The first supported surface is:

- `client.templates`
- `client.executions`
- `client.batch`
- `client.batchRuns`
- `client.yolo`
- `client.yoloRuns`
- `client.sandboxes`
- `client.snapshots`
- `client.pools`

Examples under `examples/` are bridge examples against `void-control`:

- `templateExecute.mjs`
- `batchRun.mjs`
- `sandboxCreate.mjs`

`batch` is the canonical remote-background execution API. `yolo` is an alias
for the same high-level surface.

They are not ComputeSDK compatibility examples yet. A real ComputeSDK adapter
still needs to model the sandbox lifecycle and action contract:

- `compute.sandbox.create`
- `compute.sandbox.runCode`
- `compute.sandbox.runCommand`
- filesystem actions
- `compute.sandbox.destroy`
