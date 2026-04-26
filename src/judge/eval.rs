//! Judge prompt construction for CLI-based LLM-as-judge evaluation.
//!
//! The judge is executed via the same CLI tool adapter framework used to run
//! scenarios — not via direct API calls. This module builds the prompt that
//! gets passed to the CLI tool.

use crate::judge::types::Rubric;

/// Build the judge prompt for CLI-based evaluation.
///
/// Constructs a prompt containing the task description, transcript file
/// reference, rubric criteria, and required JSON response format.
/// This prompt is passed to the CLI tool (e.g., `opencode run <prompt>`).
pub fn build_judge_prompt(
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
        r#"Evaluate this LLM tool interaction.

Task: {}

Files to review:
- @{} - The interaction transcript

# Evaluation Criteria
{}

Return evaluation as JSON with this structure:
{{
  "scores": {{
    "criterion_id": <score_0_to_1>,
    ...
  }},
  "weighted_score": <weighted_average_0_to_1>,
  "confidence": <confidence_0_to_1>,
  "issues": ["issue1", "issue2", ...],
  "highlights": ["good_practice1", "good_practice2", ...]
}}

Provide JSON only, no additional text."#,
        task_description, transcript_path, criteria_text
    )
}
