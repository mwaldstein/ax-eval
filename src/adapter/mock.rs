use super::{TargetProvision, ToolAdapter, ToolRunOutput};
use crate::interaction_evidence::{CommandEvent, InteractionInput, McpToolCallEvent};
use crate::scenario::{Scenario, TargetConfig};
use crate::target_env::{AgentEnvironment, TargetEnvironment};
use std::path::Path;

pub struct MockAdapter;

impl MockAdapter {
    pub fn generate_command_events(&self, scenario: &Scenario) -> Vec<CommandEvent> {
        vec![CommandEvent {
            command: format!("{} mock", scenario.target.display_name()),
            exit_code: Some(0),
        }]
    }

    pub fn generate_mcp_tool_call_events(&self, scenario: &Scenario) -> Vec<McpToolCallEvent> {
        let TargetConfig::Mcp(target) = &scenario.target else {
            return Vec::new();
        };

        vec![McpToolCallEvent {
            server: target.name.clone(),
            tool: target
                .tools
                .first()
                .cloned()
                .unwrap_or_else(|| "mock".to_string()),
            arguments: serde_json::json!({}),
            is_error: false,
            duration_ms: None,
        }]
    }

    pub fn generate_transcript(&self, scenario: &Scenario) -> String {
        if scenario.target.is_mcp() {
            return format!(
                "Called MCP server {} successfully\n",
                scenario.target.display_name()
            );
        }

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
    fn requires_mcp_inspection(&self) -> bool {
        false
    }

    fn supports_structured_tool_calls(&self) -> bool {
        true
    }

    fn is_available(&self) -> Result<super::ToolStatus, super::AdapterError> {
        Ok(super::ToolStatus {
            available: true,
            authenticated: true,
        })
    }

    fn provision_target(
        &self,
        _target: &TargetConfig,
        _workspace: &Path,
        _target_env: &TargetEnvironment,
    ) -> anyhow::Result<TargetProvision> {
        Ok(TargetProvision::none())
    }

    fn run(
        &self,
        scenario: &Scenario,
        _cwd: &Path,
        _model: Option<&str>,
        _timeout_secs: u64,
        _agent_env: &AgentEnvironment,
    ) -> anyhow::Result<ToolRunOutput> {
        let transcript = self.generate_transcript(scenario);
        let interaction_input = match &scenario.target {
            TargetConfig::Cli(_) => {
                InteractionInput::StructuredToolCalls(self.generate_command_events(scenario))
            }
            TargetConfig::Mcp(_) => InteractionInput::StructuredMcpToolCalls(
                self.generate_mcp_tool_call_events(scenario),
            ),
        };
        Ok(ToolRunOutput {
            transcript,
            raw_output: None,
            exit_code: 0,
            cost_usd: None,
            token_usage: None,
            interaction_input,
        })
    }
}
