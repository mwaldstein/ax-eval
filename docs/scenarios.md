# Scenarios

**Status: Stable**

## Purpose

Define the scenario format, target tool configuration, and fixture structure for ax-eval.

A scenario is the unit of evaluation. Each scenario defines a task for an LLM agent to attempt using a specific CLI tool, then evaluates the result. The framework is tool-agnostic: the scenario declares what tool is being evaluated and how to interact with it.

---

## Target Tool Configuration

Each scenario must declare the CLI tool under evaluation. This is the **target tool** — the thing the LLM agent will be asked to use.

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `target.binary` | string | yes | Binary name used to identify target tool invocations in interaction evidence. Not executed by the harness; the agent discovers and runs the tool based on AGENTS.md and the fixture workspace. The path portion is ignored — only the filename is matched (e.g. `./todo`, `todo`, and `/usr/local/bin/todo` all match `todo`). |
| `target.command_pattern` | string | no | Regex for identifying target tool invocations only when transcript regex fallback evidence is used. No default; when omitted, only aggregate command counts are available for regex-based metrics. |
| `target.env` | map<string, string> | no | Environment variables to set for the target tool (e.g., config paths, auth tokens, fixture paths) |
| `target.health_check` | string | no | Command to verify the tool is working before/after runs |

### Configuration Source

Target tool config is defined inline in scenario YAML so each scenario is self-contained:

```yaml
target:
  binary: mytool
  command_pattern: "mytool\\s+"
  health_check: "mytool --version"
  env:
    MYTOOL_ROOT_DIR: "${AX_EVAL_FIXTURE_DIR}"
    MYTOOL_EXPORT: "${AX_EVAL_RESULTS_DIR}/mytool-export.json"
    MYTOOL_CONFIG: "/etc/mytool/config.toml"
    MYTOOL_AUTH_TOKEN: "token-value"
```

`target.binary` identifies which commands in the agent's interaction belong to
your tool (for interaction profiling and judge prompts). The harness does not
execute it — the agent runs the tool itself based on the fixture's AGENTS.md.
Only the filename is used for matching, so `./mytool`, `mytool`, and
`/usr/local/bin/mytool` are equivalent.

If no target is specified, scenario loading fails with a clear error.
Most environment values are passed literally. Two run-directory placeholders are
expanded after the isolated workspace is created:

- `${AX_EVAL_FIXTURE_DIR}`: absolute path to the per-run fixture directory.
- `${AX_EVAL_RESULTS_DIR}`: absolute path to the per-run results directory.

Use these when the target tool needs a root/config/output path inside the test
workspace:

```yaml
target:
  binary: mytool
  env:
    MYTOOL_ROOT_DIR: "${AX_EVAL_FIXTURE_DIR}"
```

`ax-eval` does not modify `PATH` to locate the target tool. The agent
discovers and runs the tool based on AGENTS.md and the fixture workspace.
During local development, where the target tool often lives in a build output
directory such as `target/debug`, `target/release`, or `dist`, make sure it
is visible to the agent — either by putting the binary in the fixture directory
(and documenting it in AGENTS.md) or by setting PATH before invoking the
harness:

```bash
PATH="$PWD/target/debug:$PATH" \
  AX_EVAL_ENABLED=1 \
  ax-eval run --scenario my_scenario --tool opencode
```

### Environment value expansion

`target.env` values are literal except for two framework placeholders:

- `${AX_EVAL_FIXTURE_DIR}` — absolute path to the per-run fixture directory.
- `${AX_EVAL_RESULTS_DIR}` — absolute path to the per-run results directory.

The child process inherits the parent environment. `target.env` entries are
layered on top, overriding any inherited values by name. This means setting
`PATH` in `target.env` **replaces** the inherited PATH entirely — it does not
prepend or append. If you need to modify PATH, do it at the shell level before
invoking `ax-eval` (as shown above).

### `command_pattern`

The command pattern is used only by transcript regex fallback analysis. Adapters
that support structured tool calls must provide structured interaction evidence
instead; `command_pattern` is ignored for those adapter-backed metrics.

For fallback adapters, the pattern identifies tool invocations, counts them, and
extracts arguments. The capture group is optional —
when present, it enables per-subcommand analytics:

```yaml
# Simple: just match the binary
command_pattern: "mytool"

# Complex: match the binary plus known subcommands
command_pattern: "mytool\\s+(add|list|remove|search|show)"
```

### `health_check`

Replaces the domain-specific `doctor_passes` gate. The health check command runs before the scenario (to verify prerequisites) and optionally after (as an evaluation gate via `command_succeeds`). Any command that returns exit code 0 on success works.

```yaml
health_check: "mytool --version"
health_check: "mytool doctor"
health_check: "mytool status --json | jq -e '.healthy'"
```

---

## Interaction Configuration

Scenarios can declare whether a completed run is expected to include target-tool
commands:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `interaction.target_commands` | `required`, `optional`, or `forbidden` | `required` | Controls how zero or nonzero target-tool command evidence is interpreted. |

```yaml
interaction:
  target_commands: required
```

Use `required` for normal CLI evaluation scenarios. A completed structured run
that returns zero usable target-tool events is treated as an evaluation error,
which catches adapter normalization failures and agents that skipped the target
tool entirely.

Use `optional` for validation scenarios where the target tool may legitimately
not be invoked, such as prompts that only ask the agent to respond with a marker
string.

Use `forbidden` when target-tool invocation would be a scenario failure, such as
a validation scenario that explicitly requires no target interaction.

---

## Scenario Format

Scenarios are YAML files. Each file defines one scenario.

### Schema

```yaml
name: string                     # Human-readable name (required)
description: string              # What this scenario tests (required)

target:                          # Target tool configuration (required)
  binary: string
  command_pattern: string        # optional
  health_check: string           # optional
  env:                           # optional
    KEY: value

template_folder: string          # Path to fixture directory (required)

task:
  prompt: string                 # Prompt given to the LLM agent (required)

interaction:                     # optional
  target_commands: required      # required, optional, or forbidden

setup:                           # optional
  commands:                      # Shell commands to run before the task
    - string

scripts:                         # optional, see docs/scripts.md
  post:                          # Run after agent exits, before evaluation
    - command: string
      timeout_secs: int          # optional (default: 30)
  evaluators:                    # Produce custom metrics/scores during evaluation
    - command: string
      name: string               # Key for storing results in metrics.json
      timeout_secs: int          # optional (default: 60)

evaluation:
  gates:                         # List of gate assertions
    - type: gate_type            # See docs/evaluation.md for gate types
      ...gate_params
  judge:                         # optional LLM-as-judge configuration
    enabled: bool
    tool: string                  # optional judge CLI tool: opencode, codex, claude, claude-code (default: opencode)
    rubric: string               # optional path to rubric YAML; defaults apply when omitted
    criteria:                    # optional inline criteria; used when rubric is omitted
      - id: string
        weight: float            # weights must sum to 1.0
        description: string
    pass_threshold: float        # 0.0-1.0

tool_matrix:                     # optional
  - tool: string                 # LLM agent tool name (e.g., "opencode", "claude-code")
    models:
      - string

run:
  timeout_secs: int              # Execution timeout

tags:                            # optional categorization tags
  - string

tier: int                        # Priority tier, 0 = highest (default: 0)
```

### Complete Example

This scenario tests a hypothetical task manager CLI (`taskmgr`) to verify an LLM can create tasks, set priorities, and query by status.

```yaml
name: task_manager_organize
description: >
  Verify the LLM can use taskmgr to create a set of tasks from a project
  brief, assign priorities, and filter by status.

target:
  binary: taskmgr
  command_pattern: "taskmgr\\s+(add|list|done|priority|show|search)"
  health_check: "taskmgr --version"

template_folder: fixtures/task_manager_organize

task:
  prompt: |
    You have access to `taskmgr`, a CLI task manager. Read the project brief
    in README.md, then:

    1. Create tasks for each deliverable mentioned in the brief.
    2. Set priority "high" on any task related to the database migration.
    3. Mark the "Write project charter" task as done.
    4. List all remaining high-priority tasks.

    Use `taskmgr --help` if you need to see available commands.

setup:
  commands:
    - "taskmgr init"

scripts:
  post:
    - command: "taskmgr list --format json --all > .task-export.json"
  evaluators:
    - command: "./scripts/task-quality.sh"
      name: "task_quality"

evaluation:
  gates:
    - type: command_succeeds
      command: "taskmgr list --format json | jq -e 'length >= 4'"
    - type: script
      command: "./scripts/check-priorities.sh"
      description: "Database tasks have high priority"
    - type: script
      command: "./scripts/check-completion.sh"
      description: "Charter task is marked done"
  judge:
    enabled: true
    tool: opencode
    criteria:
      - id: task_completion
        weight: 0.50
        description: "The agent achieved the user's requested goal and produced the intended outcome"
      - id: tool_usage_correctness
        weight: 0.30
        description: "The agent used the CLI tool correctly"
      - id: efficiency
        weight: 0.20
        description: "The agent completed the task without unnecessary commands or avoidable confusion"
    pass_threshold: 0.70

tool_matrix:
  - tool: opencode
    models:
      - gpt-4o
      - claude-sonnet
  - tool: claude-code
    models:
      - claude-sonnet

run:
  timeout_secs: 300

tags:
  - task-management
  - crud
  - filtering

tier: 1
```

---

## Fixture Structure

Each scenario references a `template_folder` containing the initial workspace state. This directory is copied into an isolated working directory before the scenario runs.

### Layout

```
fixtures/task_manager_organize/
├── AGENTS.md              # Guidance for the LLM — tool docs, patterns, examples
├── README.md              # Project context the LLM should read
├── scripts/               # Custom scripts for gates and evaluators (optional)
│   ├── check-priorities.sh
│   ├── check-completion.sh
│   └── task-quality.sh
└── project-brief.md       # Any other files the LLM should see
```

### AGENTS.md

**This is the most important file in the fixture.** It is the primary documentation the LLM receives about the target tool. For guidance/skills authors, this IS the thing being tested.

The AGENTS.md should contain:
- Available commands with usage examples
- Common workflows and patterns
- Output format documentation
- Error handling guidance
- Any constraints or conventions

Example for the task manager scenario:

```markdown
# Task Manager (taskmgr)

## Commands

taskmgr init                          # Initialize task store in current directory
taskmgr add "Title" [--priority P]    # Create a task (priority: low|medium|high)
taskmgr list [--status S] [--priority P] [--format F]
                                      # List tasks (status: todo|done, format: human|json)
taskmgr done <id>                     # Mark task as done
taskmgr priority <id> <level>         # Set priority
taskmgr show <id>                     # Show task details
taskmgr search "query"                # Search tasks by title/description

## Workflow

1. Initialize: `taskmgr init`
2. Add tasks: `taskmgr add "My task" --priority high`
3. List: `taskmgr list` or `taskmgr list --priority high`
4. Complete: `taskmgr done <id>`

## Notes

- IDs are printed when tasks are created. Capture them for subsequent commands.
- Use `--format json` for machine-readable output.
```

### README.md

Provides project context. The LLM reads this to understand what it's working on.

---

## Scenario Discovery

### Default Location

Scenarios live in the `fixtures/` directory at the project root. Each scenario is a `.yaml` file.

```
fixtures/
├── task_manager_organize/         # Fixture directory
│   ├── AGENTS.md
│   └── README.md
├── task_manager_organize.yaml     # Scenario file
├── file_organizer_sort/
│   ├── AGENTS.md
│   └── README.md
└── file_organizer_sort.yaml
```

### Listing Scenarios

```bash
ax-eval scenarios
ax-eval scenarios --tags crud
ax-eval scenarios --tier 0
```

### Filtering

- `--tags`: repeat the flag for multiple tags; matches scenarios with any of the given tags
- `--tier`: runs scenarios at or below the given tier (0 = smoke tests only, 1 = smoke + quick, etc.)

---

## Artifacts

Each run produces a results directory:

```
ax-eval-results/<timestamp>-<agent>-<model>-<scenario>/
├── metrics.json            # Machine-readable evaluation profile
├── evaluation.md           # Human-readable evaluation profile
├── report.md               # Execution details, guardrail results, efficiency metrics
├── artifacts/
│   ├── transcript.raw.txt  # Complete output from the LLM agent session
│   ├── events.jsonl        # Structured event log
│   ├── tool-output.raw.txt # Raw adapter output when available
│   └── command-events.json # Normalized command events when available
└── fixture/                # The working directory, preserved after the run
    ├── AGENTS.md            # (from template)
    ├── README.md            # (from template)
    └── ...                  # Any files created or modified by the LLM agent
```

### `metrics.json`

```json
{
  "gates_passed": 4,
  "gates_total": 4,
  "details": [
    {
      "gate_type": "command_succeeds",
      "passed": true,
      "message": "Command succeeded: my-tool list --format json"
    },
    {
      "gate_type": "script",
      "passed": true,
      "message": "Quality script passed"
    }
  ],
  "judge_score": 0.83,
  "judge_passed": true,
  "judge_threshold": 0.70,
  "efficiency": {
    "total_commands": 6,
    "unique_commands": 5,
    "error_count": 0,
    "retry_count": 1,
    "help_invocations": 1,
    "first_try_success_rate": 1.0,
    "iteration_ratio": 0.83,
    "completed": true
  },
  "interaction_evidence_source": "structured_tool_calls",
  "composite_score": 0.91,
  "evaluator_results": [
    {
      "name": "task_quality",
      "score": 0.86,
      "summary": "Good task coverage"
    }
  ]
}
```

### `fixture/`

The working directory is preserved inside the results directory after the run completes. It contains the original template files plus any files created or modified by the LLM agent during the scenario. Gates run against this directory, and it serves as the complete post-run state for manual inspection — no separate snapshot mechanism is needed.

---

## Guidance Testing Workflow

For AGENTS.md authors evaluating which guidance produces the best LLM behavior.

### Approach

Create multiple fixture variants with different AGENTS.md content, run the same task prompt against each, and compare results.

### Setup

```
fixtures/
├── notes_guidance_minimal/
│   ├── AGENTS.md              # Bare minimum: just command names
│   ├── README.md
│   └── notes                  # Mock CLI tool
├── example_guidance_minimal.yaml
├── notes_guidance_rich/
│   ├── AGENTS.md              # Full docs with examples, workflows, error handling
│   ├── README.md
│   └── notes                  # Same mock CLI tool
└── example_guidance_rich.yaml
```

Each scenario YAML uses the **same** task prompt and evaluation gates, but points to a different `template_folder`.

### Running

```bash
ax-eval run --all --tags guidance-test --tier 1
```

### Comparing

Compare `metrics.json` across variants. The primary signal is **interaction metrics** (Layer 1) and any configured qualitative judge results, not gate pass rate:

| Metric | What it tells you |
|--------|-------------------|
| **Error rate** | Lower with better docs. High error rate means the LLM is guessing wrong. |
| **Retry rate** | Lower with better docs. High retry means error messages aren't helping recovery. |
| **Help-seeking** | Lower with better docs. High help-seeking means the AGENTS.md lacks upfront information. |
| **First-try success** | Higher with better docs. Measures how often the LLM gets commands right the first time. |
| **Command count** | Lower with better docs (for the same outcome). Measures efficiency. |
| **Gate pass rate** | Guardrail only. It should be similar across variants if the task is achievable. Differences here indicate the task itself may be too hard or too easy, not that one interaction was automatically better. |

The `evaluation.md` reports are human-readable and can be compared side by side.

**Important**: Keep gates minimal for guidance evaluation. Gates should verify the outcome (e.g., notes were created and linked), not the process (e.g., no errors were made, exact search terms were used). Gates are guardrails, not the primary evaluation signal. See `docs/evaluation.md` for gate design philosophy.

### What to Vary

- **Verbosity**: minimal vs. detailed command documentation
- **Examples**: with vs. without usage examples
- **Structure**: flat list vs. grouped by workflow
- **Error guidance**: with vs. without error handling docs
- **Constraints**: explicit constraints vs. implicit conventions
