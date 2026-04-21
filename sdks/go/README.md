# void-control Go SDK

Go client for the `void-control` bridge API.

The first supported surface is:

- templates list/get/dry-run/execute
- executions get/wait

Examples under `examples/` are template-execution examples against the
`void-control` bridge.

They are not ComputeSDK compatibility examples yet. A real ComputeSDK adapter
still needs to model the sandbox lifecycle and action contract:

- `compute.sandbox.create`
- `compute.sandbox.runCode`
- `compute.sandbox.runCommand`
- filesystem actions
- `compute.sandbox.destroy`
