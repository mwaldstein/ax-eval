use super::super::*;

#[test]
fn test_command_succeeds_gate() {
    let yaml = r#"
name: test
description: "Test"
template_folder: fixture
target:
  binary: tool
task:
  prompt: "Test prompt"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();

    match &scenario.evaluation.gates[0] {
        Gate::CommandSucceeds { command } => assert_eq!(command, "true"),
        _ => panic!("Expected CommandSucceeds gate"),
    }
}

#[test]
fn test_judge_config_with_tool() {
    let yaml = r#"
name: test
description: "Test"
template_folder: fixture
target:
  binary: tool
task:
  prompt: "Test prompt"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
  judge:
    enabled: true
    tool: codex
    rubric: rubrics/test.yaml
    pass_threshold: 0.75
"#;
    let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();
    let judge = scenario.evaluation.judge.expect("judge config");

    assert!(judge.enabled);
    assert_eq!(judge.tool.as_deref(), Some("codex"));
    assert_eq!(judge.rubric, "rubrics/test.yaml");
    assert_eq!(judge.pass_threshold, 0.75);
}

#[test]
fn test_command_output_contains_gate() {
    let yaml = r#"
name: test
description: "Test"
template_folder: fixture
target:
  binary: tool
task:
  prompt: "Test prompt"
evaluation:
  gates:
    - type: command_output_contains
      command: "printf hello"
      substring: "hell"
"#;
    let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();

    match &scenario.evaluation.gates[0] {
        Gate::CommandOutputContains { command, substring } => {
            assert_eq!(command, "printf hello");
            assert_eq!(substring, "hell");
        }
        _ => panic!("Expected CommandOutputContains gate"),
    }
}

#[test]
fn test_command_output_matches_gate() {
    let yaml = r#"
name: test
description: "Test"
template_folder: fixture
target:
  binary: tool
task:
  prompt: "Test prompt"
evaluation:
  gates:
    - type: command_output_matches
      command: "printf hello"
      pattern: "^hello$"
"#;
    let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();

    match &scenario.evaluation.gates[0] {
        Gate::CommandOutputMatches { command, pattern } => {
            assert_eq!(command, "printf hello");
            assert_eq!(pattern, "^hello$");
        }
        _ => panic!("Expected CommandOutputMatches gate"),
    }
}

#[test]
fn test_command_json_path_gate() {
    let yaml = r#"
name: test
description: "Test"
template_folder: fixture
target:
  binary: tool
task:
  prompt: "Test prompt"
evaluation:
  gates:
    - type: command_json_path
      command: "echo '{\"ok\": true}'"
      path: "$.ok"
      assertion: "equals true"
"#;
    let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();

    match &scenario.evaluation.gates[0] {
        Gate::CommandJsonPath {
            command,
            path,
            assertion,
        } => {
            assert_eq!(command, "echo '{\"ok\": true}'");
            assert_eq!(path, "$.ok");
            assert_eq!(assertion, "equals true");
        }
        _ => panic!("Expected CommandJsonPath gate"),
    }
}

#[test]
fn test_file_gates() {
    let yaml = r#"
name: test
description: "Test"
template_folder: fixture
target:
  binary: tool
task:
  prompt: "Test prompt"
evaluation:
  gates:
    - type: file_exists
      path: "README.md"
    - type: file_contains
      path: "README.md"
      substring: "hello"
    - type: file_matches
      path: "README.md"
      pattern: "hello.*world"
"#;
    let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();

    match &scenario.evaluation.gates[0] {
        Gate::FileExists { path } => assert_eq!(path, "README.md"),
        _ => panic!("Expected FileExists gate"),
    }

    match &scenario.evaluation.gates[1] {
        Gate::FileContains { path, substring } => {
            assert_eq!(path, "README.md");
            assert_eq!(substring, "hello");
        }
        _ => panic!("Expected FileContains gate"),
    }

    match &scenario.evaluation.gates[2] {
        Gate::FileMatches { path, pattern } => {
            assert_eq!(path, "README.md");
            assert_eq!(pattern, "hello.*world");
        }
        _ => panic!("Expected FileMatches gate"),
    }
}

#[test]
fn test_script_gate() {
    let yaml = r#"
name: test
description: "Test"
template_folder: fixture
target:
  binary: tool
task:
  prompt: "Test prompt"
evaluation:
  gates:
    - type: script
      command: "./scripts/check.sh"
      description: "custom check"
"#;
    let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();

    match &scenario.evaluation.gates[0] {
        Gate::Script {
            command,
            description,
            timeout_secs,
        } => {
            assert_eq!(command, "./scripts/check.sh");
            assert_eq!(description, "custom check");
            assert_eq!(*timeout_secs, 30);
        }
        _ => panic!("Expected Script gate"),
    }
}

#[test]
fn test_no_transcript_errors_gate() {
    let yaml = r#"
name: test
description: "Test"
template_folder: fixture
target:
  binary: tool
task:
  prompt: "Test prompt"
evaluation:
  gates:
    - type: no_transcript_errors
"#;
    let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();

    match &scenario.evaluation.gates[0] {
        Gate::NoTranscriptErrors => {}
        _ => panic!("Expected NoTranscriptErrors gate"),
    }
}
