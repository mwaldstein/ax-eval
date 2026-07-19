mod evidence;
mod target;

use self::evidence::{extract_target_interaction_evidence, InteractionEvidenceInput};
use self::target::{Outcome, TargetAction};
use crate::interaction_evidence::InteractionInput;
pub use crate::interaction_evidence::{
    AdapterEvidenceCapability, InteractionEvidenceSource, TargetInteractionSpec,
};
use crate::scenario::TargetCommandPolicy;
use crate::transcript::EfficiencyMetrics;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct InteractionProfile {
    pub metrics: EfficiencyMetrics,
    pub evidence_source: InteractionEvidenceSource,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

pub struct InteractionProfileInput<'a> {
    pub target: &'a TargetInteractionSpec,
    pub interaction_input: &'a InteractionInput,
    pub adapter_capability: AdapterEvidenceCapability,
    pub transcript_path: PathBuf,
    pub completed: bool,
    pub target_command_policy: TargetCommandPolicy,
}

pub(crate) fn reduce_target_actions(actions: &[TargetAction]) -> EfficiencyMetrics {
    let total_commands = actions.len();
    let error_count = actions
        .iter()
        .filter(|action| action.outcome == Outcome::Failure)
        .count();
    let help_invocations = actions
        .iter()
        .filter(|action| action.action == "help")
        .count();

    let unique_commands = actions
        .iter()
        .map(|action| action.action.clone())
        .collect::<std::collections::HashSet<_>>();
    let tool_reuse_count = total_commands.saturating_sub(unique_commands.len());

    let mut seen_first: HashMap<String, bool> = HashMap::new();
    let mut first_try_success_count: usize = 0;

    for action in actions {
        if !seen_first.contains_key(&action.action) {
            let first_try_success = action.outcome != Outcome::Failure;
            seen_first.insert(action.action.clone(), first_try_success);
            if first_try_success {
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
        tool_reuse_count,
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
            | InteractionInput::StructuredMcpToolCalls(_)
            | InteractionInput::StructuredMixedToolCalls { .. }
    );
    let extracted = extract_target_interaction_evidence(InteractionEvidenceInput {
        target: input.target,
        interaction_input: input.interaction_input,
        adapter_capability: input.adapter_capability,
        transcript_path: input.transcript_path,
    })?;
    let mut metrics = reduce_target_actions(&extracted.target_actions.actions);

    metrics.completed = input.completed;

    if input.completed
        && structured_evidence
        && metrics.total_commands == 0
        && input.target_command_policy == TargetCommandPolicy::Required
    {
        anyhow::bail!(
            "Adapter supports structured tool calls but returned no usable structured target-tool events"
        );
    }

    if input.completed
        && metrics.total_commands > 0
        && input.target_command_policy == TargetCommandPolicy::Forbidden
    {
        anyhow::bail!("target-tool events are forbidden for this scenario");
    }

    Ok(InteractionProfile {
        metrics,
        evidence_source: extracted.source,
        warnings: extracted.target_actions.warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction_evidence::{CommandEvent, McpToolCallEvent};
    use crate::scenario::types::TargetCommandPolicy;

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
            target_command_policy: TargetCommandPolicy::Required,
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
            target_command_policy: TargetCommandPolicy::Required,
        })
        .expect("profile");

        assert_eq!(profile.metrics.total_commands, 4);
        assert_eq!(profile.metrics.unique_commands, 3);
        assert_eq!(profile.metrics.error_count, 1);
        assert_eq!(profile.metrics.tool_reuse_count, 1);
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
            target_command_policy: TargetCommandPolicy::Required,
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
            target_command_policy: TargetCommandPolicy::Required,
        })
        .unwrap_err();

        assert!(error.to_string().contains("no usable structured"));
    }

    #[test]
    fn optional_target_commands_allow_completed_structured_run_without_target_tool_events() {
        let events = vec![CommandEvent {
            command: "ls -la".to_string(),
            exit_code: Some(0),
        }];
        let target = target();

        let profile = build_interaction_profile(InteractionProfileInput {
            target: &target,
            interaction_input: &InteractionInput::StructuredToolCalls(events),
            adapter_capability: AdapterEvidenceCapability::StructuredToolCalls,
            transcript_path: unused_transcript_path(),
            completed: true,
            target_command_policy: TargetCommandPolicy::Optional,
        })
        .expect("profile");

        assert_eq!(profile.metrics.total_commands, 0);
        assert!(profile.metrics.completed);
    }

    #[test]
    fn forbidden_target_commands_fail_when_completed_run_uses_target_tool() {
        let events = vec![CommandEvent {
            command: "notes add hello".to_string(),
            exit_code: Some(0),
        }];
        let target = target();

        let error = build_interaction_profile(InteractionProfileInput {
            target: &target,
            interaction_input: &InteractionInput::StructuredToolCalls(events),
            adapter_capability: AdapterEvidenceCapability::StructuredToolCalls,
            transcript_path: unused_transcript_path(),
            completed: true,
            target_command_policy: TargetCommandPolicy::Forbidden,
        })
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("target-tool events are forbidden"));
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
            target_command_policy: TargetCommandPolicy::Required,
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
            target_command_policy: TargetCommandPolicy::Required,
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
            target_command_policy: TargetCommandPolicy::Required,
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

    #[test]
    fn mcp_structured_evidence_reduces_shared_metrics() {
        let target =
            TargetInteractionSpec::mcp("todo", vec!["add".to_string(), "list".to_string()]);
        let events = vec![
            McpToolCallEvent {
                server: "todo".to_string(),
                tool: "add".to_string(),
                arguments: serde_json::json!({"text":"a"}),
                is_error: true,
                duration_ms: Some(10),
            },
            McpToolCallEvent {
                server: "todo".to_string(),
                tool: "add".to_string(),
                arguments: serde_json::json!({"text":"a"}),
                is_error: false,
                duration_ms: Some(9),
            },
            McpToolCallEvent {
                server: "todo".to_string(),
                tool: "list".to_string(),
                arguments: serde_json::json!({}),
                is_error: false,
                duration_ms: None,
            },
        ];

        let profile = build_interaction_profile(InteractionProfileInput {
            target: &target,
            interaction_input: &InteractionInput::StructuredMcpToolCalls(events),
            adapter_capability: AdapterEvidenceCapability::StructuredToolCalls,
            transcript_path: unused_transcript_path(),
            completed: true,
            target_command_policy: TargetCommandPolicy::Required,
        })
        .expect("profile");

        assert_eq!(
            profile.evidence_source,
            InteractionEvidenceSource::StructuredMcpToolCalls
        );
        assert_eq!(profile.metrics.total_commands, 3);
        assert_eq!(profile.metrics.unique_commands, 2);
        assert_eq!(profile.metrics.error_count, 1);
        assert_eq!(profile.metrics.tool_reuse_count, 1);
        assert_eq!(profile.metrics.first_try_success_rate, 1.0 / 3.0);
    }

    #[test]
    fn mixed_structured_evidence_uses_mcp_events_for_mcp_targets() {
        let target = TargetInteractionSpec::mcp("todo", vec!["add".to_string()]);
        let interaction_input = InteractionInput::StructuredMixedToolCalls {
            commands: vec![CommandEvent {
                command: "notes list".to_string(),
                exit_code: Some(0),
            }],
            mcp: vec![McpToolCallEvent {
                server: "todo".to_string(),
                tool: "add".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
                is_error: false,
                duration_ms: None,
            }],
        };

        let profile = build_interaction_profile(InteractionProfileInput {
            target: &target,
            interaction_input: &interaction_input,
            adapter_capability: AdapterEvidenceCapability::StructuredToolCalls,
            transcript_path: unused_transcript_path(),
            completed: true,
            target_command_policy: TargetCommandPolicy::Required,
        })
        .expect("profile");

        assert_eq!(
            profile.evidence_source,
            InteractionEvidenceSource::StructuredMcpToolCalls
        );
        assert_eq!(profile.metrics.total_commands, 1);
        assert_eq!(profile.metrics.unique_commands, 1);
    }

    #[test]
    fn mcp_undeclared_tool_counts_and_warns() {
        let target = TargetInteractionSpec::mcp("todo", vec!["add".to_string()]);
        let events = vec![McpToolCallEvent {
            server: "todo".to_string(),
            tool: "delete".to_string(),
            arguments: serde_json::json!({}),
            is_error: false,
            duration_ms: None,
        }];

        let profile = build_interaction_profile(InteractionProfileInput {
            target: &target,
            interaction_input: &InteractionInput::StructuredMcpToolCalls(events),
            adapter_capability: AdapterEvidenceCapability::StructuredToolCalls,
            transcript_path: unused_transcript_path(),
            completed: true,
            target_command_policy: TargetCommandPolicy::Required,
        })
        .expect("profile");

        assert_eq!(profile.metrics.total_commands, 1);
        assert_eq!(profile.warnings.len(), 1);
        assert!(profile.warnings[0].contains("undeclared tool 'delete'"));
    }

    #[test]
    fn mcp_target_matches_server_identity() {
        let target = TargetInteractionSpec::mcp("todo", vec!["add".to_string()]);
        let events = vec![McpToolCallEvent {
            server: "other".to_string(),
            tool: "add".to_string(),
            arguments: serde_json::json!({}),
            is_error: false,
            duration_ms: None,
        }];

        let error = build_interaction_profile(InteractionProfileInput {
            target: &target,
            interaction_input: &InteractionInput::StructuredMcpToolCalls(events),
            adapter_capability: AdapterEvidenceCapability::StructuredToolCalls,
            transcript_path: unused_transcript_path(),
            completed: true,
            target_command_policy: TargetCommandPolicy::Required,
        })
        .unwrap_err();

        assert!(error.to_string().contains("no usable structured"));
    }

    #[test]
    fn completed_structured_mcp_run_requires_target_tool_events() {
        let target = TargetInteractionSpec::mcp("todo", vec!["add".to_string()]);

        let error = build_interaction_profile(InteractionProfileInput {
            target: &target,
            interaction_input: &InteractionInput::StructuredMcpToolCalls(vec![]),
            adapter_capability: AdapterEvidenceCapability::StructuredToolCalls,
            transcript_path: unused_transcript_path(),
            completed: true,
            target_command_policy: TargetCommandPolicy::Required,
        })
        .unwrap_err();

        assert!(error.to_string().contains("no usable structured"));
    }
}
