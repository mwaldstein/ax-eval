use crate::interaction_evidence::{CommandEvent, InteractionInput, McpToolCallEvent};
use crate::interaction_profile::{
    AdapterEvidenceCapability, InteractionEvidenceSource, TargetInteractionSpec,
};
use anyhow::{Context, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct ExtractedInteractionEvidence {
    pub target_actions: super::target::TargetActionProjection,
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
    StructuredMcpToolCalls(Vec<McpToolCallEvent>),
    StructuredMixedToolCalls {
        commands: Vec<CommandEvent>,
        mcp: Vec<McpToolCallEvent>,
    },
    TranscriptRegexFallback {
        transcript_path: PathBuf,
    },
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
                InteractionInput::StructuredMcpToolCalls(mcp_events),
                AdapterEvidenceCapability::StructuredToolCalls,
            ) => Ok(Self::StructuredMcpToolCalls(mcp_events.clone())),
            (
                InteractionInput::StructuredMixedToolCalls { commands, mcp },
                AdapterEvidenceCapability::StructuredToolCalls,
            ) => Ok(Self::StructuredMixedToolCalls {
                commands: commands.clone(),
                mcp: mcp.clone(),
            }),
            (
                InteractionInput::StructuredToolCalls(_),
                AdapterEvidenceCapability::TranscriptRegexFallback,
            ) => {
                anyhow::bail!(
                    "Adapter does not declare structured tool-call support but returned structured tool calls"
                )
            }
            (
                InteractionInput::StructuredMcpToolCalls(_),
                AdapterEvidenceCapability::TranscriptRegexFallback,
            ) => {
                anyhow::bail!(
                    "Adapter does not declare structured tool-call support but returned structured MCP tool calls"
                )
            }
            (
                InteractionInput::StructuredMixedToolCalls { .. },
                AdapterEvidenceCapability::TranscriptRegexFallback,
            ) => {
                anyhow::bail!(
                    "Adapter does not declare structured tool-call support but returned mixed structured tool calls"
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
                target_actions: super::target::structured_target_actions(
                    &command_events,
                    input.target,
                ),
                source: InteractionEvidenceSource::StructuredToolCalls,
            })
        }
        RawInteractionEvidence::StructuredMcpToolCalls(mcp_events) => {
            Ok(ExtractedInteractionEvidence {
                target_actions: super::target::structured_mcp_target_actions(
                    &mcp_events,
                    input.target,
                ),
                source: InteractionEvidenceSource::StructuredMcpToolCalls,
            })
        }
        RawInteractionEvidence::StructuredMixedToolCalls { commands, mcp } => match input.target {
            crate::interaction_evidence::TargetSpec::Cli { .. } => {
                Ok(ExtractedInteractionEvidence {
                    target_actions: super::target::structured_target_actions(
                        &commands,
                        input.target,
                    ),
                    source: InteractionEvidenceSource::StructuredToolCalls,
                })
            }
            crate::interaction_evidence::TargetSpec::Mcp { .. } => {
                Ok(ExtractedInteractionEvidence {
                    target_actions: super::target::structured_mcp_target_actions(
                        &mcp,
                        input.target,
                    ),
                    source: InteractionEvidenceSource::StructuredMcpToolCalls,
                })
            }
        },
        RawInteractionEvidence::TranscriptRegexFallback { transcript_path } => {
            let content = std::fs::read_to_string(&transcript_path)
                .with_context(|| "Failed to read transcript file for regex interaction profile")?;
            Ok(ExtractedInteractionEvidence {
                target_actions: super::target::transcript_target_actions(&content, input.target),
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
            .target_actions
            .actions
            .iter()
            .map(|event| (event.action.as_str(), event.outcome))
            .collect::<Vec<_>>();

        assert_eq!(
            commands,
            vec![
                ("init", super::super::target::Outcome::Success),
                ("add", super::super::target::Outcome::Failure),
                ("help", super::super::target::Outcome::Success)
            ]
        );
    }
}
