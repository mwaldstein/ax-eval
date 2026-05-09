pub mod claude_code;
pub mod codex;
pub mod mock;
pub(crate) mod normalize;
pub mod opencode;

#[cfg(test)]
mod mock_test;

use crate::scenario::Scenario;
use crate::transcript::{CommandEvent, InteractionMetricsSource};
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
    /// Which canonical artifact must be used for interaction metrics.
    pub metrics_source: InteractionMetricsSource,
    /// Canonical command events, used when `metrics_source` is `StructuredToolCalls`.
    pub command_events: Vec<CommandEvent>,
}

/// Trait for tool adapters that execute LLM CLI tools.
pub trait ToolAdapter: Send + Sync {
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
    ) -> anyhow::Result<ToolRunOutput>;
}
