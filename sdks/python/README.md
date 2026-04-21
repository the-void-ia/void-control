# void-control Python SDK

Async-first Python client for the `void-control` bridge API.

## Quick start

```python
from void_control import VoidControlClient

client = VoidControlClient(base_url="http://127.0.0.1:43210")
```

The first supported surface is:

- `client.templates`
- `client.executions`

Examples under `examples/` are template-execution examples against the
`void-control` bridge.

They are not ComputeSDK compatibility examples yet. A real ComputeSDK adapter
still needs to model the sandbox lifecycle and action contract:

- `compute.sandbox.create`
- `compute.sandbox.runCode`
- `compute.sandbox.runCommand`
- filesystem actions
- `compute.sandbox.destroy`
