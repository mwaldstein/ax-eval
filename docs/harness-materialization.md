# Harness Materialization

**Status: Draft (deferred to 0.5.0)**

## Purpose

Define how ax-eval provisions agent-facing artifacts — guidance docs and skills —
into the fixture workspace in the layout each harness expects.

Today the fixture template is copied verbatim into the workspace. That copy is
harness-agnostic, so a fixture that ships `AGENTS.md` is invisible to
claude-code (which reads `CLAUDE.md`), and there is no way to provision skills
at all. Harnesses also discover skills from different roots.

This design adds a **materialization step** that runs after the verbatim copy
and before the adapter, installing scenario-declared artifacts for the selected
harness (`--tool`).

---

## Goals

- Author guidance and skills once, in the fixture.
- Declare them in the scenario YAML.
- Let ax-eval install them correctly for whichever harness runs.
- Preserve backward compatibility: scenarios that declare nothing behave exactly
  as today.

## Non-Goals

- Transforming skills between harness-specific dialects (e.g. generating
  Claude Code's extra frontmatter fields). The bundle is passed through.
- Provisioning subagents, agents, or MCP config. Only guidance docs and skills.
- Installing into user-global locations (`~/.claude/`, `~/.agents/`). Project
  workspace only.
- Changing `discover` to generate these fields. That is a later integration.

---

## Design Overview

Materialization is a new step in the run pipeline:

```text
fixture copy (verbatim)  →  materialize artifacts (--tool)  →  adapter.run()
```

It is **scenario-field-centric**: the scenario declares which artifacts apply;
the template stays a generic file source. It is **additive**: the verbatim copy
still happens, and declared artifacts are materialized on top. Existing
scenarios are unchanged.

The selected `--tool` determines where each artifact is written, because no
single path is read by all three harnesses.

---

## Scenario Schema

Two optional fields on the scenario root:

```yaml
agent_guidance: guidance/my_tool.md   # path into fixture → harness guidance file
skills:                               # list of skill bundle dirs in the fixture
  - skills/deploy-recipe
  - skills/release-notes
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `agent_guidance` | string | no | Path (relative to the fixture template) to a markdown guidance file. ax-eval copies its content to the harness's guidance filename. |
| `skills` | list[string] | no | Paths (relative to the fixture template) to skill bundle directories. Each must contain a `SKILL.md`. |

Paths resolve relative to the fixture template root, mirroring `template_folder`
resolution. Omitting both fields means no materialization — the run behaves as
today.

---

## Compatibility Model

Additive. The verbatim template copy is unchanged. Declaring `agent_guidance`
or `skills` only adds harness-specific files to the workspace.

- A scenario with neither field is byte-for-byte identical to today.
- A fixture may still contain `AGENTS.md` directly; it is copied verbatim and
  remains visible to opencode and codex. Declaring `agent_guidance` is how you
  additionally reach claude-code (via `CLAUDE.md`).
- Declared artifacts do not overwrite fixture files that already exist at the
  target path unless they target the same harness file; the declaration is the
  explicit intent and wins.

---

## Guidance Mapping

The declared guidance file is copied to the harness's primary guidance
filename:

| Harness | Target | Notes |
|---|---|---|
| `claude-code` | `CLAUDE.md` | Recent claude-code also reads `AGENTS.md`; `CLAUDE.md` is the safe primary. |
| `opencode` | `AGENTS.md` | opencode reads `AGENTS.md` at the project root. `CLAUDE.md` is only a fallback when `AGENTS.md` is absent and is disableable. |
| `codex` | `AGENTS.md` | codex reads `AGENTS.md` at the repo root. |

If the fixture already provides an `AGENTS.md`, declaring `agent_guidance` for
opencode or codex would duplicate it; prefer pointing `agent_guidance` at the
canonical file and letting materialization place it, or rely on the verbatim
copy for opencode/codex and declare the field only when running claude-code.

---

## Skills

Skill bundles use the [Agent Skills open standard](https://agentskills.io).
A bundle is a directory containing, at minimum, a `SKILL.md`:

```text
skills/deploy-recipe/
├── SKILL.md          # required: frontmatter (name + description) + instructions
├── scripts/          # optional: executable code
├── references/       # optional: documentation
└── assets/           # optional: templates, resources
```

`SKILL.md` frontmatter:

```yaml
---
name: deploy-recipe            # required; lowercase, hyphens, must match dir name
description: Render the deploy recipe for the current tool. Use when deploying.
---
```

### Validation

ax-eval validates each declared skill during `validate` and before a run:

- The path exists in the fixture template.
- It contains a `SKILL.md`.
- Frontmatter `name` and `description` are present.
- `name` matches the parent directory name and matches `^[a-z0-9]+(-[a-z0-9]+)*$`.

Invalid skills are hard errors, like invalid rubrics.

### Per-Harness Install

The bundle is copied verbatim into the harness's skills root:

| Harness | Target | Why |
|---|---|---|
| `claude-code` | `.claude/skills/<name>/` | claude-code's project skills root. |
| `opencode` | `.opencode/skills/<name>/` | opencode-native, always on. opencode also reads `.agents/skills/` and `.claude/skills/`, but the latter is Claude-compat fallback and disableable. |
| `codex` | `.agents/skills/<name>/` | codex's project skills root. |

`<name>` is the skill's frontmatter `name` (equal to the directory name).

### Cross-Tool Note

No single skills path is read by all three harnesses. `.claude/skills/` covers
claude-code and opencode (via compat); `.agents/skills/` covers codex and
opencode. Because ax-eval runs one harness per run, materialization targets only
that harness's primary root rather than writing redundant copies.

opencode's `.agents/` support is **skills-only**. opencode does not read
`.agents/AGENTS.md` for guidance (it reads `AGENTS.md` at the project root) nor
`.agents/agents/` for subagents (those live in `.opencode/agents/`).

---

## Pipeline Integration

Materialization runs after `setup_fixture_relative_to` copies the template and
before the adapter is invoked. It needs:

- The selected `--tool` (already threaded through the run request).
- The materialized workspace root.
- The scenario's `agent_guidance` and `skills` fields.

Target selection is keyed on `--tool`, so adding a future harness means adding
its target paths without changing the scenario schema.

---

## Open Questions

- **Guidance aliasing**: write strictly one file per harness, or also write a
  redundant alias (e.g. both `AGENTS.md` and `CLAUDE.md`) for maximum
  cross-harness robustness when a workspace might be inspected by multiple tools.
- **opencode skills root**: prefer `.opencode/skills/` (native) or
  `.agents/skills/` (shares a path with codex). Current proposal: native.
- **Overwrite semantics**: confirm declared artifacts overwrite a same-named
  harness file already present in the verbatim copy, or error on conflict.

---

## In-System Documentation

Per [ADR 0003](adr/0003-keep-cli-self-documenting.md), new user-facing schema or
workflow changes must update help text, templates, docs, and tests together.
Implementing this feature touches the scenario schema, so update these surfaces
in lockstep:

- **`docs/scenarios.md`**: add `agent_guidance` and `skills` to the scenario
  schema reference (field table and a worked example).
- **`src/commands.rs` (`SCENARIO_TEMPLATE`)**: surface both fields in the
  printable scenario template (`ax-eval template scenario`) as commented
  examples, so users discover them without reading the docs.
- **`docs/user-guide.md`**: add a workflow section on declaring guidance and
  skills, and note the new skill-bundle validation under `validate`.
- **`docs/harness-materialization.md`**: flip **Status** from Draft to Stable
  and resolve the open questions above as decisions are made.
- **`CHANGELOG.md`**: add `Added` entries under the `0.4.0` release for both
  fields and the materialization step.
- **`ax-eval-fixtures/`**: add an example scenario exercising guidance and a
  skill bundle, parallel to how `example_judge` demonstrates the judge.
- **CLI help + snapshots**: run `scripts/generate-cli-reference.sh` and commit
  `docs/reference/cli-commands.md` + `ax-eval-fixtures/cli-help-snapshots/` if
  any CLI help text changes (likely none — these are YAML fields, not flags).
- **`AGENTS.md` (Common Tasks)**: add an "Adding a Harness Materialization
  Target" entry parallel to the existing gate/adapter/schema tasks, listing the
  types, validation dispatch, docs, and tests to touch for a new `--tool` target.

As with prior schema changes, update `src/scenario/types.rs`, add
deserialization coverage in `src/scenario/tests/`, and run `cargo test` before
release.

---

## References

- [Agent Skills specification](https://agentskills.io/specification)
- [Claude Code skills](https://docs.claude.com/en/docs/claude-code/skills)
- [opencode skills](https://opencode.ai/docs/skills/)
- [opencode rules (AGENTS.md)](https://opencode.ai/docs/rules/)
- [Codex skills](https://developers.openai.com/codex/skills/)
