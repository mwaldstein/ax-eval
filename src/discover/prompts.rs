use std::path::Path;

pub(super) fn inspect_prompt(target: &str) -> String {
    format!(
        r#"You are inspecting a CLI tool for ax-eval discovery.

Target executable: {target}

Work only from the executable command surface. Use the target command's help,
subcommand help, examples, errors, and any files the command itself points you
to. Do not rely on external documentation unless the command explicitly opens
or names it.

Create a Markdown file at ./understanding.md. Do this before attempting any
destructive or permission-sensitive experiments. Keep all scratch work inside
the current directory; do not use /tmp, parent directories, or other external
paths.

Focus on why the tool exists and how an LLM agent should think about using it.
Use exactly these section headings:

# Discovery Understanding

## What the Tool Appears to Be For

## Core Concepts and Mental Model

## Primary Workflows

## Useful Goal Areas

## Evidence Consulted

## Self-Description Quality

## Ambiguities or Missing Information

## Five Candidate Scenario Ideas

The final file must be a synthesized Markdown report, not a command transcript.
If an exploratory command is denied or fails, still write ./understanding.md
from the evidence already collected and mention the limitation under
Ambiguities or Missing Information.

Keep the artifact useful to the tool author as evidence of how self-describing
the executable is."#
    )
}

pub(super) fn understanding_repair_prompt(
    target: &str,
    transcript_path: &Path,
    understanding_path: &Path,
) -> String {
    format!(
        r#"You are repairing the inspect artifact for ax-eval discovery.

Target executable: {target}
Inspect transcript: {}
Required output file: {}

Read the inspect transcript and synthesize the required Markdown understanding
artifact. Work only from the transcript and any command-surface evidence named
inside it. Do not run more experiments unless absolutely necessary, and keep any
scratch files inside the current directory.

Overwrite the required output file with a synthesized Markdown report using
exactly these section headings:

# Discovery Understanding

## What the Tool Appears to Be For

## Core Concepts and Mental Model

## Primary Workflows

## Useful Goal Areas

## Evidence Consulted

## Self-Description Quality

## Ambiguities or Missing Information

## Five Candidate Scenario Ideas

The output must not be a command transcript. Include concise evidence references
instead of pasting command output."#,
        transcript_path.display(),
        understanding_path.display()
    )
}

pub(super) fn author_prompt(target: &str, count: usize, understanding_path: &Path) -> String {
    format!(
        r#"You are authoring ax-eval discovery fixtures.

Target executable: {target}
Understanding document: {}
Required scenario count: {count}

Read the understanding document, then create a coordinated set of {count}
complex, goal-oriented ax-eval scenarios under ./scenarios/.

Hard requirements:
- Write complete runnable YAML scenario files under ./scenarios/.
- Create any required template directories beside the YAML files.
- Each scenario must use target.binary: "{target}".
- Each scenario must have evaluation.gates: [].
- Each scenario must include evaluation.judge.enabled: true.
- Omit evaluation.judge.rubric unless the scenario needs custom criteria. The
  default judge rubric assesses goal achievement, CLI usage quality, and
  efficiency.
- If custom criteria are needed, prefer inline evaluation.judge.criteria. Use a
  separate rubric file only when the criteria need a reusable output contract.
- Use pass_threshold as a general rubric reference, not as the main discovery
  value. 0.70 is a reasonable default.
- Prefer goal-based tasks over command-prescriptive tasks. The prompt should
  ask for an outcome that requires understanding the tool's role, not a fixed
  command sequence.
- Avoid near-duplicate scenarios; cover different useful goal areas.
- If extra notes would help the tool author, write ./scenarios/README.md.

Useful scenario shape:

name: discover_example_goal
description: "Goal-oriented discovery scenario"
template_folder: templates/discover_example_goal
target:
  binary: "{target}"
task:
  prompt: |
    Achieve a realistic user goal with the target tool. Decide which commands
    are appropriate and verify your work.
evaluation:
  gates: []
  judge:
    enabled: true
    pass_threshold: 0.70
tags:
  - discovery

Use relative template paths beside each scenario file. If you create a custom
rubric path, keep it relative to the generated YAML; the harness will resolve it."#,
        understanding_path.display()
    )
}

pub(super) fn summary_prompt(understanding_path: &Path, manifest_path: &Path) -> String {
    format!(
        r#"You are summarizing an ax-eval discovery run for the target tool author.

Read:
- Understanding: {}
- Discovery manifest: {}
- Scenario run reports and evaluations under ./runs/

Write ./discovery-summary.md as a concise but useful Markdown report. Focus on:
- What the inspecting agent understood about why the tool exists
- How self-describing the tool appeared to be
- Quality and diversity of the generated goal-oriented scenarios
- How well the evaluated agent used the target tool, emphasizing judge scores,
  judge rationales, confidence, issues, highlights, failed calls, retries, and
  help-seeking rather than only pass/fail
- Which failures are fixture-authoring problems, tool self-description
  problems, agent usage problems, or harness problems
- High-impact recommendations for improving the tool's LLM usability"#,
        understanding_path.display(),
        manifest_path.display()
    )
}
