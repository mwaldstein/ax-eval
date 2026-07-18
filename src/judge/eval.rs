//! Judge prompt construction for LLM-as-judge evaluation.
//!
//! The judge is executed via the same CLI tool adapter framework used to run
//! scenarios — not via direct API calls. This module builds the prompt that
//! gets passed to the CLI tool.

use crate::interaction_evidence::McpToolCallEvent;
use crate::judge::types::Rubric;
use crate::scenario::TargetConfig;

// Keep the MCP evidence excerpt bounded so judge prompts stay predictable even
// for exploratory runs with many server calls.
const MCP_TOOL_CALL_EXCERPT_LIMIT: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JudgeTargetKind {
    Cli,
    Mcp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgeTargetView {
    pub kind: JudgeTargetKind,
    pub name: String,
    pub summary: String,
}

impl JudgeTargetView {
    pub fn from_target(target: &TargetConfig) -> Self {
        match target {
            TargetConfig::Cli(target) => Self {
                kind: JudgeTargetKind::Cli,
                name: target.binary.clone(),
                summary: format!("the CLI tool `{}`", target.binary),
            },
            TargetConfig::Mcp(target) => Self {
                kind: JudgeTargetKind::Mcp,
                name: target.name.clone(),
                summary: format!(
                    "the MCP server `{}` exposing tools [{}]",
                    target.name,
                    target.tools.join(", ")
                ),
            },
        }
    }

    #[cfg(test)]
    pub fn cli(tool_name: &str) -> Self {
        Self {
            kind: JudgeTargetKind::Cli,
            name: tool_name.to_string(),
            summary: format!("the CLI tool `{tool_name}`"),
        }
    }
}

/// Build the judge prompt for target-aware evaluation.
///
/// Constructs a prompt containing the target summary, task description,
/// transcript file reference, rubric criteria, optional MCP evidence excerpt,
/// and required judge result format. This prompt is passed to a supported
/// judge CLI tool.
pub fn build_judge_prompt_for_target(
    target: &JudgeTargetView,
    task_description: &str,
    transcript_path: &str,
    rubric: &Rubric,
    mcp_tool_call_events: &[McpToolCallEvent],
) -> String {
    build_judge_prompt_inner(
        target,
        task_description,
        transcript_path,
        rubric,
        &mcp_evidence_excerpt(target, mcp_tool_call_events),
    )
}

/// Build the judge prompt for CLI-based evaluation.
///
/// Constructs a prompt containing the tool name, task description, transcript
/// file reference, rubric criteria, and required judge result format.
/// This prompt is passed to a supported judge CLI tool.
#[cfg(test)]
pub fn build_judge_prompt(
    tool_name: &str,
    task_description: &str,
    transcript_path: &str,
    rubric: &Rubric,
) -> String {
    let target = JudgeTargetView::cli(tool_name);
    build_judge_prompt_inner(&target, task_description, transcript_path, rubric, "")
}

fn build_judge_prompt_inner(
    target: &JudgeTargetView,
    task_description: &str,
    transcript_path: &str,
    rubric: &Rubric,
    evidence_excerpt: &str,
) -> String {
    let criteria_text = rubric
        .criteria
        .iter()
        .map(|c| format!("- {}: {} (weight: {:.2})", c.id, c.description, c.weight))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are evaluating how effectively an LLM agent used {target_summary}.

Read the transcript at @{transcript_path}, then score the interaction against the criteria below.{evidence_excerpt}

## Task the agent was given
{task_description}

## Evaluation Criteria
{criteria_text}

## Scoring Guidelines
- Score each criterion 0.0–1.0. 0.0 = complete failure, 0.5 = partial, 1.0 = excellent.
- Compute `weighted_score` as the weighted average across all criteria.
- `confidence` reflects how certain you are in your scores (0.0–1.0). Lower confidence if the transcript is ambiguous or incomplete.
- `issues`: specific problems observed (e.g., "Retried `{target_name} create` 3 times with same args").
- `highlights`: specific good practices observed (e.g., "Used `{target_name} search` to verify data before proceeding").
- `rationale`: 2–4 sentence explanation of the overall assessment — why the scores are what they are, what the agent did well, and where it struggled.

Return one valid JSON object with this exact structure:
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

Wrap only that JSON object in a single <judge_result> tag:
<judge_result>
{{ ...the JSON object above... }}
</judge_result>

Do not put prose, markdown, or code fences inside <judge_result>. If you need to say anything else, put it outside the tag."#,
        target_summary = target.summary,
        target_name = target.name,
        task_description = task_description,
        transcript_path = transcript_path,
        criteria_text = criteria_text,
        evidence_excerpt = evidence_excerpt,
    )
}

fn mcp_evidence_excerpt(target: &JudgeTargetView, events: &[McpToolCallEvent]) -> String {
    if target.kind != JudgeTargetKind::Mcp {
        return String::new();
    }

    let mut excerpt = format!(
        "\n\n## Structured MCP tool-call excerpt\nFirst {} of {} captured MCP tool calls are shown (bound: {}):",
        events.len().min(MCP_TOOL_CALL_EXCERPT_LIMIT),
        events.len(),
        MCP_TOOL_CALL_EXCERPT_LIMIT
    );

    for event in events.iter().take(MCP_TOOL_CALL_EXCERPT_LIMIT) {
        let arguments = serde_json::to_string(&event.arguments)
            .unwrap_or_else(|_| "<unserializable arguments>".to_string());
        excerpt.push_str(&format!(
            "\n- {} arguments={} error={}",
            event.tool, arguments, event.is_error
        ));
    }

    excerpt.push('\n');
    excerpt
}
