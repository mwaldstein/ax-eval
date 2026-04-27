use super::super::*;

#[test]
fn test_load_scenario_without_tool_matrix() {
    let yaml = r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test prompt"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario: Scenario = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(scenario.name, "test");
    assert!(scenario.tool_matrix.is_none());
}

#[test]
fn test_load_scenario_with_tool_matrix() {
    let yaml = r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test prompt"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
tool_matrix:
  - tool: opencode
    models: [claude-sonnet-4-20250514, gpt-4o]
  - tool: claude-code
    models: [default]
"#;
    let scenario: Scenario = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(scenario.name, "test");
    assert!(scenario.tool_matrix.is_some());

    let matrix = scenario.tool_matrix.unwrap();
    assert_eq!(matrix.len(), 2);
    assert_eq!(matrix[0].tool, "opencode");
    assert_eq!(matrix[0].models, vec!["claude-sonnet-4-20250514", "gpt-4o"]);
    assert_eq!(matrix[1].tool, "claude-code");
    assert_eq!(matrix[1].models, vec!["default"]);
}

#[test]
fn test_load_scenario_with_empty_models() {
    let yaml = r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test prompt"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
tool_matrix:
  - tool: opencode
    models: []
  - tool: claude-code
"#;
    let scenario: Scenario = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(scenario.name, "test");
    assert!(scenario.tool_matrix.is_some());

    let matrix = scenario.tool_matrix.unwrap();
    assert_eq!(matrix.len(), 2);
    assert_eq!(matrix[0].tool, "opencode");
    assert!(matrix[0].models.is_empty());
    assert_eq!(matrix[1].tool, "claude-code");
    assert!(matrix[1].models.is_empty());
}

#[test]
fn test_load_scenario_with_tier() {
    let yaml = r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test prompt"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
tier: 1
"#;
    let scenario: Scenario = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(scenario.name, "test");
    assert_eq!(scenario.tier, 1);
    assert!(scenario.tool_matrix.is_none());
}

#[test]
fn test_default_tier() {
    let yaml = r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test prompt"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
    let scenario: Scenario = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(scenario.name, "test");
    assert_eq!(scenario.tier, 0);
}

#[test]
fn test_tags_field() {
    let yaml = r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test prompt"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
tags: [capture, links, retrieval]
"#;
    let scenario: Scenario = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(scenario.name, "test");
    assert_eq!(scenario.tags, vec!["capture", "links", "retrieval"]);
}

#[test]
fn test_complete_scenario() {
    let yaml = r#"
name: capture_article_basic
description: "Capture article ideas as linked notes"
template_folder: qipu
target:
  binary: qipu
tags: [capture, links, retrieval]
task:
  prompt: "Capture key ideas from this article"
tool_matrix:
  - tool: amp
    models: [default]
  - tool: opencode
    models: [default]
run:
  timeout_secs: 600
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
    - type: no_transcript_errors
setup:
  commands:
    - "qipu init"
"#;
    let scenario: Scenario = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(scenario.name, "capture_article_basic");
    assert_eq!(
        scenario.description,
        "Capture article ideas as linked notes"
    );
    assert_eq!(scenario.tags, vec!["capture", "links", "retrieval"]);

    // Run config
    assert!(scenario.run.is_some());
    let run = scenario.run.unwrap();
    assert_eq!(run.timeout_secs, Some(600));

    // Setup
    assert!(scenario.setup.is_some());
    let setup = scenario.setup.unwrap();
    assert_eq!(setup.commands.len(), 1);
    assert_eq!(setup.commands[0], "qipu init");

    // Tool matrix
    assert!(scenario.tool_matrix.is_some());
    let matrix = scenario.tool_matrix.unwrap();
    assert_eq!(matrix.len(), 2);

    // Evaluation
    assert_eq!(scenario.evaluation.gates.len(), 2);
}
