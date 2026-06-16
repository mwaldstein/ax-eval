# Scripts

**Status: Stable**

## Purpose

Define a scripts system that lets scenario authors run custom logic at defined points in the evaluation lifecycle. Scripts are the primary extension mechanism — they keep the framework core generic while allowing tool-specific behavior to live outside the framework.

Scripts replace the need for domain-specific gates, custom snapshot commands, and hardcoded evaluation logic. The framework provides the lifecycle hooks and a structured output contract; the scenario author provides the scripts.

---

## Design Principles

1. **Scripts are shell commands or executable files.** No plugin API, no dynamic loading. If it runs in a shell, it works.
2. **Scripts run in the fixture directory.** They have access to the working directory and everything the agent created or modified.
3. **Scripts receive context via environment variables.** The framework sets variables with scenario metadata, paths, and run information.
4. **Scripts communicate results via exit codes and stdout.** Exit code 0 = success. Structured JSON on stdout when the hook type requires it.

---

## Lifecycle Hooks

Scripts can run at four points in the execution flow:

```
1. Setup commands         (existing: setup.commands)
2. [Agent runs]
3. Post-execution scripts (new: scripts.post)
4. Custom gates           (new: gate type "script")
   Custom evaluators      (new: scripts.evaluators)
5. [Artifacts written]
```

### Hook 1: Setup Commands (existing)

Already defined in the scenario format as `setup.commands`. These run before the agent launches. No changes needed — this is already the "pre-execution" hook.

```yaml
setup:
  commands:
    - "mytool init"
    - "mytool import seed-data.json"
```

### Hook 2: Post-Execution Scripts

Run after the agent exits (or is killed by timeout), before evaluation. Use these for:

- Exporting tool state to a file for gate inspection
- Running tool-specific cleanup or normalization
- Capturing state that isn't file-based (e.g., database dumps)
- Transforming tool output into formats gates can check

```yaml
scripts:
  post:
    - command: "mytool export --format json > .tool-export.json"
    - command: "./scripts/normalize-output.sh"
    - command: "mytool doctor --json > .doctor-report.json"
```

Post scripts run sequentially in declaration order. If a post script fails (non-zero exit), the failure is logged as a warning but evaluation still proceeds — the point of post scripts is data capture, not pass/fail gating.

### Hook 3: Script Gates

A new gate type that runs a script as an evaluation assertion. More powerful than `command_succeeds` because script gates return structured results.

```yaml
evaluation:
  gates:
    - type: script
      command: "./scripts/check-task-count.sh"
      description: "At least 3 tasks were created"

    - type: script
      command: "./scripts/validate-links.sh"
      description: "All cross-references resolve"
```

#### Script Gate Contract

- **Exit code 0** = pass, **non-zero** = fail.
- **Stdout** (optional): JSON with additional detail:

```json
{
  "passed": true,
  "message": "Found 5 tasks (minimum 3)"
}
```

If stdout is valid JSON with a `passed` field, the framework uses it. If stdout is not JSON or has no `passed` field, the framework falls back to exit code only. The `message` field, when present, is included in the gate result. Additional JSON fields are currently ignored.

This means a script gate can be as simple as:

```bash
#!/bin/bash
# check-task-count.sh
count=$(mytool list --format json | jq length)
[ "$count" -ge 3 ]
```

Or as rich as:

```bash
#!/bin/bash
# check-task-count.sh
count=$(mytool list --format json | jq length)
if [ "$count" -ge 3 ]; then
  echo "{\"passed\": true, \"message\": \"Found $count tasks (minimum 3)\"}"
  exit 0
else
  echo "{\"passed\": false, \"message\": \"Found $count tasks, need at least 3\"}"
  exit 1
fi
```

### Hook 4: Custom Evaluators

Scripts that produce metrics or scores, independent of gates and the LLM judge. Use these for tool-specific quality analysis that doesn't fit the pass/fail gate model.

```yaml
scripts:
  evaluators:
    - command: "./scripts/measure-quality.sh"
      name: "store_quality"
```

#### Evaluator Contract

- **Exit code 0** = evaluator ran successfully (does not imply pass/fail).
- **Stdout**: JSON with metrics:

```json
{
  "metrics": {
    "avg_title_length": 12.5,
    "orphan_count": 2,
    "link_density": 0.75
  },
  "score": 0.82,
  "summary": "Good overall structure with 2 orphaned items"
}
```

All fields are optional:
- `metrics`: arbitrary key-value pairs, stored in `metrics.json` under the evaluator's `name`.
- `score`: a 0.0–1.0 score, included in the evaluation report.
- `summary`: human-readable summary, included in `evaluation.md`.

If the evaluator exits non-zero, the failure is logged but does not affect the run outcome. Evaluators are diagnostic, not gating.

---

## Environment Variables

All scripts receive these environment variables:

| Variable | Description |
|----------|-------------|
| `AX_EVAL_FIXTURE_DIR` | Absolute path to the fixture (working) directory |
| `AX_EVAL_RESULTS_DIR` | Absolute path to the results directory |
| `AX_EVAL_SCENARIO` | Scenario name |
| `AX_EVAL_AGENT` | LLM agent tool used (e.g., "opencode") |
| `AX_EVAL_MODEL` | Model used (e.g., "gpt-4o") |
| `AX_EVAL_TRANSCRIPT` | Path to `artifacts/transcript.raw.txt` (set when transcript path is available) |
| `AX_EVAL_EVENTS` | Path to `artifacts/events.jsonl` (set when events path is available) |

Scripts also inherit any `target.env` variables defined in the scenario, after
run-directory placeholders such as `${AX_EVAL_FIXTURE_DIR}` and
`${AX_EVAL_RESULTS_DIR}` are expanded.

---

## Scenario Schema Additions

```yaml
scripts:
  post:                            # optional: run after agent exits, before evaluation
    - command: string
  evaluators:                      # optional: produce custom metrics/scores
    - command: string
      name: string                 # key for storing results in metrics.json

evaluation:
  gates:
    - type: script                 # new gate type
      command: string
      description: string          # human-readable description for reports
```

### Complete Example

```yaml
name: task_manager_organize
description: >
  Verify the LLM can use taskmgr to create and organize tasks.

target:
  binary: taskmgr
  command_pattern: "taskmgr\\s+(add|list|done|priority|show|search)"
  health_check: "taskmgr --version"

template_folder: ax-eval-fixtures/task_manager_organize

task:
  prompt: |
    Read the project brief in README.md, then create tasks for each
    deliverable. Set high priority on database-related tasks.

setup:
  commands:
    - "taskmgr init"

scripts:
  post:
    - command: "taskmgr list --format json --all > .task-export.json"
    - command: "taskmgr stats --json > .task-stats.json"
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
```

### Fixture with Scripts

```
ax-eval-fixtures/task_manager_organize/
├── AGENTS.md
├── README.md
└── scripts/
    ├── check-priorities.sh
    ├── check-completion.sh
    └── task-quality.sh
```

Scripts live in the fixture directory alongside AGENTS.md. They are copied into the working directory with the rest of the template, so they are available at the paths referenced in the scenario YAML.

---

## Execution Order

The full lifecycle with scripts:

1. **Load scenario** — parse YAML, resolve target config
2. **Prepare workspace** — copy fixture template to `fixture/` in results dir
3. **Setup commands** — run `setup.commands` sequentially
4. **Launch agent** — adapter runs LLM agent in fixture directory
5. **Capture transcript** — PTY output saved, events extracted
6. **Post-execution scripts** — run `scripts.post` sequentially; failures logged as warnings
7. **Evaluate**:
   a. Interaction metrics — derived from events/transcript
   b. Gates — built-in gates and script gates, all run regardless of earlier failures
   c. LLM-as-judge — if enabled and gates pass
   d. Custom evaluators — run `scripts.evaluators`; results stored in metrics
8. **Generate artifacts** — write metrics, evaluation summary

---

## Timeout and Failure Handling

- **Post scripts** have a default timeout of 30 seconds each. If a post script times out or fails, it is logged and evaluation proceeds.
- **Script gates** have a default timeout of 30 seconds each. If a script gate times out, the gate fails.
- **Evaluators** have a default timeout of 60 seconds each. If an evaluator times out or fails, `metrics.json` includes an evaluator result with `error` and no `metrics` or `score`.
- All script timeouts can be overridden per-script:

```yaml
scripts:
  post:
    - command: "./scripts/slow-export.sh"
      timeout_secs: 120
```

---

## Relationship to Built-in Gates

Script gates do not replace built-in gates. Built-in gates (`command_succeeds`, `file_exists`, `command_json_path`, etc.) are simpler to write, faster to execute, and easier to read in scenario YAML. Use script gates when:

- The assertion logic is too complex for a single command
- You need to check multiple conditions as a unit
- You want structured pass/fail messages in reports
- The check requires multi-step logic (query, parse, compare)

A rule of thumb: if the check fits in a `command_succeeds` one-liner, use that. If it needs a script, use a script gate.

---

## Not In Scope

- **Script dependencies or package management** — scripts manage their own dependencies. If a script needs `jq`, it's the scenario author's responsibility to ensure `jq` is available.
- **Sandboxing** — scripts run with the same permissions as the harness. No isolation beyond the fixture directory.
- **Script registries or sharing** — scripts are local to each fixture. Reuse across scenarios is via copying or symlinks, not a framework mechanism.
- **Async or parallel script execution** — all scripts run sequentially within their hook phase.
