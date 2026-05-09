use crate::transcript::{CommandEvent, EfficiencyMetrics, InteractionMetricsSource};
use anyhow::{Context, Result};
use std::path::Path;

/// Checks if the transcript has no errors.
pub fn no_transcript_errors(
    env_root: &Path,
    target_binary: &str,
    command_pattern: Option<&str>,
    metrics_source: InteractionMetricsSource,
    command_events: &[CommandEvent],
) -> Result<bool> {
    let metrics = compute_efficiency_metrics(
        env_root,
        target_binary,
        command_pattern,
        metrics_source,
        command_events,
    )?;
    Ok(metrics.error_count == 0)
}

/// Computes efficiency metrics from the transcript.
pub fn compute_efficiency_metrics(
    env_root: &Path,
    target_binary: &str,
    command_pattern: Option<&str>,
    metrics_source: InteractionMetricsSource,
    command_events: &[CommandEvent],
) -> Result<EfficiencyMetrics> {
    match metrics_source {
        InteractionMetricsSource::StructuredToolCalls => Ok(
            crate::transcript::TranscriptAnalyzer::analyze_command_events_for_target(
                command_events,
                target_binary,
            ),
        ),
        InteractionMetricsSource::TranscriptRegex => {
            let transcript_path = env_root.join("transcript.raw.txt");
            let content = std::fs::read_to_string(&transcript_path)
                .context("Failed to read transcript file for regex efficiency metrics")?;
            Ok(
                crate::transcript::TranscriptAnalyzer::analyze_with_exit_codes_for_target(
                    &content,
                    target_binary,
                    command_pattern,
                ),
            )
        }
    }
}

/// Computes a composite score from judge score, gates, and efficiency metrics.
pub fn compute_composite_score(
    judge_score: Option<f64>,
    gates_passed: usize,
    gates_total: usize,
    efficiency: &crate::transcript::EfficiencyMetrics,
    weights: Option<&crate::scenario::CompositeConfig>,
) -> f64 {
    let (judge_weight, gates_weight, efficiency_weight) = match weights {
        Some(w) => (w.judge_weight, w.gate_weight, w.interaction_weight),
        None => (0.55, 0.35, 0.10), // Default weights
    };

    let judge_component = judge_score.unwrap_or(0.0);

    let gates_component = if gates_total > 0 {
        gates_passed as f64 / gates_total as f64
    } else {
        0.0
    };

    let efficiency_component = efficiency.first_try_success_rate;

    let composite = (judge_weight * judge_component)
        + (gates_weight * gates_component)
        + (efficiency_weight * efficiency_component);

    composite.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_tool_calls_do_not_fall_back_to_transcript_regex() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("transcript.raw.txt"),
            "notes add\nexit code: 1\n",
        )
        .expect("write transcript");
        let events = vec![CommandEvent {
            command: "notes add".to_string(),
            exit_code: Some(0),
        }];

        let metrics = compute_efficiency_metrics(
            temp.path(),
            "notes",
            None,
            InteractionMetricsSource::StructuredToolCalls,
            &events,
        )
        .expect("compute metrics");

        assert_eq!(metrics.total_commands, 1);
        assert_eq!(metrics.error_count, 0);
    }

    #[test]
    fn transcript_regex_source_ignores_structured_events() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("transcript.raw.txt"),
            "notes add\nexit code: 1\n",
        )
        .expect("write transcript");
        let events = vec![CommandEvent {
            command: "notes add".to_string(),
            exit_code: Some(0),
        }];

        let metrics = compute_efficiency_metrics(
            temp.path(),
            "notes",
            None,
            InteractionMetricsSource::TranscriptRegex,
            &events,
        )
        .expect("compute metrics");

        assert_eq!(metrics.total_commands, 1);
        assert_eq!(metrics.error_count, 1);
    }
}
