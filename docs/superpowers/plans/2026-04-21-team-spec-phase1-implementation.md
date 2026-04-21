# TeamSpec Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a first usable `TeamSpec` surface that lets users author `agents`, `tasks`, and `process`, then compiles that into the existing `ExecutionSpec` engine without exposing raw `swarm` or `supervision` fields by default.

**Architecture:** Keep `TeamSpec` as a front-door authoring model in a dedicated `src/team/` module. Parse and validate team documents there, compile them into normal `ExecutionSpec`, and expose bridge/CLI entry points similar to the existing template and batch surfaces. Phase 1 should support `team kind`, `sequential`, `parallel`, and `lead_worker` process types only, with sane defaults and no graph mode yet.

**Tech Stack:** Rust (`serde`, existing bridge/CLI stack), existing `ExecutionSpec` orchestration engine, integration tests in `tests/`, bridge/CLI JSON/YAML handling.

---

## File map

- Create: `src/team/mod.rs`
  - Public `TeamSpec` module entrypoint and re-exports.
- Create: `src/team/schema.rs`
  - `AgentSpec`, `TaskSpec`, `TeamSpec`, `ProcessSpec`, validation helpers.
- Create: `src/team/compile.rs`
  - Compilation from `TeamSpec` to normal `ExecutionSpec`.
- Modify: `src/lib.rs`
  - Export the new `team` module.
- Modify: `src/bridge.rs`
  - Add `POST /v1/teams/dry-run`, `POST /v1/teams/run`, and `GET /v1/team-runs/{id}`.
- Modify: `src/bin/voidctl.rs`
  - Add top-level `team` commands and interactive `/team ...` commands.
- Create: `examples/team/rust_article_team.yaml`
  - Canonical checked-in `TeamSpec` example.
- Create: `tests/team_api.rs`
  - Bridge and compile integration coverage for the new `team` surface.
- Modify: `tests/voidctl_execution_cli.rs`
  - CLI and interactive `/team` coverage.
- Modify: `README.md`
  - Document the new `team` surface and example.
- Modify: `AGENTS.md`
  - Document operator workflows and endpoints for `team`.

## Chunk 1: TeamSpec schema and compiler

### Task 1: Add the failing schema tests

**Files:**
- Create: `tests/team_api.rs`
- Reference: `docs/superpowers/specs/2026-04-21-team-spec-authoring-draft.md`

- [ ] **Step 1: Write the failing validation test**

```rust
#[test]
fn team_dry_run_rejects_missing_agents() {
    let spec = r#"
api_version: v1
kind: team
tasks:
  - name: write
    description: Write the article
process:
  type: sequential
"#;

    let response = submit_team_dry_run(spec);
    assert_eq!(response.status, 400);
    assert!(response.body.contains("team spec must include at least one agent"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --features serde --test team_api team_dry_run_rejects_missing_agents -- --nocapture`

Expected: FAIL because `/v1/teams/dry-run` and schema validation do not exist yet.

- [ ] **Step 3: Write the failing compilation test**

```rust
#[test]
fn team_dry_run_compiles_parallel_process_to_swarm() {
    let spec = r#"
api_version: v1
kind: team
agents:
  - name: researcher
    role: Researcher
    goal: Find information
tasks:
  - name: research
    description: Gather evidence
    agent: researcher
process:
  type: parallel
"#;

    let response = submit_team_dry_run(spec);
    assert_eq!(response.status, 200);
    assert!(response.body.contains("\"compiled_primitive\":\"swarm\""));
}
```

- [ ] **Step 4: Run test to verify it fails**

Run: `cargo test --features serde --test team_api team_dry_run_compiles_parallel_process_to_swarm -- --nocapture`

Expected: FAIL because the compiler path does not exist yet.

### Task 2: Implement `TeamSpec` schema

**Files:**
- Create: `src/team/mod.rs`
- Create: `src/team/schema.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Add public module wiring**

```rust
pub mod team;
```

- [ ] **Step 2: Define the public schema types**

```rust
pub struct AgentSpec {
    pub name: String,
    pub role: String,
    pub goal: String,
    pub template: Option<String>,
}

pub struct TaskSpec {
    pub name: String,
    pub description: String,
    pub agent: Option<String>,
    pub depends_on: Vec<String>,
}

pub struct TeamSpec {
    pub api_version: String,
    pub kind: String,
    pub metadata: Option<TeamMetadata>,
    pub agents: Vec<AgentSpec>,
    pub tasks: Vec<TaskSpec>,
    pub process: ProcessSpec,
}
```

- [ ] **Step 3: Add validation helpers**

Rules:
- `kind` must be `team`
- at least one agent
- at least one task
- `task.agent` must refer to a known agent when present
- `process.type` must be one of `sequential`, `parallel`, `lead_worker`

- [ ] **Step 4: Run the focused tests**

Run: `cargo test --features serde --test team_api team_dry_run_rejects_missing_agents -- --nocapture`

Expected: FAIL later in the route/compiler layer, not in Rust type resolution.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/team/mod.rs src/team/schema.rs tests/team_api.rs
git commit -m "team: add phase1 schema"
```

### Task 3: Implement the compiler

**Files:**
- Create: `src/team/compile.rs`
- Modify: `src/team/mod.rs`
- Reference: `src/orchestration/spec.rs`
- Reference: `src/orchestration/variation.rs`

- [ ] **Step 1: Write the minimal compiler contract**

```rust
pub struct CompiledTeamExecution {
    pub compiled_primitive: &'static str,
    pub execution_spec: ExecutionSpec,
}

pub fn compile_team_spec(spec: &TeamSpec) -> Result<CompiledTeamExecution, String> {
    todo!()
}
```

- [ ] **Step 2: Implement process mapping**

Rules:
- `parallel` -> `swarm`
- `sequential` -> `swarm` with one task-active-at-a-time defaults
- `lead_worker` -> `supervision`

- [ ] **Step 3: Generate the minimal `ExecutionSpec`**

For phase 1:
- use explicit candidate generation
- derive one candidate per task/agent binding
- map agent template/default runtime from the agent or a phase-1 default
- set policy/evaluation defaults internally instead of exposing them to the caller

- [ ] **Step 4: Run the focused compiler test**

Run: `cargo test --features serde --test team_api team_dry_run_compiles_parallel_process_to_swarm -- --nocapture`

Expected: PASS for the new compile path.

- [ ] **Step 5: Commit**

```bash
git add src/team/mod.rs src/team/compile.rs tests/team_api.rs
git commit -m "team: compile spec to execution"
```

## Chunk 2: Bridge API and example spec

### Task 4: Add bridge routes

**Files:**
- Modify: `src/bridge.rs`
- Modify: `tests/team_api.rs`

- [ ] **Step 1: Write the failing bridge route test**

```rust
#[test]
fn team_run_returns_execution_summary() {
    let spec = load_fixture("examples/team/rust_article_team.yaml");
    let response = submit_team_run(&spec);
    assert_eq!(response.status, 200);
    assert!(response.body.contains("\"kind\":\"team\""));
    assert!(response.body.contains("\"execution_id\""));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --features serde --test team_api team_run_returns_execution_summary -- --nocapture`

Expected: FAIL because `/v1/teams/run` does not exist yet.

- [ ] **Step 3: Implement routes**

Add:
- `POST /v1/teams/dry-run`
- `POST /v1/teams/run`
- `GET /v1/team-runs/{id}`

Behavior:
- parse YAML/JSON `TeamSpec`
- validate + compile
- dry-run returns compile summary
- run returns normal execution summary plus `kind=team`
- `GET /v1/team-runs/{id}` returns the underlying execution detail in a `team` wrapper

- [ ] **Step 4: Run the team API tests**

Run: `cargo test --features serde --test team_api -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/bridge.rs tests/team_api.rs
git commit -m "bridge: add team endpoints"
```

### Task 5: Check in the canonical example

**Files:**
- Create: `examples/team/rust_article_team.yaml`
- Modify: `README.md`
- Modify: `AGENTS.md`

- [ ] **Step 1: Add the example file**

Use the smallest realistic example from the draft:
- two agents
- two tasks
- `process.type: sequential`

- [ ] **Step 2: Document the new endpoints and CLI workflow**

Add:
- HTTP examples for `POST /v1/teams/dry-run` and `POST /v1/teams/run`
- CLI examples for `voidctl team dry-run` and `voidctl team run`

- [ ] **Step 3: Run docs sanity checks**

Run: `rg -n "/v1/teams|voidctl team|/team " README.md AGENTS.md examples/team`

Expected: the new surface is documented consistently.

- [ ] **Step 4: Commit**

```bash
git add examples/team/rust_article_team.yaml README.md AGENTS.md
git commit -m "docs: add team example workflow"
```

## Chunk 3: CLI and interactive console

### Task 6: Add top-level `voidctl team ...`

**Files:**
- Modify: `src/bin/voidctl.rs`
- Modify: `tests/voidctl_execution_cli.rs`

- [ ] **Step 1: Write the failing top-level CLI tests**

Add tests for:
- `voidctl team dry-run <spec-path>`
- `voidctl team run <spec-path>`

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --features serde --test voidctl_execution_cli team_ -- --nocapture`

Expected: FAIL because the parser has no `team` surface.

- [ ] **Step 3: Implement top-level parser and output**

Add:
- `CliCommand::Team`
- `voidctl team dry-run`
- `voidctl team run`

Reuse the team bridge endpoints instead of a second code path.

- [ ] **Step 4: Run focused CLI tests**

Run: `cargo test --features serde --test voidctl_execution_cli team_ -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/bin/voidctl.rs tests/voidctl_execution_cli.rs
git commit -m "cli: add team commands"
```

### Task 7: Add interactive `/team ...`

**Files:**
- Modify: `src/bin/voidctl.rs`
- Modify: `tests/voidctl_execution_cli.rs`

- [ ] **Step 1: Write the failing interactive tests**

Add:
- `/team dry-run <spec-path>`
- `/team run <spec-path>`
- interactive bridge failure path

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --features serde --test voidctl_execution_cli interactive_team -- --nocapture`

Expected: FAIL because the interactive parser does not know `/team`.

- [ ] **Step 3: Implement interactive parser/help/completion**

Add:
- `/team` to completion candidates
- `/team dry-run`
- `/team run`
- help text entries

- [ ] **Step 4: Run the CLI suite**

Run: `cargo test --features serde --test voidctl_execution_cli -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/bin/voidctl.rs tests/voidctl_execution_cli.rs
git commit -m "cli: add interactive team commands"
```

## Chunk 4: Final verification

### Task 8: Run the full Rust verification slice

**Files:**
- Modify: none

- [ ] **Step 1: Run formatting**

Run: `cargo fmt --all -- --check`

Expected: PASS.

- [ ] **Step 2: Run CLI/unit/integration coverage**

Run: `cargo test --features serde --bin voidctl -- --nocapture`

Expected: PASS.

- [ ] **Step 3: Run the new team integration tests**

Run: `cargo test --features serde --test team_api -- --nocapture`

Expected: PASS.

- [ ] **Step 4: Run the broader serde suite**

Run: `cargo test --features serde`

Expected: PASS.

- [ ] **Step 5: Final commit or fix-forward commit**

```bash
git status --short
```

Expected: clean working tree before merge/push decisions.

