# CLI Command Reference

Generated from `src/cli.rs`. Do not edit by hand.
Run `scripts/generate-cli-reference.sh` to regenerate.

## ax-eval

```
ax-eval runs coding agents against reproducible CLI scenarios and writes evaluation profiles.

Use it to improve CLI help, docs, and AGENTS.md guidance by seeing whether agents complete the task, how many wrong turns they take, what they spend, and which artifacts changed.

Common commands:
  ax-eval scenarios
  ax-eval template scenario > fixtures/my_scenario.yaml
  ax-eval guidance list
  ax-eval guidance start
  AX_EVAL_ENABLED=1 ax-eval discover qipu --tool opencode
  AX_EVAL_ENABLED=1 ax-eval run --scenario my_scenario --tool opencode
  ax-eval show <run-id>

Use `ax-eval template <kind>` for copyable schema examples.

Usage: ax-eval <COMMAND>

Commands:
  run        Run a test scenario
  discover   Discover how well a target CLI describes itself to LLM agents
  scenarios  List available scenarios
  show       Show details for a saved run ID
  clean      Clean cache and legacy transcript artifacts
  guidance   Show guidance for building LLM-usable tools and docs
  template   Print copyable scenario, config, and script templates
  help       Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

Common commands:
  ax-eval scenarios
  ax-eval template scenario > fixtures/my_scenario.yaml
  ax-eval template config > ax-eval-config.toml
  ax-eval guidance start
  AX_EVAL_ENABLED=1 ax-eval discover qipu --tool opencode
  AX_EVAL_ENABLED=1 ax-eval run --scenario my_scenario --tool opencode
  ax-eval show <run-id>
```

## ax-eval run

```
Run one scenario, all selected scenarios, or a configured tool/model matrix.

Real agent execution is disabled unless AX_EVAL_ENABLED=1 is set, because real adapters may spend LLM API credits and execute agent-driven CLI commands. Use --dry-run without that environment variable to validate scenario loading, fixture setup, cache keys, and run planning without invoking an LLM agent.

Artifacts are written under ax-eval-results/ by default, including reports, transcripts, metrics, and the isolated fixture workspace.

Usage: ax-eval run [OPTIONS]

Options:
  -s, --scenario <SCENARIO>
          Path to scenario file or name

      --all
          Run all scenarios in fixtures directory

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

      --no-cache
          Disable caching

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
  AX_EVAL_ENABLED=1 ax-eval run --scenario fixtures/my_scenario.yaml --tool opencode
  PATH="$PWD/target/debug:$PATH" AX_EVAL_ENABLED=1 ax-eval run --scenario fixtures/my_scenario.yaml --tool opencode
  AX_EVAL_ENABLED=1 ax-eval run --all --tags smoke --tier 1 --tool claude-code
  AX_EVAL_ENABLED=1 ax-eval run --scenario fixtures/my_scenario.yaml --profile quick
  ax-eval run --scenario fixtures/my_scenario.yaml --dry-run

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

  -h, --help
          Print help (see a summary with '-h')

Example:
  AX_EVAL_ENABLED=1 ax-eval discover qipu --tool opencode

Use --discover-tool/--discover-model when the agent authoring the discovery artifacts should differ from the evaluated scenario-run agent.
```

## ax-eval scenarios

```
List available scenarios

Usage: ax-eval scenarios [OPTIONS]

Options:
      --tags <TAGS>  Filter by tags
      --tier <TIER>  Filter scenarios by tier (0=smoke, 1=quick, 2=standard, 3=comprehensive) [default: 0]
  -h, --help         Print help
```

## ax-eval show

```
Show details for a saved run ID

Usage: ax-eval show <NAME>

Arguments:
  <NAME>  Name of the scenario

Options:
  -h, --help  Print help
```

## ax-eval clean

```
Clean cache and legacy transcript artifacts

Usage: ax-eval clean [OPTIONS]

Options:
      --older-than <OLDER_THAN>  Clean artifacts older than duration (e.g., "30d", "7d", "1h")
  -h, --help                     Print help
```

## ax-eval guidance

```
Show guidance for building LLM-usable tools and docs

Usage: ax-eval guidance <COMMAND>

Commands:
  list  List available guidance topics
  show  Show one or more guidance topics
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

## ax-eval template

```
Print copyable scenario, config, and script templates

Usage: ax-eval template <KIND>

Arguments:
  <KIND>
          Template to print

          Possible values:
          - scenario:    Scenario YAML with target, task, setup, scripts, gates, judge, and matrix fields
          - config:      ax-eval-config.toml with supported config fields and valid profiles
          - script-gate: Shell script gate that reports pass/fail JSON
          - evaluator:   Custom evaluator script that reports metrics, score, and summary JSON

Options:
  -h, --help
          Print help (see a summary with '-h')
```

