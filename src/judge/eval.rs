//! Evaluation execution for LLM-as-judge.
//!
//! This module provides functionality for running LLM-based evaluations
//! using OpenAI API or custom API endpoints.

#[cfg(test)]
use crate::judge::types::{JudgeResponse, Rubric};
#[cfg(test)]
use anyhow::{Context, Result};

/// Run LLM-as-judge evaluation via OpenAI API.
///
/// This is a test-only implementation for future use. The production
/// evaluation system uses CLI-based judge execution instead.
///
/// # Arguments
///
/// * `model` - OpenAI model to use (e.g., "gpt-4o-mini")
/// * `transcript_summary` - Summary of the LLM tool interaction
/// * `store_export` - JSON export of the tool state
/// * `task_description` - Description of the task that was performed
/// * `rubric` - Evaluation rubric with criteria
///
/// # Returns
///
/// * `Ok(JudgeResponse)` - Parsed judge evaluation
/// * `Err` - API error or response parsing error
///
/// # Environment Variables
///
/// Requires either `OPENAI_API_KEY` or `LLM_TOOL_TEST_API_KEY` to be set.
#[cfg(test)]
pub async fn run_judge(
    model: &str,
    transcript_summary: &str,
    store_export: &str,
    task_description: &str,
    rubric: &Rubric,
) -> Result<JudgeResponse> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .or_else(|_| std::env::var("LLM_TOOL_TEST_API_KEY"))
        .context("OPENAI_API_KEY or LLM_TOOL_TEST_API_KEY environment variable must be set")?;

    let client = reqwest::Client::new();

    let prompt = build_judge_prompt(transcript_summary, store_export, task_description, rubric);

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You are an expert evaluator. Analyze the provided transcript and store state against the given rubric. Return your evaluation as JSON only."
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "response_format": { "type": "json_object" },
        "temperature": 0.3,
        "max_tokens": 2000,
    });

    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await
        .context("Failed to call OpenAI API")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!("OpenAI API request failed: {} - {}", status, error_text);
    }

    let response_json: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse OpenAI API response")?;

    let content = response_json
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .context("Invalid OpenAI API response format")?;

    let judge_response: JudgeResponse = serde_json::from_str(content)
        .with_context(|| format!("Failed to parse judge response JSON: {}", content))?;

    Ok(judge_response)
}

#[cfg(test)]
pub fn build_judge_prompt(
    transcript_summary: &str,
    store_export: &str,
    task_description: &str,
    rubric: &Rubric,
) -> String {
    let criteria_text = rubric
        .criteria
        .iter()
        .map(|c| format!("- {}: {} (weight: {:.2})", c.id, c.description, c.weight))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"Evaluate the following LLM tool interaction.

# Task
{}

# Transcript Summary
{}

# Store State (JSON)
{}

# Evaluation Criteria
{}

# Required Response Format
Return JSON with this exact structure:
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
        task_description, transcript_summary, store_export, criteria_text
    )
}

/// Run judge with a custom API client for testing.
///
/// Test helper that allows specifying a custom API base URL and key,
/// useful for mocking in tests.
#[cfg(test)]
pub async fn run_judge_with_client(
    model: &str,
    transcript_summary: &str,
    store_export: &str,
    task_description: &str,
    rubric: &Rubric,
    api_base: &str,
    api_key: &str,
) -> Result<JudgeResponse> {
    use reqwest::Client;

    let client = Client::new();
    let prompt = build_judge_prompt(transcript_summary, store_export, task_description, rubric);

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You are an expert evaluator. Analyze the provided transcript and store state against the given rubric. Return your evaluation as JSON only."
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "response_format": { "type": "json_object" },
        "temperature": 0.3,
        "max_tokens": 2000,
    });

    let response = client
        .post(format!("{}/v1/chat/completions", api_base))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await
        .context("Failed to call OpenAI API")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!("OpenAI API request failed: {} - {}", status, error_text);
    }

    let response_json: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse OpenAI API response")?;

    let content = response_json
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .context("Invalid OpenAI API response format")?;

    let judge_response: JudgeResponse = serde_json::from_str(content)
        .with_context(|| format!("Failed to parse judge response JSON: {}", content))?;

    Ok(judge_response)
}
