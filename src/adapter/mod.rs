pub mod claude_code;
pub mod mock;
pub mod opencode;

#[cfg(test)]
mod mock_test;

use crate::scenario::Scenario;
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
    ) -> anyhow::Result<(String, i32, Option<f64>, Option<TokenUsage>)>;
}
