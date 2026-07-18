use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEvent {
    pub command: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolCallEvent {
    pub server: String,
    pub tool: String,
    pub arguments: serde_json::Value,
    pub is_error: bool,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InteractionInput {
    StructuredToolCalls(Vec<CommandEvent>),
    StructuredMcpToolCalls(Vec<McpToolCallEvent>),
    TranscriptRegex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionEvidenceSource {
    StructuredToolCalls,
    StructuredMcpToolCalls,
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
pub enum TargetSpec {
    Cli {
        binary: String,
        command_pattern: Option<String>,
    },
    Mcp {
        server: String,
        tools: std::collections::HashSet<String>,
    },
}

pub type TargetInteractionSpec = TargetSpec;

impl TargetSpec {
    pub fn new(binary: impl Into<String>, command_pattern: Option<String>) -> Self {
        Self::Cli {
            binary: binary.into(),
            command_pattern,
        }
    }

    pub fn mcp(server: impl Into<String>, tools: impl IntoIterator<Item = String>) -> Self {
        Self::Mcp {
            server: server.into(),
            tools: tools.into_iter().collect(),
        }
    }

    pub fn binary(&self) -> &str {
        match self {
            Self::Cli { binary, .. } => binary,
            Self::Mcp { .. } => panic!("binary is only available for CLI targets"),
        }
    }

    pub fn command_pattern(&self) -> Option<&str> {
        match self {
            Self::Cli {
                command_pattern, ..
            } => command_pattern.as_deref(),
            Self::Mcp { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_mcp_tool_calls_serialize_with_snake_case_source() {
        let input = InteractionInput::StructuredMcpToolCalls(vec![McpToolCallEvent {
            server: "todo".to_string(),
            tool: "add".to_string(),
            arguments: serde_json::json!({"text":"hello"}),
            is_error: false,
            duration_ms: Some(12),
        }]);

        let value = serde_json::to_value(&input).expect("serialize input");

        assert_eq!(value["StructuredMcpToolCalls"][0]["server"], "todo");
        assert_eq!(value["StructuredMcpToolCalls"][0]["tool"], "add");

        let source = serde_json::to_string(&InteractionEvidenceSource::StructuredMcpToolCalls)
            .expect("serialize source");
        assert_eq!(source, "\"structured_mcp_tool_calls\"");
    }
}
