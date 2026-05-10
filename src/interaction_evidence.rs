use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEvent {
    pub command: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InteractionInput {
    StructuredToolCalls(Vec<CommandEvent>),
    TranscriptRegex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionEvidenceSource {
    StructuredToolCalls,
    TranscriptRegexFallback,
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
