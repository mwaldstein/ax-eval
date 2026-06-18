# CLI Command Reference

Generated from `src/cli.rs`. Do not edit by hand.
Run `scripts/generate-cli-reference.sh` to regenerate.

## ax-eval

```
ax-eval runs coding agents against reproducible CLI scenarios and writes evaluation profiles.

Use it to improve CLI help, docs, and AGENTS.md guidance by seeing whether agents complete the task, how many wrong turns they take, what they spend, and which artifacts changed.

Common commands:
  ax-eval scenarios
  ax-eval template scenario > ax-eval-fixtures/my_scenario.yaml
  ax-eval validate --scenario ax-eval-fixtures/my_scenario.yaml
  ax-eval guidance list
  ax-eval guidance start
  AX_EVAL_ENABLED=1 ax-eval discover mytool --tool opencode
  AX_EVAL_ENABLED=1 ax-eval run --scenario my_scenario --tool opencode
  ax-eval show <run-id>

Use `ax-eval template <kind>` for copyable schema examples.

Usage: ax-eval [OPTIONS] <COMMAND>

Commands:
  run        Run a test scenario
  discover   Discover how well a target CLI describes itself to LLM agents
  scenarios  List available scenarios
  show       Show details for a saved run
  clean      Clean cache and legacy transcript artifacts
  guidance   Show guidance for building LLM-usable tools and docs
  validate   Validate scenario YAML without running
  template   Print copyable scenario, config, rubric, and script templates
  help       Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose
          Enable verbose output (or set RUST_LOG for fine-grained control)

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

Common commands:
  ax-eval scenarios
  ax-eval template scenario > ax-eval-fixtures/my_scenario.yaml
  ax-eval template config > ax-eval-config.toml
  ax-eval validate --scenario ax-eval-fixtures/my_scenario.yaml
  ax-eval guidance start
  AX_EVAL_ENABLED=1 ax-eval discover mytool --tool opencode
  AX_EVAL_ENABLED=1 ax-eval run --scenario my_scenario --tool opencode
  ax-eval show <run-id>
```

## ax-eval run

```
Run one scenario, all selected scenarios, or a configured tool/model matrix.

Real agent execution is disabled unless AX_EVAL_ENABLED=1 is set, because real adapters may spend LLM API credits and execute agent-driven CLI commands. Use --dry-run without that environment variable to validate scenario loading, fixture setup, cache keys, and run planning without invoking an LLM agent.

The harness does not execute the target binary — the LLM agent does. Ensure it is on PATH or in the fixture workspace so the agent can discover and use it naturally.

Rubric paths in `evaluation.judge.rubric` resolve in this order: relative to the scenario YAML, then the workspace (after fixture copy), then fixtures_path.

Artifacts are written under ax-eval-results/ by default, including reports, transcripts, metrics, and the isolated fixture workspace.

Usage: ax-eval run [OPTIONS]

Options:
  -s, --scenario <SCENARIO>
          Path to scenario file or name

      --all
          Run all scenarios in ax-eval-fixtures directory

      --tags <TAGS>
          Filter scenarios by tags

      --tier <TIER>
          Filter scenarios by tier (0=smoke, 1=quick, 2=standard, 3=comprehensive)
          
          [default: 0]

      --tool <TOOL>
          Tool to test (e.g., claude-code, opencode)

      --model <MODEL>
          Model to use with the tool (e.g., claude-sonnet-4-20250514, gpt-4o)

      --profile <PROFILE>
          Profile to use for matrix run (defined in config)

      --dry-run
          Dry run (don't execute LLM calls)

      --cache
          Enable result caching (skip re-runs for unchanged scenario/tool/model)

  -v, --verbose
          Enable verbose output (or set RUST_LOG for fine-grained control)

      --judge-model <JUDGE_MODEL>
          Judge model for LLM-as-judge evaluation

      --judge-tool <JUDGE_TOOL>
          Tool to use for LLM-as-judge evaluation (defaults to judge config or opencode)

      --no-judge
          Disable LLM-as-judge evaluation

      --timeout-secs <TIMEOUT_SECS>
          Maximum execution time in seconds per command
          
          [default: 300]

  -h, --help
          Print help (see a summary with '-h')

Examples:
  AX_EVAL_ENABLED=1 ax-eval run --scenario ax-eval-fixtures/my_scenario.yaml --tool opencode
  PATH="$PWD/target/debug:$PATH" AX_EVAL_ENABLED=1 ax-eval run --scenario ax-eval-fixtures/my_scenario.yaml --tool opencode
  AX_EVAL_ENABLED=1 ax-eval run --all --tags smoke --tier 1 --tool claude-code
  AX_EVAL_ENABLED=1 ax-eval run --scenario ax-eval-fixtures/my_scenario.yaml --profile quick
  ax-eval run --scenario ax-eval-fixtures/my_scenario.yaml --dry-run

Start with `ax-eval template scenario` for a copyable scenario schema.
```

## ax-eval discover

```
Run an all-in-one discovery workflow for a target executable. Discovery asks an LLM agent to understand the target command, author five complex goal-oriented scenarios, run the generated scenario batch, judge usage quality, and summarize the results.

Real agent execution is disabled unless AX_EVAL_ENABLED=1 is set, because discovery may spend LLM API credits and execute agent-driven CLI commands.

Usage: ax-eval discover [OPTIONS] <TARGET>

Arguments:
  <TARGET>
          Target executable binary or command to discover

Options:
      --tool <TOOL>
          Agent tool to evaluate in generated scenarios
          
          [default: opencode]

      --model <MODEL>
          Model to evaluate in generated scenarios

      --discover-tool <DISCOVER_TOOL>
          Agent tool used for inspect, fixture authoring, and final summary

      --discover-model <DISCOVER_MODEL>
          Agent model used for inspect, fixture authoring, and final summary

      --judge-model <JUDGE_MODEL>
          Judge model for LLM-as-judge evaluation

      --judge-tool <JUDGE_TOOL>
          Tool to use for LLM-as-judge evaluation

      --timeout-secs <TIMEOUT_SECS>
          Maximum execution time in seconds per agent command
          
          [default: 300]

  -v, --verbose
          Enable verbose output (or set RUST_LOG for fine-grained control)

  -h, --help
          Print help (see a summary with '-h')

Example:
  AX_EVAL_ENABLED=1 ax-eval discover mytool --tool opencode

Use --discover-tool/--discover-model when the agent authoring the discovery artifacts should differ from the evaluated scenario-run agent.
```

## ax-eval scenarios

```
List available scenarios

Usage: ax-eval scenarios [OPTIONS]

Options:
      --tags <TAGS>  Filter by tags
      --tier <TIER>  Filter scenarios by tier (0=smoke, 1=quick, 2=standard, 3=comprehensive) [default: 0]
  -v, --verbose      Enable verbose output (or set RUST_LOG for fine-grained control)
  -h, --help         Print help
```

## ax-eval show

```
Show details for a saved run

Usage: ax-eval show [OPTIONS] <ID>

Arguments:
  <ID>  Run ID to look up

Options:
  -v, --verbose  Enable verbose output (or set RUST_LOG for fine-grained control)
  -h, --help     Print help
```

## ax-eval clean

```
Clean cache and legacy transcript artifacts

Usage: ax-eval clean [OPTIONS]

Options:
      --older-than <OLDER_THAN>  Clean artifacts older than duration (e.g., "30d", "7d", "1h")
  -v, --verbose                  Enable verbose output (or set RUST_LOG for fine-grained control)
  -h, --help                     Print help
```

## ax-eval validate

```
Validate one or more scenario files for schema correctness.

Checks YAML syntax, required fields, gate configuration, regex compilation, and judge setup. No fixture setup, no LLM spend, no agent execution.

With --scenario, the given file is always validated regardless of whether it looks like a scenario.
With --all, YAML files are scanned recursively and a lightweight heuristic (at least two distinctive scenario keys: name, target, task, evaluation, or template_folder) is used to skip non-scenario files such as rubrics. This matches the discovery logic used by `run --all`.

Usage: ax-eval validate [OPTIONS]

Options:
  -s, --scenario <SCENARIO>
          Path to scenario file or name (always validated, even if not scenario-like)

      --all
          Validate all scenario-like YAML files in ax-eval-fixtures directory

  -v, --verbose
          Enable verbose output (or set RUST_LOG for fine-grained control)

  -h, --help
          Print help (see a summary with '-h')

Examples:
  ax-eval validate --scenario ax-eval-fixtures/my_scenario.yaml
  ax-eval validate --all
  ax-eval validate --scenario ax-eval-fixtures/my_scenario.yaml --verbose
```

## ax-eval guidance

```
Show guidance for building LLM-usable tools and docs

Usage: ax-eval guidance [OPTIONS] <COMMAND>

Commands:
  list  List available guidance topics
  show  Show one or more guidance topics
  help  Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose  Enable verbose output (or set RUST_LOG for fine-grained control)
  -h, --help     Print help
```

## ax-eval template

```
Print copyable scenario, config, rubric, and script templates

Usage: ax-eval template [OPTIONS] <KIND>

Arguments:
  <KIND>
          Template to print

          Possible values:
          - scenario:    Scenario YAML with target, task, setup, scripts, gates, judge, and matrix fields
          - config:      ax-eval-config.toml with supported config fields and valid profiles
          - script-gate: Shell script gate that reports pass/fail JSON
          - evaluator:   Custom evaluator script that reports metrics, score, and summary JSON
          - rubric:      Rubric YAML with weighted criteria and judge output contract

Options:
  -v, --verbose
          Enable verbose output (or set RUST_LOG for fine-grained control)

  -h, --help
          Print help (see a summary with '-h')
```

