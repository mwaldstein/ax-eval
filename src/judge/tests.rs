//! Tests for judge module.

use super::eval::build_judge_prompt;
use super::rubric::load_rubric;
use super::types::{Criterion, OutputFormat, Rubric};

#[test]
fn test_build_judge_prompt() {
    let rubric = Rubric {
        criteria: vec![Criterion {
            id: "test_criterion".to_string(),
            weight: 1.0,
            description: "Test description".to_string(),
        }],
        output: OutputFormat {
            format: "json".to_string(),
            require_fields: vec!["scores".to_string()],
        },
    };

    let prompt = build_judge_prompt("Test task", "/path/to/transcript.txt", &rubric);

    assert!(prompt.contains("Test task"));
    assert!(prompt.contains("/path/to/transcript.txt"));
    assert!(prompt.contains("test_criterion"));
    assert!(prompt.contains("Test description"));
    assert!(prompt.contains("weighted_score"));
}

#[test]
fn test_build_judge_prompt_multiple_criteria() {
    let rubric = Rubric {
        criteria: vec![
            Criterion {
                id: "correctness".to_string(),
                weight: 0.40,
                description: "Uses valid commands".to_string(),
            },
            Criterion {
                id: "efficiency".to_string(),
                weight: 0.60,
                description: "Minimal wasted effort".to_string(),
            },
        ],
        output: OutputFormat {
            format: "json".to_string(),
            require_fields: vec!["scores".to_string(), "weighted_score".to_string()],
        },
    };

    let prompt = build_judge_prompt("Create 3 notes", "/tmp/transcript.txt", &rubric);

    assert!(prompt.contains("correctness"));
    assert!(prompt.contains("efficiency"));
    assert!(prompt.contains("0.40"));
    assert!(prompt.contains("0.60"));
}

#[test]
fn test_load_rubric_weights_sum_to_one() {
    let rubric_yaml = r#"
criteria:
  - id: relevance
    weight: 0.35
    description: "Notes directly address the task prompt"
  - id: coherence
    weight: 0.35
    description: "Notes are logically connected"
  - id: granularity
    weight: 0.30
    description: "Notes are appropriately scoped"
output:
  format: json
  require_fields:
    - scores
    - weighted_score
"#;
    let temp_dir = tempfile::tempdir().unwrap();
    let rubric_path = temp_dir.path().join("test_rubric.yaml");
    std::fs::write(&rubric_path, rubric_yaml).unwrap();

    let rubric = load_rubric(&rubric_path).unwrap();
    assert_eq!(rubric.criteria.len(), 3);
    assert_eq!(rubric.criteria[0].id, "relevance");
    assert_eq!(rubric.criteria[0].weight, 0.35);
}

#[test]
fn test_load_rubric_weights_sum_error() {
    let rubric_yaml = r#"
criteria:
  - id: criterion1
    weight: 0.5
    description: "First criterion"
  - id: criterion2
    weight: 0.4
    description: "Second criterion"
output:
  format: json
  require_fields:
    - scores
"#;
    let temp_dir = tempfile::tempdir().unwrap();
    let rubric_path = temp_dir.path().join("bad_rubric.yaml");
    std::fs::write(&rubric_path, rubric_yaml).unwrap();

    let result = load_rubric(&rubric_path);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must sum to 1.0"));
}
