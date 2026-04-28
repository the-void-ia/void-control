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
- `client.batch`
- `client.batch_runs`
- `client.yolo`
- `client.yolo_runs`
- `client.sandboxes`
- `client.snapshots`
- `client.pools`

Examples under `examples/` are bridge examples against `void-control`:

- `template_execute.py`
- `batch_run.py`
- `sandbox_create.py`

`batch` is the canonical remote-background execution API. `yolo` is an alias
for the same high-level surface.

They are not ComputeSDK compatibility examples yet. A real ComputeSDK adapter
still needs to model the sandbox lifecycle and action contract:

- `compute.sandbox.create`
- `compute.sandbox.runCode`
- `compute.sandbox.runCommand`
- filesystem actions
- `compute.sandbox.destroy`
