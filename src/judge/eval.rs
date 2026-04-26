//! Judge prompt construction for CLI-based LLM-as-judge evaluation.
//!
//! The judge is executed via the same CLI tool adapter framework used to run
//! scenarios — not via direct API calls. This module builds the prompt that
//! gets passed to the CLI tool.

use crate::judge::types::Rubric;

/// Build the judge prompt for CLI-based evaluation.
///
/// Constructs a prompt containing the tool name, task description, transcript
/// file reference, rubric criteria, and required JSON response format.
/// This prompt is passed to the CLI tool (e.g., `opencode run <prompt>`).
pub fn build_judge_prompt(
    tool_name: &str,
    task_description: &str,
    transcript_path: &str,
    rubric: &Rubric,
) -> String {
    let criteria_text = rubric
        .criteria
        .iter()
        .map(|c| format!("- {}: {} (weight: {:.2})", c.id, c.description, c.weight))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are evaluating how effectively an LLM agent used the CLI tool `{tool_name}`.

Read the transcript at @{transcript_path}, then score the interaction against the criteria below.

## Task the agent was given
{task_description}

## Evaluation Criteria
{criteria_text}

## Scoring Guidelines
- Score each criterion 0.0–1.0. 0.0 = complete failure, 0.5 = partial, 1.0 = excellent.
- Compute `weighted_score` as the weighted average across all criteria.
- `confidence` reflects how certain you are in your scores (0.0–1.0). Lower confidence if the transcript is ambiguous or incomplete.
- `issues`: specific problems observed (e.g., "Retried `{tool_name} create` 3 times with same args").
- `highlights`: specific good practices observed (e.g., "Used `{tool_name} search` to verify data before proceeding").
- `rationale`: 2–4 sentence explanation of the overall assessment — why the scores are what they are, what the agent did well, and where it struggled.

Return JSON with this exact structure:
{{
  "scores": {{
    "criterion_id": <score_0_to_1>,
    ...
  }},
  "weighted_score": <weighted_average_0_to_1>,
  "confidence": <confidence_0_to_1>,
  "issues": ["issue1", "issue2", ...],
  "highlights": ["good_practice1", "good_practice2", ...],
  "rationale": "<2-4 sentence explanation of the overall assessment>"
}}

Provide JSON only, no additional text."#,
        tool_name = tool_name,
        task_description = task_description,
        transcript_path = transcript_path,
        criteria_text = criteria_text,
    )
}
