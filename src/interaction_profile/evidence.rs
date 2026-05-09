use crate::interaction_profile::{
    AdapterEvidenceCapability, InteractionEvidenceSource, TargetInteractionSpec,
};
use crate::transcript::{CommandEvent, InteractionInput};
use anyhow::{Context, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct ExtractedInteractionEvidence {
    pub target_events: Vec<CommandEvent>,
    pub source: InteractionEvidenceSource,
}

pub(crate) struct InteractionEvidenceInput<'a> {
    pub target: &'a TargetInteractionSpec,
    pub interaction_input: &'a InteractionInput,
    pub adapter_capability: AdapterEvidenceCapability,
    pub transcript_path: PathBuf,
}

enum RawInteractionEvidence {
    StructuredToolCalls(Vec<CommandEvent>),
    TranscriptRegexFallback { transcript_path: PathBuf },
}

impl RawInteractionEvidence {
    fn from_interaction_input(
        interaction_input: &InteractionInput,
        adapter_capability: AdapterEvidenceCapability,
        transcript_path: impl Into<PathBuf>,
    ) -> Result<Self> {
        match (interaction_input, adapter_capability) {
            (
                InteractionInput::StructuredToolCalls(command_events),
                AdapterEvidenceCapability::StructuredToolCalls,
            ) => Ok(Self::StructuredToolCalls(command_events.clone())),
            (
                InteractionInput::StructuredToolCalls(_),
                AdapterEvidenceCapability::TranscriptRegexFallback,
            ) => {
                anyhow::bail!(
                    "Adapter does not declare structured tool-call support but returned structured tool calls"
                )
            }
            (
                InteractionInput::TranscriptRegex,
                AdapterEvidenceCapability::TranscriptRegexFallback,
            ) => Ok(Self::TranscriptRegexFallback {
                transcript_path: transcript_path.into(),
            }),
            (InteractionInput::TranscriptRegex, AdapterEvidenceCapability::StructuredToolCalls) => {
                anyhow::bail!(
                    "Adapter supports structured tool calls but returned transcript regex evidence"
                )
            }
        }
    }
}

pub(crate) fn extract_target_interaction_evidence(
    input: InteractionEvidenceInput<'_>,
) -> Result<ExtractedInteractionEvidence> {
    let evidence = RawInteractionEvidence::from_interaction_input(
        input.interaction_input,
        input.adapter_capability,
        input.transcript_path,
    )?;

    match evidence {
        RawInteractionEvidence::StructuredToolCalls(command_events) => {
            Ok(ExtractedInteractionEvidence {
                target_events: super::target::structured_target_events(
                    &command_events,
                    input.target,
                ),
                source: InteractionEvidenceSource::StructuredToolCalls,
            })
        }
        RawInteractionEvidence::TranscriptRegexFallback { transcript_path } => {
            let content = std::fs::read_to_string(&transcript_path)
                .with_context(|| "Failed to read transcript file for regex interaction profile")?;
            Ok(ExtractedInteractionEvidence {
                target_events: super::target::transcript_target_events(&content, input.target),
                source: InteractionEvidenceSource::TranscriptRegexFallback,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> TargetInteractionSpec {
        TargetInteractionSpec::new("notes", None)
    }

    #[test]
    fn structured_evidence_extracts_target_commands_without_transcript_regex() {
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
                command: "/tmp/work/notes list --help".to_string(),
                exit_code: Some(0),
            },
        ];
        let target = target();

        let extracted = extract_target_interaction_evidence(InteractionEvidenceInput {
            target: &target,
            interaction_input: &InteractionInput::StructuredToolCalls(events),
            adapter_capability: AdapterEvidenceCapability::StructuredToolCalls,
            transcript_path: PathBuf::from("unused-transcript.raw.txt"),
        })
        .expect("evidence");

        let commands = extracted
            .target_events
            .iter()
            .map(|event| (event.command.as_str(), event.exit_code))
            .collect::<Vec<_>>();

        assert_eq!(
            commands,
            vec![("init", Some(0)), ("add", Some(1)), ("help", Some(0))]
        );
    }
}
