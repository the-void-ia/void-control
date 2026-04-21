# void-control Go SDK

Go client for the `void-control` bridge API.

The first supported surface is:

- templates list/get/dry-run/execute
- executions get/wait
- batch run/dry-run/get/wait
- yolo run/dry-run/get/wait
- sandboxes create/get/list/exec/stop/delete
- snapshots create/get/list/replicate/delete
- pools create/get/scale

Examples under `examples/` are bridge examples against `void-control`:

- `template_execute`
- `batch_run`
- `sandbox_create`

`batch` is the canonical remote-background execution API. `yolo` is an alias
for the same high-level surface.

They are not ComputeSDK compatibility examples yet. A real ComputeSDK adapter
still needs to model the sandbox lifecycle and action contract:

- `compute.sandbox.create`
- `compute.sandbox.runCode`
- `compute.sandbox.runCommand`
- filesystem actions
- `compute.sandbox.destroy`
