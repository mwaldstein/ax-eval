//! Tests for judge module.

use super::eval::{build_judge_prompt, build_judge_prompt_for_target, JudgeTargetView};
use super::rubric::load_rubric;
use super::types::{Criterion, OutputFormat, Rubric};
use crate::interaction_evidence::McpToolCallEvent;
use crate::scenario::{McpTarget, McpTransport, TargetConfig};

fn single_criterion_rubric() -> Rubric {
    Rubric {
        criteria: vec![Criterion {
            id: "test_criterion".to_string(),
            weight: 1.0,
            description: "Test description".to_string(),
        }],
        output: OutputFormat {
            format: "json".to_string(),
            require_fields: vec!["scores".to_string()],
        },
    }
}

#[test]
fn test_build_judge_prompt_includes_tool_name() {
    let rubric = single_criterion_rubric();
    let prompt = build_judge_prompt("my-tool", "Do the task", "/path/to/transcript.txt", &rubric);

    assert!(prompt.contains("my-tool"));
    assert!(prompt.contains("how effectively an LLM agent used the CLI tool `my-tool`"));
}

#[test]
fn test_build_judge_prompt_cli_wording_is_unchanged() {
    let rubric = single_criterion_rubric();
    let prompt = build_judge_prompt("my-tool", "Do the task", "/path/to/transcript.txt", &rubric);
    let target_aware_prompt = build_judge_prompt_for_target(
        &JudgeTargetView::from_target(&TargetConfig::cli_target("my-tool")),
        "Do the task",
        "",
        "/path/to/transcript.txt",
        &rubric,
        &[],
    );

    let expected = r#"You are evaluating how effectively an LLM agent used the CLI tool `my-tool`.

Read the transcript at @/path/to/transcript.txt, then score the interaction against the criteria below.

## Task the agent was given
Do the task

## Agent guidance the agent was given
None provided — the agent received only the task prompt above.

## Evaluation Criteria
- test_criterion: Test description (weight: 1.00)

## How to score
Do the analysis before you assign any number. Be a demanding reviewer: the top band is for genuinely flawless, expert-level work, and most real runs are not flawless. Do not inflate a score to balance out the highlights.

First, in prose OUTSIDE the <judge_result> tag, write two or three sentences covering:
1. What an ideal run of this task looks like — the essential steps and the correct end state.
2. Every deviation you see in the transcript: invalid or wrong commands, retries, dead ends, missed verification, redundant steps, and — most important — any required part of the task left undone or any incorrect final state.

Then score each criterion 0.0–1.0, and make the scores reflect that list. Use these bands for every criterion:
- 0.90–1.00 — Flawless on this dimension: no material shortcoming.
- 0.70–0.85 — Strong: one minor, recoverable shortcoming.
- 0.50–0.65 — Mixed: repeated problems, though the dimension's goal was ultimately met.
- 0.30–0.45 — Weak: the dimension's goal was only partly met.
- 0.10–0.25 — Poor: the dimension's goal was largely not met.
- 0.00 — Absent: not met at all.

Hard caps — apply these after scoring, overriding the bands above:
- If any required part of the task is left undone or the final state is wrong, that is the dominant fact about the run: score task completion at 0.40 or below AND cap the overall `weighted_score` at 0.40 or below, even if every command was valid and efficient. Incomplete work is not a B.
- A run for which you listed several material issues cannot land in the top two bands (0.70+). If your score and your `issues` disagree, lower the score.

Then compute `weighted_score` as the weighted average across all criteria (subject to the caps above), and fill in:
- `confidence`: how certain you are in your scores (0.0–1.0). Lower it if the transcript is ambiguous or incomplete.
- `issues`: specific problems observed (e.g., "Retried `my-tool create` 3 times with same args").
- `highlights`: specific good practices observed (e.g., "Used `my-tool search` to verify data before proceeding").
- `rationale`: 2–4 sentence explanation of the overall assessment — why the scores are what they are, what the agent did well, and where it struggled.

## Guidance prescriptiveness
Separately from the criteria above, rate how prescriptive the scenario's guidance was — the task prompt plus any agent guidance — taken as a whole. This rates the inputs the agent was handed, not its performance, so do NOT fold it into `weighted_score`; achieving a goal that was spelled out step by step is less impressive than deciding the approach unaided. Use this scale:
- 0 (goal-only): states an outcome or goal and names no tools or steps; the agent must decide everything.
- 1 (light hints): mentions relevant tools or capabilities but not how or when to use them.
- 2 (partial recipe): spells out specific commands or steps for part of the task; the agent fills the gaps.
- 3 (step-by-step): the prompt and/or guidance dictate the exact sequence of tool calls, so success is mostly obedience.
Report the integer `level` and a one-sentence `rationale` under `prescriptiveness`.

Return one valid JSON object with this exact structure:
{
  "scores": {
    "criterion_id": <score_0_to_1>,
    ...
  },
  "weighted_score": <weighted_average_0_to_1>,
  "confidence": <confidence_0_to_1>,
  "issues": ["issue1", "issue2", ...],
  "highlights": ["good_practice1", "good_practice2", ...],
  "rationale": "<2-4 sentence explanation of the overall assessment>",
  "prescriptiveness": { "level": <0_to_3>, "rationale": "<one sentence>" }
}

Wrap only that JSON object in a single <judge_result> tag:
<judge_result>
{ ...the JSON object above... }
</judge_result>

Do not put prose, markdown, or code fences inside <judge_result>. If you need to say anything else, put it outside the tag."#;

    assert_eq!(prompt, expected);
    assert_eq!(target_aware_prompt, expected);
}

#[test]
fn test_build_judge_prompt_includes_task_and_transcript() {
    let rubric = single_criterion_rubric();
    let prompt = build_judge_prompt("notes", "Create 3 notes", "/tmp/transcript.txt", &rubric);

    assert!(prompt.contains("Create 3 notes"));
    assert!(prompt.contains("/tmp/transcript.txt"));
}

#[test]
fn test_build_judge_prompt_includes_criteria() {
    let rubric = single_criterion_rubric();
    let prompt = build_judge_prompt("notes", "task", "/t.txt", &rubric);

    assert!(prompt.contains("test_criterion"));
    assert!(prompt.contains("Test description"));
}

#[test]
fn test_build_judge_prompt_multiple_criteria() {
    let rubric = Rubric {
        criteria: vec![
            Criterion {
                id: "correctness".to_string(),
                weight: 0.40,
                description: "Uses valid commands".to_string(),
            },
            Criterion {
                id: "efficiency".to_string(),
                weight: 0.60,
                description: "Minimal wasted effort".to_string(),
            },
        ],
        output: OutputFormat {
            format: "json".to_string(),
            require_fields: vec!["scores".to_string(), "weighted_score".to_string()],
        },
    };

    let prompt = build_judge_prompt("taskmgr", "Create tasks", "/tmp/t.txt", &rubric);

    assert!(prompt.contains("taskmgr"));
    assert!(prompt.contains("correctness"));
    assert!(prompt.contains("efficiency"));
    assert!(prompt.contains("0.40"));
    assert!(prompt.contains("0.60"));
}

#[test]
fn test_build_judge_prompt_includes_agent_guidance_when_provided() {
    let rubric = single_criterion_rubric();
    let prompt = build_judge_prompt_for_target(
        &JudgeTargetView::from_target(&TargetConfig::cli_target("notes")),
        "Do the task",
        "# Notes CLI\nRun `notes add` to create a note.",
        "/t.txt",
        &rubric,
        &[],
    );

    assert!(prompt.contains("## Agent guidance the agent was given"));
    assert!(prompt.contains("Run `notes add` to create a note."));
    assert!(!prompt.contains("None provided —"));
}

#[test]
fn test_build_judge_prompt_requests_prescriptiveness_rating() {
    let rubric = single_criterion_rubric();
    let prompt = build_judge_prompt("notes", "task", "/t.txt", &rubric);

    assert!(prompt.contains("## Guidance prescriptiveness"));
    assert!(prompt.contains("0 (goal-only)"));
    assert!(prompt.contains("3 (step-by-step)"));
    assert!(prompt
        .contains(r#""prescriptiveness": { "level": <0_to_3>, "rationale": "<one sentence>" }"#));
}

#[test]
fn test_build_judge_prompt_requests_rationale() {
    let rubric = single_criterion_rubric();
    let prompt = build_judge_prompt("notes", "task", "/t.txt", &rubric);

    assert!(prompt.contains("rationale"));
}

#[test]
fn test_build_judge_prompt_requests_judge_result_envelope() {
    let rubric = single_criterion_rubric();
    let prompt = build_judge_prompt("notes", "task", "/t.txt", &rubric);

    assert!(prompt.contains("<judge_result>"));
    assert!(prompt.contains("</judge_result>"));
    assert!(prompt.contains("Do not put prose, markdown, or code fences inside <judge_result>"));
}

#[test]
fn test_build_judge_prompt_uses_tool_name_in_examples() {
    let rubric = single_criterion_rubric();
    let prompt = build_judge_prompt("qipu", "task", "/t.txt", &rubric);

    assert!(prompt.contains("`qipu create`"));
    assert!(prompt.contains("`qipu search`"));
}

#[test]
fn test_build_judge_prompt_for_mcp_includes_target_summary_and_structured_arguments() {
    let rubric = single_criterion_rubric();
    let target = TargetConfig::Mcp(McpTarget {
        name: "todo".to_string(),
        transport: McpTransport::Stdio {
            command: "todo-mcp".to_string(),
            args: vec![],
        },
        auth: None,
        tools: vec![
            "add".to_string(),
            "list".to_string(),
            "complete".to_string(),
        ],
        env: None,
        health_check: None,
    });
    let events = vec![McpToolCallEvent {
        server: "todo".to_string(),
        tool: "add".to_string(),
        arguments: serde_json::json!({"text":"buy milk","priority":2}),
        is_error: false,
        duration_ms: Some(14),
    }];

    let prompt = build_judge_prompt_for_target(
        &JudgeTargetView::from_target(&target),
        "Create a todo",
        "",
        "/tmp/transcript.txt",
        &rubric,
        &events,
    );

    assert!(prompt.contains("the MCP server `todo` exposing tools [add, list, complete]"));
    assert!(prompt.contains("## Structured MCP tool-call excerpt"));
    assert!(prompt.contains(r#"- add arguments={"priority":2,"text":"buy milk"} error=false"#));
}

#[test]
fn test_build_judge_prompt_for_mcp_bounds_structured_excerpt() {
    let rubric = single_criterion_rubric();
    let target = TargetConfig::Mcp(McpTarget {
        name: "todo".to_string(),
        transport: McpTransport::Stdio {
            command: "todo-mcp".to_string(),
            args: vec![],
        },
        auth: None,
        tools: vec!["add".to_string()],
        env: None,
        health_check: None,
    });
    let events = (0..52)
        .map(|index| McpToolCallEvent {
            server: "todo".to_string(),
            tool: "add".to_string(),
            arguments: serde_json::json!({ "index": index }),
            is_error: index % 2 == 1,
            duration_ms: None,
        })
        .collect::<Vec<_>>();

    let prompt = build_judge_prompt_for_target(
        &JudgeTargetView::from_target(&target),
        "Create todos",
        "",
        "/tmp/transcript.txt",
        &rubric,
        &events,
    );

    assert!(prompt.contains("First 50 of 52 captured MCP tool calls are shown (bound: 50):"));
    assert!(prompt.contains(r#"- add arguments={"index":49} error=true"#));
    assert!(!prompt.contains(r#"- add arguments={"index":50} error=false"#));
}

#[test]
fn test_load_rubric_weights_sum_to_one() {
    let rubric_yaml = r#"
criteria:
  - id: relevance
    weight: 0.35
    description: "Notes directly address the task prompt"
  - id: coherence
    weight: 0.35
    description: "Notes are logically connected"
  - id: granularity
    weight: 0.30
    description: "Notes are appropriately scoped"
output:
  format: json
  require_fields:
    - scores
    - weighted_score
"#;
    let temp_dir = tempfile::tempdir().unwrap();
    let rubric_path = temp_dir.path().join("test_rubric.yaml");
    std::fs::write(&rubric_path, rubric_yaml).unwrap();

    let rubric = load_rubric(&rubric_path).unwrap();
    assert_eq!(rubric.criteria.len(), 3);
    assert_eq!(rubric.criteria[0].id, "relevance");
    assert_eq!(rubric.criteria[0].weight, 0.35);
}

#[test]
fn test_load_rubric_weights_sum_error() {
    let rubric_yaml = r#"
criteria:
  - id: criterion1
    weight: 0.5
    description: "First criterion"
  - id: criterion2
    weight: 0.4
    description: "Second criterion"
output:
  format: json
  require_fields:
    - scores
"#;
    let temp_dir = tempfile::tempdir().unwrap();
    let rubric_path = temp_dir.path().join("bad_rubric.yaml");
    std::fs::write(&rubric_path, rubric_yaml).unwrap();

    let result = load_rubric(&rubric_path);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must sum to 1.0"));
}
