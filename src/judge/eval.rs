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
/// Constructs a prompt containing the target summary, task description, the
/// agent guidance the agent was given, transcript file reference, rubric
/// criteria, a guidance-prescriptiveness rating request, optional MCP evidence
/// excerpt, and required judge result format. This prompt is passed to a
/// supported judge CLI tool.
///
/// `agent_guidance` is the concatenated content of any AGENTS.md / CLAUDE.md the
/// scenario provided to the agent under test; pass an empty string when the
/// scenario supplied none.
pub fn build_judge_prompt_for_target(
    target: &JudgeTargetView,
    task_description: &str,
    agent_guidance: &str,
    transcript_path: &str,
    rubric: &Rubric,
    mcp_tool_call_events: &[McpToolCallEvent],
) -> String {
    build_judge_prompt_inner(
        target,
        task_description,
        agent_guidance,
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
    build_judge_prompt_inner(&target, task_description, "", transcript_path, rubric, "")
}

/// Rendered when a scenario provided no agent guidance files.
const NO_AGENT_GUIDANCE: &str = "None provided — the agent received only the task prompt above.";

fn build_judge_prompt_inner(
    target: &JudgeTargetView,
    task_description: &str,
    agent_guidance: &str,
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

    let guidance_block = if agent_guidance.trim().is_empty() {
        NO_AGENT_GUIDANCE
    } else {
        agent_guidance.trim()
    };

    format!(
        r#"You are evaluating how effectively an LLM agent used {target_summary}.

Read the transcript at @{transcript_path}, then score the interaction against the criteria below.{evidence_excerpt}

## Task the agent was given
{task_description}

## Agent guidance the agent was given
{guidance_block}

## Evaluation Criteria
{criteria_text}

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
- `issues`: specific problems observed (e.g., "Retried `{target_name} create` 3 times with same args").
- `highlights`: specific good practices observed (e.g., "Used `{target_name} search` to verify data before proceeding").
- `rationale`: 2–4 sentence explanation of the overall assessment — why the scores are what they are, what the agent did well, and where it struggled.

## Guidance prescriptiveness
Separately from the criteria above, rate how prescriptive the scenario's guidance was — the task prompt plus any agent guidance — taken as a whole. This rates the inputs the agent was handed, not its performance, so do NOT fold it into `weighted_score`; achieving a goal that was spelled out step by step is less impressive than deciding the approach unaided. Use this scale:
- 0 (goal-only): states an outcome or goal and names no tools or steps; the agent must decide everything.
- 1 (light hints): mentions relevant tools or capabilities but not how or when to use them.
- 2 (partial recipe): spells out specific commands or steps for part of the task; the agent fills the gaps.
- 3 (step-by-step): the prompt and/or guidance dictate the exact sequence of tool calls, so success is mostly obedience.
Report the integer `level` and a one-sentence `rationale` under `prescriptiveness`.

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
  "rationale": "<2-4 sentence explanation of the overall assessment>",
  "prescriptiveness": {{ "level": <0_to_3>, "rationale": "<one sentence>" }}
}}

Wrap only that JSON object in a single <judge_result> tag:
<judge_result>
{{ ...the JSON object above... }}
</judge_result>

Do not put prose, markdown, or code fences inside <judge_result>. If you need to say anything else, put it outside the tag."#,
        target_summary = target.summary,
        target_name = target.name,
        task_description = task_description,
        guidance_block = guidance_block,
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
