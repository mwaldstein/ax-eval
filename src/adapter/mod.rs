pub mod claude_code;
pub mod codex;
pub mod mock;
pub(crate) mod normalize;
pub mod opencode;
pub mod registry;

#[cfg(test)]
mod mock_test;

use crate::interaction_evidence::{CommandEvent, InteractionInput, McpToolCallEvent};
use crate::scenario::{Scenario, TargetConfig};
use crate::target_env::{AgentEnvironment, TargetEnvironment};
use std::path::Path;

/// Error type for adapter operations.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("Tool not available: {0}")]
    NotAvailable(String),

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

/// Status of a tool's availability.
#[derive(Debug, Clone)]
pub struct ToolStatus {
    pub available: bool,
    pub authenticated: bool,
}

/// Token usage statistics.
#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub input: usize,
    pub output: usize,
}

/// Output from a tool execution.
#[derive(Debug, Clone)]
pub struct ToolRunOutput {
    /// Canonical transcript text used for human review and fallback analysis.
    pub transcript: String,
    /// Original tool output, when it differs from the transcript.
    pub raw_output: Option<String>,
    pub exit_code: i32,
    pub cost_usd: Option<f64>,
    pub token_usage: Option<TokenUsage>,
    /// Canonical input for interaction metrics.
    pub interaction_input: InteractionInput,
}

impl ToolRunOutput {
    pub fn command_events(&self) -> Option<&[CommandEvent]> {
        match &self.interaction_input {
            InteractionInput::StructuredToolCalls(events) => Some(events),
            InteractionInput::StructuredMixedToolCalls { commands, .. } => Some(commands),
            InteractionInput::StructuredMcpToolCalls(_) | InteractionInput::TranscriptRegex => None,
        }
    }

    pub fn mcp_tool_call_events(&self) -> Option<&[McpToolCallEvent]> {
        match &self.interaction_input {
            InteractionInput::StructuredMcpToolCalls(events) => Some(events),
            InteractionInput::StructuredMixedToolCalls { mcp, .. } => Some(mcp),
            InteractionInput::StructuredToolCalls(_) | InteractionInput::TranscriptRegex => None,
        }
    }
}

pub struct TargetProvision {
    cleanup: Option<Box<dyn FnOnce() -> anyhow::Result<()> + Send>>,
}

impl TargetProvision {
    pub fn none() -> Self {
        Self { cleanup: None }
    }

    pub fn with_cleanup(cleanup: impl FnOnce() -> anyhow::Result<()> + Send + 'static) -> Self {
        Self {
            cleanup: Some(Box::new(cleanup)),
        }
    }

    pub fn cleanup(mut self) -> anyhow::Result<()> {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup()?;
        }

        Ok(())
    }
}

/// Trait for tool adapters that execute LLM CLI tools.
pub trait ToolAdapter: Send + Sync {
    /// Whether real runs through this adapter should preflight MCP `tools/list`.
    fn requires_mcp_inspection(&self) -> bool {
        true
    }

    /// Whether this host reads MCP bearer credentials from the agent process.
    fn requires_mcp_bearer_env(&self) -> bool {
        false
    }

    /// Parent environment names required to launch and authenticate this agent.
    fn required_agent_env(&self) -> &'static [&'static str] {
        &[
            "HOME",
            "PATH",
            "USER",
            "LOGNAME",
            "SHELL",
            "TERM",
            "COLORTERM",
            "TMPDIR",
            "TMP",
            "TEMP",
            "LANG",
            "LC_ALL",
            "LC_CTYPE",
            "XDG_CACHE_HOME",
            "XDG_DATA_HOME",
            "SSH_AUTH_SOCK",
        ]
    }

    /// Whether this adapter provides structured tool-call evidence for interaction profiles.
    fn supports_structured_tool_calls(&self) -> bool {
        false
    }

    /// Write agent-native config so the agent can discover target MCP servers.
    fn provision_target(
        &self,
        target: &TargetConfig,
        _workspace: &Path,
        _target_env: &TargetEnvironment,
    ) -> anyhow::Result<TargetProvision> {
        match target {
            TargetConfig::Cli(_) => Ok(TargetProvision::none()),
            TargetConfig::Mcp(target) => anyhow::bail!(
                "MCP target '{}' is not supported by this adapter",
                target.name
            ),
        }
    }

    /// Check if tool is installed and authenticated.
    fn is_available(&self) -> Result<ToolStatus, AdapterError>;

    /// Check if the tool is available and ready to use.
    fn check_availability(&self) -> anyhow::Result<()> {
        match self.is_available() {
            Ok(status) if status.available => Ok(()),
            Ok(status) if !status.authenticated => Err(anyhow::anyhow!("Tool not authenticated")),
            Ok(_) => Err(anyhow::anyhow!("Tool not available")),
            Err(e) => Err(e.into()),
        }
    }

    /// Run the tool with the given scenario in the specified working directory.
    /// Returns the tool output, exit code, estimated cost in USD (if available), and token usage (if available).
    fn run(
        &self,
        scenario: &Scenario,
        cwd: &Path,
        model: Option<&str>,
        timeout_secs: u64,
        agent_env: &AgentEnvironment,
    ) -> anyhow::Result<ToolRunOutput>;
}
