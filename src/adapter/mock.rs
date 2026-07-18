use super::{ToolAdapter, ToolRunOutput};
use crate::interaction_evidence::{CommandEvent, InteractionInput};
use crate::scenario::Scenario;
use crate::target_env::TargetEnvironment;
use std::path::Path;

pub struct MockAdapter;

impl MockAdapter {
    pub fn generate_command_events(&self, scenario: &Scenario) -> Vec<CommandEvent> {
        vec![CommandEvent {
            command: format!("{} mock", scenario.target.display_name()),
            exit_code: Some(0),
        }]
    }

    pub fn generate_transcript(&self, scenario: &Scenario) -> String {
        let mut transcript = String::new();

        for event in self.generate_command_events(scenario) {
            transcript.push_str("$ ");
            transcript.push_str(&event.command);
            transcript.push('\n');
            transcript.push_str("Mock execution completed successfully\n");
            transcript.push_str(&format!("exit code: {}\n\n", event.exit_code.unwrap_or(0)));
        }

        transcript
    }
}

impl ToolAdapter for MockAdapter {
    fn supports_structured_tool_calls(&self) -> bool {
        true
    }

    fn is_available(&self) -> Result<super::ToolStatus, super::AdapterError> {
        Ok(super::ToolStatus {
            available: true,
            authenticated: true,
        })
    }

    fn run(
        &self,
        scenario: &Scenario,
        _cwd: &Path,
        _model: Option<&str>,
        _timeout_secs: u64,
        _target_env: &TargetEnvironment,
    ) -> anyhow::Result<ToolRunOutput> {
        let transcript = self.generate_transcript(scenario);
        let command_events = self.generate_command_events(scenario);
        Ok(ToolRunOutput {
            transcript,
            raw_output: None,
            exit_code: 0,
            cost_usd: None,
            token_usage: None,
            interaction_input: InteractionInput::StructuredToolCalls(command_events),
        })
    }
}
