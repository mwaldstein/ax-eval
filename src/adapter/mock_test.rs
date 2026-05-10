#[cfg(test)]
mod tests {
    use crate::adapter::mock::MockAdapter;
    use crate::adapter::ToolAdapter;
    use crate::interaction_evidence::InteractionInput;
    use crate::scenario::Scenario;
    use crate::target_env::TargetEnvironment;

    #[test]
    fn test_mock_adapter_is_available() {
        let adapter = MockAdapter;
        assert!(adapter.check_availability().is_ok());
    }

    #[test]
    fn test_mock_adapter_generates_transcript() {
        let adapter = MockAdapter;
        let scenario_yaml = r#"
name: test
description: "Test scenario"
template_folder: mock_template
target:
  binary: mock
task:
  prompt: "Test prompt"
evaluation:
  gates: []
"#;
        let scenario: Scenario = yaml_serde::from_str(scenario_yaml).unwrap();
        let transcript = adapter.generate_transcript(&scenario);

        // The mock adapter should return a non-empty transcript
        assert!(!transcript.is_empty());
    }

    #[test]
    fn test_mock_adapter_run_returns_success() {
        let adapter = MockAdapter;

        let scenario_yaml = r#"
name: test
description: "Test scenario"
template_folder: mock_template
target:
  binary: mock
task:
  prompt: "Test prompt"
evaluation:
  gates: []
"#;
        let scenario: Scenario = yaml_serde::from_str(scenario_yaml).unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let result = adapter.run(
            &scenario,
            temp_dir.path(),
            Some("mock"),
            30,
            &TargetEnvironment::default(),
        );

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.exit_code, 0, "Exit code should be 0");
        assert!(!output.transcript.is_empty(), "Output should not be empty");
        let InteractionInput::StructuredToolCalls(events) = output.interaction_input else {
            panic!("mock adapter should provide structured tool calls");
        };
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].command, "mock mock");
        assert_eq!(events[0].exit_code, Some(0));
    }

    #[test]
    fn test_mock_adapter_run_with_gates() {
        let adapter = MockAdapter;

        let scenario_yaml = r#"
name: test_with_gates
description: "Test scenario with gates"
template_folder: mock_template
target:
  binary: mock
task:
  prompt: "Test prompt"
evaluation:
  gates:
    - type: command_succeeds
      command: "echo test"
"#;
        let scenario: Scenario = yaml_serde::from_str(scenario_yaml).unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let result = adapter.run(
            &scenario,
            temp_dir.path(),
            Some("mock"),
            30,
            &TargetEnvironment::default(),
        );

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(!output.transcript.is_empty());
    }

    #[test]
    fn test_mock_adapter_cost_and_token_usage() {
        let adapter = MockAdapter;

        let scenario_yaml = r#"
name: cost_test
description: "Test cost and token usage"
template_folder: mock_template
target:
  binary: mock
task:
  prompt: "Test prompt"
evaluation:
  gates: []
"#;
        let scenario: Scenario = yaml_serde::from_str(scenario_yaml).unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let result = adapter.run(
            &scenario,
            temp_dir.path(),
            Some("mock"),
            30,
            &TargetEnvironment::default(),
        );

        assert!(result.is_ok());
        let output = result.unwrap();
        // Mock adapter doesn't report cost or token usage
        assert!(output.cost_usd.is_none());
        assert!(output.token_usage.is_none());
    }
}
