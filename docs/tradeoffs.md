# Tradeoffs

Known weaknesses and accepted tradeoffs in ax-eval. Each entry states the
limitation, its impact, why it is currently accepted, and any direction. Add
items here when a limitation is understood and unlikely to be removed soon —
not for ordinary bugs or TODOs.

---

## Degree Of Fixture Isolation

**Limitation.** The fixture workspace is file-isolated but not
environment-isolated. Each run copies the template into a fresh per-run
directory under `ax-eval-results/`, but the agent process inherits the caller's
full environment. The session runner applies only specific overrides
(`XDG_CONFIG_HOME` for opencode, plus `target.env` vars) and never clears the
inherited environment.

**Impact.**

- **Global skills and guidance leak in.** Because `HOME` is inherited, global
  skill roots (`~/.claude/skills/`, `~/.agents/skills/`) and global guidance
  (`~/.claude/CLAUDE.md`, `~/.codex/`) are visible to the agent. A run on a
  machine with skills installed behaves differently than one without, eroding
  reproducibility.
- **Materialized artifacts compete with leaked ones.** The harness-materialization
  step (see [harness-materialization.md](harness-materialization.md)) writes
  guidance and skills into the workspace, but global copies at the same names
  may take precedence depending on each harness's resolution order.
- **Target tool state is uncontained.** The tool under test may read or write
  its own global config and caches outside the fixture, so the fixture does not
  fully determine target-tool state.
- **Execution scope is the caller's.** Network access is unrestricted and the
  agent can run arbitrary commands with the caller's privileges.

**Why accepted.** Full sandboxing (containers, VMs, or `env_clear` plus a
deny-by-default filesystem) is heavy and out of scope for the local
development and CI workflow ax-eval targets. The caller's environment is often
part of what is being evaluated, and real agent execution is gated behind
`AX_EVAL_ENABLED=1` consent. The opencode adapter's `XDG_CONFIG_HOME` override
is a targeted mitigation, not a complete boundary.

**Direction.** Harness materialization (0.4.0) makes workspace artifacts
explicit so they do not depend on ambient global state. A future opt-in
sandboxed runner (containerized or env-cleared) could provide a strict
isolation mode for reproducible comparisons without removing the lighter
default.
