use crate::transcript::{EfficiencyMetrics, InteractionInput, TranscriptAnalyzer};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionEvidenceSource {
    StructuredToolCalls,
    TranscriptRegexFallback,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InteractionProfile {
    pub metrics: EfficiencyMetrics,
    pub evidence_source: InteractionEvidenceSource,
}

pub struct InteractionProfileInput<'a> {
    pub env_root: &'a Path,
    pub target_binary: &'a str,
    pub command_pattern: Option<&'a str>,
    pub interaction_input: &'a InteractionInput,
    pub completed: bool,
    pub supports_structured_tool_calls: bool,
}

pub fn build_interaction_profile(input: InteractionProfileInput<'_>) -> Result<InteractionProfile> {
    let (mut metrics, evidence_source) = match input.interaction_input {
        InteractionInput::StructuredToolCalls(command_events) => {
            let metrics = TranscriptAnalyzer::analyze_command_events_for_target(
                command_events,
                input.target_binary,
            );
            (metrics, InteractionEvidenceSource::StructuredToolCalls)
        }
        InteractionInput::TranscriptRegex => {
            if input.supports_structured_tool_calls {
                anyhow::bail!(
                    "Adapter supports structured tool calls but returned transcript regex evidence"
                );
            }

            let transcript_path = input.env_root.join("transcript.raw.txt");
            let content = std::fs::read_to_string(&transcript_path)
                .with_context(|| "Failed to read transcript file for regex interaction profile")?;
            let metrics = TranscriptAnalyzer::analyze_with_exit_codes_for_target(
                &content,
                input.target_binary,
                input.command_pattern,
            );
            (metrics, InteractionEvidenceSource::TranscriptRegexFallback)
        }
    };

    metrics.completed = input.completed;

    if input.supports_structured_tool_calls
        && input.completed
        && matches!(
            input.interaction_input,
            InteractionInput::StructuredToolCalls(_)
        )
        && metrics.total_commands == 0
    {
        anyhow::bail!(
            "Adapter supports structured tool calls but returned no usable structured target-tool events"
        );
    }

    Ok(InteractionProfile {
        metrics,
        evidence_source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::CommandEvent;

    #[test]
    fn structured_evidence_builds_profile() {
        let temp = tempfile::tempdir().expect("tempdir");
        let events = vec![CommandEvent {
            command: "notes add hello".to_string(),
            exit_code: Some(0),
        }];

        let profile = build_interaction_profile(InteractionProfileInput {
            env_root: temp.path(),
            target_binary: "notes",
            command_pattern: None,
            interaction_input: &InteractionInput::StructuredToolCalls(events),
            completed: true,
            supports_structured_tool_calls: true,
        })
        .expect("profile");

        assert_eq!(
            profile.evidence_source,
            InteractionEvidenceSource::StructuredToolCalls
        );
        assert_eq!(profile.metrics.total_commands, 1);
        assert!(profile.metrics.completed);
    }

    #[test]
    fn structured_capable_adapter_cannot_use_regex_fallback() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("transcript.raw.txt"), "notes add\n").expect("write");

        let error = build_interaction_profile(InteractionProfileInput {
            env_root: temp.path(),
            target_binary: "notes",
            command_pattern: None,
            interaction_input: &InteractionInput::TranscriptRegex,
            completed: true,
            supports_structured_tool_calls: true,
        })
        .unwrap_err();

        assert!(error.to_string().contains("returned transcript regex"));
    }

    #[test]
    fn completed_structured_run_requires_target_tool_events() {
        let temp = tempfile::tempdir().expect("tempdir");
        let events = vec![CommandEvent {
            command: "ls -la".to_string(),
            exit_code: Some(0),
        }];

        let error = build_interaction_profile(InteractionProfileInput {
            env_root: temp.path(),
            target_binary: "notes",
            command_pattern: None,
            interaction_input: &InteractionInput::StructuredToolCalls(events),
            completed: true,
            supports_structured_tool_calls: true,
        })
        .unwrap_err();

        assert!(error.to_string().contains("no usable structured"));
    }

    #[test]
    fn regex_fallback_is_available_for_non_structured_adapters() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("transcript.raw.txt"),
            "notes add hello\nexit code: 0\n",
        )
        .expect("write");

        let profile = build_interaction_profile(InteractionProfileInput {
            env_root: temp.path(),
            target_binary: "notes",
            command_pattern: None,
            interaction_input: &InteractionInput::TranscriptRegex,
            completed: true,
            supports_structured_tool_calls: false,
        })
        .expect("profile");

        assert_eq!(
            profile.evidence_source,
            InteractionEvidenceSource::TranscriptRegexFallback
        );
        assert_eq!(profile.metrics.total_commands, 1);
    }
}
