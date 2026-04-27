use super::super::*;

#[test]
fn test_run_config() {
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
run:
  timeout_secs: 600
"#;
    let scenario: Scenario = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(scenario.name, "test");
    assert!(scenario.run.is_some());
    let run = scenario.run.unwrap();
    assert_eq!(run.timeout_secs, Some(600));
}

#[test]
fn test_run_config_optional() {
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
    assert!(scenario.run.is_none());
}

#[test]
fn test_run_config_partial() {
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
run:
  timeout_secs: 300
"#;
    let scenario: Scenario = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(scenario.name, "test");
    assert!(scenario.run.is_some());
    let run = scenario.run.unwrap();
    assert_eq!(run.timeout_secs, Some(300));
}
