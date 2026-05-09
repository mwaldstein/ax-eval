use super::{ToolAdapter, ToolRunOutput};
use crate::scenario::Scenario;
use crate::transcript::InteractionInput;
use std::path::Path;

pub struct MockAdapter;

impl MockAdapter {
    pub fn generate_transcript(&self, _scenario: &Scenario) -> String {
        // Generate a simple mock transcript without executing any commands
        // This is used for testing the framework without requiring a real tool
        "mock command output\nMock execution completed successfully".to_string()
    }
}

impl ToolAdapter for MockAdapter {
    fn is_available(&self) -> Result<super::ToolStatus, super::AdapterError> {
        Ok(super::ToolStatus {
            available: true,
            authenticated: true,
        })
    }

    fn check_availability(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn run(
        &self,
        scenario: &Scenario,
        _cwd: &Path,
        _model: Option<&str>,
        _timeout_secs: u64,
    ) -> anyhow::Result<ToolRunOutput> {
        // Generate mock output without executing any commands
        let transcript = self.generate_transcript(scenario);
        Ok(ToolRunOutput {
            transcript,
            raw_output: None,
            exit_code: 0,
            cost_usd: None,
            token_usage: None,
            interaction_input: InteractionInput::TranscriptRegex,
        })
    }
}
