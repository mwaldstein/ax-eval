# Tradeoffs

Known weaknesses and accepted tradeoffs in ax-eval. Each entry states the
limitation, its impact, why it is currently accepted, and any direction. Add
items here when a limitation is understood and unlikely to be removed soon —
not for ordinary bugs or TODOs.

---

## Degree Of Fixture Isolation

**Limitation.** The fixture workspace is file-isolated but not an operating-
system sandbox. Each run copies the template into a fresh per-run directory
under `ax-eval-results/`. Agent processes use a deny-by-default environment
projection with adapter launch/authentication variables and scenario
`agent_env`; stdio MCP `target.env` values are reserved for the server child.
The agent still runs as the caller and retains the caller's filesystem and
network authority.

**Impact.**

- **Global skills and guidance leak in.** Because `HOME` is inherited, global
  skill roots (`~/.claude/skills/`, `~/.agents/skills/`) and global guidance
  (`~/.claude/CLAUDE.md`, `~/.codex/`) are visible to the agent. A run on a
  machine with skills installed behaves differently than one without, eroding
  reproducibility.
- **Future materialized artifacts may compete with global ones.** The deferred
  harness-materialization design (see
  [harness-materialization.md](harness-materialization.md)) will write guidance
  and skills into the workspace, but global copies at the same names may take
  precedence depending on each harness's resolution order.
- **Target tool state is uncontained.** The tool under test may read or write
  its own global config and caches outside the fixture, so the fixture does not
  fully determine target-tool state.
- **Codex MCP provisioning touches global config.** MCP target runs for Codex
  must write `~/.codex/config.toml` because Codex discovers MCP servers there,
  not from a workspace-local file. ax-eval snapshots the prior file content and
  restores it after the agent exits.
- **Execution scope is the caller's.** Network access is unrestricted and the
  agent can run arbitrary commands with the caller's privileges.

**Why accepted.** Full sandboxing (containers, VMs, or `env_clear` plus a
deny-by-default filesystem) is heavy and out of scope for the local
development and CI workflow ax-eval targets. The caller's environment is often
part of what is being evaluated, and real agent execution is gated behind
`AX_EVAL_ENABLED=1` consent. The opencode adapter's `XDG_CONFIG_HOME` override
is a targeted mitigation, not a complete boundary.

**Direction.** Harness materialization is deferred to 0.5.0 and will make
workspace guidance artifacts explicit. A future opt-in sandboxed runner
(containerized or privilege-separated) could provide strict filesystem and
network isolation without removing the lighter default.
