# Template-First Agent API Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a file-backed, template-first API to `void-control` that compiles template inputs into normal `ExecutionSpec` objects and reuses the existing execution creation and dry-run flow.

**Architecture:** Introduce a small `src/templates/` module that loads checked-in template files, validates user inputs, compiles them into the current `ExecutionSpec` plus runtime template path, and exposes new bridge routes for list/get/dry-run/execute. Phase 1 remains execution-centric: templates create normal `Execution`s and reuse the existing bridge and orchestration stack.

**Tech Stack:** Rust, serde/serde_yaml/serde_json, existing `bridge.rs`, existing `ExecutionSpec` validation, checked-in YAML templates.

---

## File Map

**Create:**

- `src/templates/mod.rs`
- `src/templates/schema.rs`
- `src/templates/compile.rs`
- `templates/single-agent-basic.yaml`
- `templates/warm-agent-basic.yaml`
- `tests/template_api.rs`

**Modify:**

- `src/lib.rs`
- `src/bridge.rs`
- `README.md`
- `AGENTS.md`

**Reference:**

- `src/orchestration/spec.rs`
- `examples/runtime-templates/claude_mcp_diagnostic_agent.yaml`
- `examples/runtime-templates/warm_agent_basic.yaml`
- `docs/superpowers/specs/2026-04-20-template-first-agent-api-design.md`

## Chunk 1: Template Core

### Task 1: Add the template module skeleton

**Files:**
- Create: `src/templates/mod.rs`
- Modify: `src/lib.rs`
- Test: `tests/template_api.rs`

- [ ] **Step 1: Write the failing compile-time test coverage entry**

Add a test file import path in `tests/template_api.rs` that will eventually use the public template module.

- [ ] **Step 2: Run the targeted test command to verify the module is missing**

Run: `cargo test --features serde template_api -- --nocapture`
Expected: FAIL with unresolved module or missing imports.

- [ ] **Step 3: Create `src/templates/mod.rs` with public exports**

Add:
- template data types
- loader entry points
- compiler entry points

- [ ] **Step 4: Export the module from `src/lib.rs`**

Add `pub mod templates;`.

- [ ] **Step 5: Run the targeted test command again**

Run: `cargo test --features serde template_api -- --nocapture`
Expected: FAIL later, now due to missing schema/loader behavior instead of missing module wiring.

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs src/templates/mod.rs tests/template_api.rs
git commit -m "templates: add module skeleton"
```

### Task 2: Define the phase-1 template schema

**Files:**
- Create: `src/templates/schema.rs`
- Modify: `src/templates/mod.rs`
- Test: `tests/template_api.rs`

- [ ] **Step 1: Write failing tests for template parsing**

Add tests that load inline YAML and assert support for:
- `template.id`
- `template.execution_kind`
- `inputs`
- `defaults.workflow_template`
- `defaults.execution_spec`
- `compile.bindings`

- [ ] **Step 2: Run the parsing tests and verify they fail**

Run: `cargo test --features serde template_schema -- --nocapture`
Expected: FAIL because template parsing/types do not exist yet.

- [ ] **Step 3: Implement phase-1 schema types in `src/templates/schema.rs`**

Define:
- `ControlTemplate`
- `TemplateMetadata`
- `InputField`
- `InputFieldType`
- `TemplateDefaults`
- `CompileBinding`
- `TemplateCompile`

Use serde derives and explicit enums for phase-1 constraints.

- [ ] **Step 4: Add schema validation helpers**

Validate:
- only `single_agent` and `warm_agent`
- only supported field types
- `enum` requires `values`
- required top-level sections exist

- [ ] **Step 5: Re-run the parsing tests**

Run: `cargo test --features serde template_schema -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/templates/mod.rs src/templates/schema.rs tests/template_api.rs
git commit -m "templates: add phase-1 schema"
```

### Task 3: Add file-backed template loading

**Files:**
- Modify: `src/templates/mod.rs`
- Modify: `src/templates/schema.rs`
- Test: `tests/template_api.rs`

- [ ] **Step 1: Write failing tests for checked-in template loading**

Add tests that:
- list template files from `templates/`
- load `single-agent-basic.yaml`
- load `warm-agent-basic.yaml`

- [ ] **Step 2: Run the loader tests and verify they fail**

Run: `cargo test --features serde template_loader -- --nocapture`
Expected: FAIL because loader functions and files do not exist yet.

- [ ] **Step 3: Implement template loader functions**

Add:
- `list_templates()`
- `load_template(id)`
- path resolution under repo-root `templates/`

Keep the implementation strict and deterministic.

- [ ] **Step 4: Re-run the loader tests**

Run: `cargo test --features serde template_loader -- --nocapture`
Expected: still FAIL because the checked-in templates do not exist yet.

- [ ] **Step 5: Commit**

```bash
git add src/templates/mod.rs src/templates/schema.rs tests/template_api.rs
git commit -m "templates: add file-backed loader"
```

## Chunk 2: Checked-In Templates

### Task 4: Add `single-agent-basic` template file

**Files:**
- Create: `templates/single-agent-basic.yaml`
- Test: `tests/template_api.rs`

- [ ] **Step 1: Write/expand the failing loader test for `single-agent-basic`**

Assert:
- file exists
- schema loads
- `execution_kind == "single_agent"`

- [ ] **Step 2: Run the loader test and verify it fails**

Run: `cargo test --features serde template_loader -- --nocapture`
Expected: FAIL with missing file.

- [ ] **Step 3: Add `templates/single-agent-basic.yaml`**

Use the design doc example and point to:
- `examples/runtime-templates/claude_mcp_diagnostic_agent.yaml`

- [ ] **Step 4: Re-run the loader test**

Run: `cargo test --features serde template_loader -- --nocapture`
Expected: still FAIL because `warm-agent-basic` is still missing.

- [ ] **Step 5: Commit**

```bash
git add templates/single-agent-basic.yaml tests/template_api.rs
git commit -m "templates: add single-agent starter"
```

### Task 5: Add `warm-agent-basic` template file

**Files:**
- Create: `templates/warm-agent-basic.yaml`
- Test: `tests/template_api.rs`

- [ ] **Step 1: Write/expand the failing loader test for `warm-agent-basic`**

Assert:
- file exists
- schema loads
- `execution_kind == "warm_agent"`
- `workflow_template == examples/runtime-templates/warm_agent_basic.yaml`

- [ ] **Step 2: Run the loader test and verify it fails**

Run: `cargo test --features serde template_loader -- --nocapture`
Expected: FAIL with missing file.

- [ ] **Step 3: Add `templates/warm-agent-basic.yaml`**

Use the design doc example and point to:
- `examples/runtime-templates/warm_agent_basic.yaml`

- [ ] **Step 4: Re-run the loader test**

Run: `cargo test --features serde template_loader -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add templates/warm-agent-basic.yaml tests/template_api.rs
git commit -m "templates: add warm-agent starter"
```

## Chunk 3: Compilation

### Task 6: Implement input validation and binding application

**Files:**
- Create: `src/templates/compile.rs`
- Modify: `src/templates/mod.rs`
- Test: `tests/template_api.rs`

- [ ] **Step 1: Write failing tests for compile success**

Add tests that compile:
- `single-agent-basic` with `goal`, `prompt`, `provider`
- `warm-agent-basic` with `goal`, `prompt`, `provider`

Assert the resulting `ExecutionSpec` contains:
- correct `goal`
- `workflow.template` from template defaults
- `variation.explicit[0].overrides["agent.prompt"]`
- `variation.explicit[0].overrides["llm.provider"]`

- [ ] **Step 2: Run the compile tests and verify they fail**

Run: `cargo test --features serde template_compile -- --nocapture`
Expected: FAIL because compile logic does not exist yet.

- [ ] **Step 3: Implement phase-1 compilation**

Implement:
- user input validation
- baseline `ExecutionSpec` materialization
- binding target application
- support for:
  - `execution_spec.*`
  - `variation.explicit[0].overrides.<key>`

- [ ] **Step 4: Re-run the compile tests**

Run: `cargo test --features serde template_compile -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/templates/mod.rs src/templates/compile.rs tests/template_api.rs
git commit -m "templates: compile inputs into execution specs"
```

### Task 7: Validate compile failures clearly

**Files:**
- Modify: `src/templates/compile.rs`
- Test: `tests/template_api.rs`

- [ ] **Step 1: Write failing tests for compile errors**

Add tests for:
- missing required input
- unsupported input field
- unsupported binding target
- missing explicit proposal for override target

- [ ] **Step 2: Run the failing-case tests**

Run: `cargo test --features serde template_compile_errors -- --nocapture`
Expected: FAIL until error handling is implemented.

- [ ] **Step 3: Add explicit error types/messages**

Return actionable messages that can surface directly through the bridge.

- [ ] **Step 4: Re-run the failing-case tests**

Run: `cargo test --features serde template_compile_errors -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/templates/compile.rs tests/template_api.rs
git commit -m "templates: validate compile failures"
```

## Chunk 4: Bridge API

### Task 8: Add template list/get routes

**Files:**
- Modify: `src/bridge.rs`
- Test: `tests/template_api.rs`

- [ ] **Step 1: Write failing bridge tests for list/get**

Add tests covering:
- `GET /v1/templates`
- `GET /v1/templates/single-agent-basic`
- 404 for unknown template id

- [ ] **Step 2: Run the bridge tests and verify they fail**

Run: `cargo test --features serde template_bridge_list -- --nocapture`
Expected: FAIL because routes do not exist.

- [ ] **Step 3: Implement list/get bridge handlers**

Reuse the test bridge path pattern already present in `src/bridge.rs`.

- [ ] **Step 4: Re-run the bridge tests**

Run: `cargo test --features serde template_bridge_list -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/bridge.rs tests/template_api.rs
git commit -m "bridge: add template list and get routes"
```

### Task 9: Add template dry-run route

**Files:**
- Modify: `src/bridge.rs`
- Test: `tests/template_api.rs`

- [ ] **Step 1: Write failing bridge tests for `POST /v1/templates/{id}/dry-run`**

Assert response contains:
- template metadata
- normalized inputs
- compiled execution summary
- no execution created

- [ ] **Step 2: Run the dry-run bridge tests and verify they fail**

Run: `cargo test --features serde template_bridge_dry_run -- --nocapture`
Expected: FAIL because route does not exist.

- [ ] **Step 3: Implement the dry-run route**

Compile the template and return preview JSON without persisting an execution.

- [ ] **Step 4: Re-run the dry-run bridge tests**

Run: `cargo test --features serde template_bridge_dry_run -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/bridge.rs tests/template_api.rs
git commit -m "bridge: add template dry-run route"
```

### Task 10: Add template execute route

**Files:**
- Modify: `src/bridge.rs`
- Test: `tests/template_api.rs`

- [ ] **Step 1: Write failing bridge tests for `POST /v1/templates/{id}/execute`**

Assert:
- a normal `Execution` is created
- response includes `execution_id`
- template metadata is echoed
- unknown template id returns 404

- [ ] **Step 2: Run the execute bridge tests and verify they fail**

Run: `cargo test --features serde template_bridge_execute -- --nocapture`
Expected: FAIL because route does not exist.

- [ ] **Step 3: Implement template execute by compiling to normal `ExecutionSpec`**

Reuse the existing execution creation path instead of adding a parallel
persistence model.

- [ ] **Step 4: Re-run the execute bridge tests**

Run: `cargo test --features serde template_bridge_execute -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/bridge.rs tests/template_api.rs
git commit -m "bridge: add template execute route"
```

## Chunk 5: Docs and Verification

### Task 11: Document the template API

**Files:**
- Modify: `README.md`
- Modify: `AGENTS.md`

- [ ] **Step 1: Add README section for template-first API**

Document:
- template files location
- list/get/dry-run/execute routes
- the two starter templates

- [ ] **Step 2: Update `AGENTS.md`**

Add:
- templates directory
- expectation that template files are checked-in and reviewed like specs

- [ ] **Step 3: Commit**

```bash
git add README.md AGENTS.md
git commit -m "docs: describe template-first api"
```

### Task 12: Run full verification

**Files:**
- Test: `tests/template_api.rs`

- [ ] **Step 1: Run targeted template API tests**

Run: `cargo test --features serde template_ -- --nocapture`
Expected: PASS

- [ ] **Step 2: Run Rust unit/integration verification**

Run: `cargo test`
Expected: PASS

- [ ] **Step 3: Run serde verification**

Run: `cargo test --features serde`
Expected: PASS

- [ ] **Step 4: Run lint verification**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS

- [ ] **Step 5: Optional UI verification**

Run: `cd web/void-control-ux && npm run build`
Expected: PASS

- [ ] **Step 6: Commit final verification-safe state**

```bash
git add .
git commit -m "templates: ship phase-1 template api"
```
