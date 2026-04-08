# Install The `void-control` Skill For Codex

Install this skill into Codex’s local skills directory.

## Goal

After installation, Codex should be able to use the `void-control` skill to:

- generate a spec from a problem statement
- submit orchestration or runtime specs with `voidctl execution submit`
- watch and inspect executions
- summarize results
- resolve runtime runs behind execution candidates

## Install Steps

1. Create the local skill directory:

```bash
mkdir -p ~/.codex/skills/void-control
```

2. Copy the canonical skill file from this repo into that directory:

```bash
cp skills/void-control/SKILL.md ~/.codex/skills/void-control/SKILL.md
```

3. Start a new Codex session.

## If You Are Installing From GitHub

If Codex was asked to fetch this file from GitHub instead of a local checkout, it should also fetch:

- `https://raw.githubusercontent.com/the-void-ia/void-control/refs/heads/main/skills/void-control/SKILL.md`

and save it to:

- `~/.codex/skills/void-control/SKILL.md`

## Verify Installation

Start a fresh session and ask Codex to use the `void-control` skill for a terminal workflow, for example:

- “Use the void-control skill to dry-run the 3-agent swarm example.”
- “Use the void-control skill to create a spec from this problem statement and submit it through voidctl.”

The skill should guide Codex toward:

- `voidctl execution dry-run ...`
- `voidctl execution submit ...`
- `voidctl execution watch ...`
- `voidctl execution result ...`
