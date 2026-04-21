# void-control Node SDK

Async-first Node client for the `void-control` bridge API.

## Quick start

```js
import { VoidControlClient } from "./src/index.js";

const client = new VoidControlClient({ baseUrl: "http://127.0.0.1:43210" });
```

Examples under `examples/` are template-execution examples against the
`void-control` bridge.

They are not ComputeSDK compatibility examples yet. A real ComputeSDK adapter
still needs to model the sandbox lifecycle and action contract:

- `compute.sandbox.create`
- `compute.sandbox.runCode`
- `compute.sandbox.runCommand`
- filesystem actions
- `compute.sandbox.destroy`
