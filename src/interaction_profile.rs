mod evidence;

use self::evidence::{extract_target_interaction_evidence, InteractionEvidenceInput};
use crate::transcript::{CommandEvent, EfficiencyMetrics, InteractionInput};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterEvidenceCapability {
    StructuredToolCalls,
    TranscriptRegexFallback,
}

impl AdapterEvidenceCapability {
    pub fn from_supports_structured_tool_calls(supports_structured_tool_calls: bool) -> Self {
        if supports_structured_tool_calls {
            Self::StructuredToolCalls
        } else {
            Self::TranscriptRegexFallback
        }
    }
}

#[derive(Debug, Clone)]
pub struct TargetInteractionSpec {
    binary: String,
    command_pattern: Option<String>,
}

impl TargetInteractionSpec {
    pub fn new(binary: impl Into<String>, command_pattern: Option<String>) -> Self {
        Self {
            binary: binary.into(),
            command_pattern,
        }
    }

    pub fn binary(&self) -> &str {
        &self.binary
    }

    pub fn command_pattern(&self) -> Option<&str> {
        self.command_pattern.as_deref()
    }
}

pub struct InteractionProfileInput<'a> {
    pub target: &'a TargetInteractionSpec,
    pub interaction_input: &'a InteractionInput,
    pub adapter_capability: AdapterEvidenceCapability,
    pub transcript_path: PathBuf,
    pub completed: bool,
}

pub(crate) fn reduce_command_events(events: &[CommandEvent]) -> EfficiencyMetrics {
    let commands = events
        .iter()
        .map(|event| {
            let is_error = event.exit_code.map(|code| code != 0).unwrap_or(false);
            (event.command.clone(), is_error)
        })
        .collect::<Vec<_>>();

    let total_commands = commands.len();
    let error_count = commands.iter().filter(|(_, e)| *e).count();
    let help_invocations = commands.iter().filter(|(c, _)| c == "help").count();

    let unique_commands = commands
        .iter()
        .map(|(c, _)| c.clone())
        .collect::<std::collections::HashSet<_>>();
    let retry_count = total_commands.saturating_sub(unique_commands.len());

    let mut seen_first: HashMap<String, bool> = HashMap::new();
    let mut first_try_success_count: usize = 0;

    for (cmd, is_error) in &commands {
        if !seen_first.contains_key(cmd) {
            seen_first.insert(cmd.clone(), !is_error);
            if !is_error {
                first_try_success_count += 1;
            }
        }
    }

    let first_try_success_rate = if total_commands > 0 {
        first_try_success_count as f64 / total_commands as f64
    } else {
        0.0
    };

    let iteration_ratio = if total_commands > 0 {
        unique_commands.len() as f64 / total_commands as f64
    } else {
        0.0
    };

    EfficiencyMetrics {
        total_commands,
        unique_commands: unique_commands.len(),
        error_count,
        retry_count,
        help_invocations,
        first_try_success_rate,
        iteration_ratio,
        completed: false,
    }
}

pub fn build_interaction_profile(input: InteractionProfileInput<'_>) -> Result<InteractionProfile> {
    let structured_evidence = matches!(
        input.interaction_input,
        InteractionInput::StructuredToolCalls(_)
    );
    let extracted = extract_target_interaction_evidence(InteractionEvidenceInput {
        target: input.target,
        interaction_input: input.interaction_input,
        adapter_capability: input.adapter_capability,
        transcript_path: input.transcript_path,
    })?;
    let mut metrics = reduce_command_events(&extracted.target_events);

    metrics.completed = input.completed;

    if input.completed && structured_evidence && metrics.total_commands == 0 {
        anyhow::bail!(
            "Adapter supports structured tool calls but returned no usable structured target-tool events"
        );
    }

    Ok(InteractionProfile {
        metrics,
        evidence_source: extracted.source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::CommandEvent;

    fn target() -> TargetInteractionSpec {
        TargetInteractionSpec::new("notes", None)
    }

    fn unused_transcript_path() -> PathBuf {
        PathBuf::from("unused-transcript.raw.txt")
    }

    #[test]
    fn structured_evidence_builds_profile() {
        let events = vec![CommandEvent {
            command: "notes add hello".to_string(),
            exit_code: Some(0),
        }];
        let target = target();

        let profile = build_interaction_profile(InteractionProfileInput {
            target: &target,
            interaction_input: &InteractionInput::StructuredToolCalls(events),
            adapter_capability: AdapterEvidenceCapability::StructuredToolCalls,
            transcript_path: unused_transcript_path(),
            completed: true,
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
    fn structured_evidence_reduces_target_commands_without_transcript_regex() {
        let events = vec![
            CommandEvent {
                command: "ls".to_string(),
                exit_code: Some(0),
            },
            CommandEvent {
                command: "./notes init".to_string(),
                exit_code: Some(0),
            },
            CommandEvent {
                command: "bash -lc './notes add \"Hello\"'".to_string(),
                exit_code: Some(1),
            },
            CommandEvent {
                command: "/tmp/work/notes add \"Hello\"".to_string(),
                exit_code: Some(0),
            },
            CommandEvent {
                command: "./notes list --help".to_string(),
                exit_code: Some(0),
            },
        ];
        let target = target();

        let profile = build_interaction_profile(InteractionProfileInput {
            target: &target,
            interaction_input: &InteractionInput::StructuredToolCalls(events),
            adapter_capability: AdapterEvidenceCapability::StructuredToolCalls,
            transcript_path: unused_transcript_path(),
            completed: true,
        })
        .expect("profile");

        assert_eq!(profile.metrics.total_commands, 4);
        assert_eq!(profile.metrics.unique_commands, 3);
        assert_eq!(profile.metrics.error_count, 1);
        assert_eq!(profile.metrics.retry_count, 1);
        assert_eq!(profile.metrics.help_invocations, 1);
        assert_eq!(profile.metrics.first_try_success_rate, 0.5);
    }

    #[test]
    fn structured_capable_adapter_cannot_use_regex_fallback() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("transcript.raw.txt"), "notes add\n").expect("write");

        let target = target();
        let error = build_interaction_profile(InteractionProfileInput {
            target: &target,
            interaction_input: &InteractionInput::TranscriptRegex,
            adapter_capability: AdapterEvidenceCapability::StructuredToolCalls,
            transcript_path: temp.path().join("transcript.raw.txt"),
            completed: true,
        })
        .unwrap_err();

        assert!(error.to_string().contains("returned transcript regex"));
    }

    #[test]
    fn completed_structured_run_requires_target_tool_events() {
        let events = vec![CommandEvent {
            command: "ls -la".to_string(),
            exit_code: Some(0),
        }];
        let target = target();

        let error = build_interaction_profile(InteractionProfileInput {
            target: &target,
            interaction_input: &InteractionInput::StructuredToolCalls(events),
            adapter_capability: AdapterEvidenceCapability::StructuredToolCalls,
            transcript_path: unused_transcript_path(),
            completed: true,
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
        let target = target();

        let profile = build_interaction_profile(InteractionProfileInput {
            target: &target,
            interaction_input: &InteractionInput::TranscriptRegex,
            adapter_capability: AdapterEvidenceCapability::TranscriptRegexFallback,
            transcript_path: temp.path().join("transcript.raw.txt"),
            completed: true,
        })
        .expect("profile");

        assert_eq!(
            profile.evidence_source,
            InteractionEvidenceSource::TranscriptRegexFallback
        );
        assert_eq!(profile.metrics.total_commands, 1);
    }

    #[test]
    fn structured_and_regex_fallback_use_same_target_command_extraction() {
        let target = target();
        let structured_events = vec![
            CommandEvent {
                command: "./notes init".to_string(),
                exit_code: Some(0),
            },
            CommandEvent {
                command: "bash -lc './notes add \"Hello\"'".to_string(),
                exit_code: Some(1),
            },
            CommandEvent {
                command: "/tmp/work/notes list --help".to_string(),
                exit_code: Some(0),
            },
        ];

        let structured = build_interaction_profile(InteractionProfileInput {
            target: &target,
            interaction_input: &InteractionInput::StructuredToolCalls(structured_events),
            adapter_capability: AdapterEvidenceCapability::StructuredToolCalls,
            transcript_path: unused_transcript_path(),
            completed: true,
        })
        .expect("structured profile");

        let temp = tempfile::tempdir().expect("tempdir");
        let transcript_path = temp.path().join("transcript.raw.txt");
        std::fs::write(
            &transcript_path,
            "$ ./notes init\nexit code: 0\n\
             $ bash -lc './notes add \"Hello\"'\nexit code: 1\n\
             $ /tmp/work/notes list --help\nexit code: 0\n",
        )
        .expect("write transcript");

        let fallback = build_interaction_profile(InteractionProfileInput {
            target: &target,
            interaction_input: &InteractionInput::TranscriptRegex,
            adapter_capability: AdapterEvidenceCapability::TranscriptRegexFallback,
            transcript_path,
            completed: true,
        })
        .expect("fallback profile");

        assert_eq!(
            structured.metrics.total_commands,
            fallback.metrics.total_commands
        );
        assert_eq!(
            structured.metrics.unique_commands,
            fallback.metrics.unique_commands
        );
        assert_eq!(structured.metrics.error_count, fallback.metrics.error_count);
        assert_eq!(
            structured.metrics.help_invocations,
            fallback.metrics.help_invocations
        );
    }
}
