//! Tests for judge module.

use super::eval::build_judge_prompt;
use super::rubric::load_rubric;
use super::types::{Criterion, OutputFormat, Rubric};

fn single_criterion_rubric() -> Rubric {
    Rubric {
        criteria: vec![Criterion {
            id: "test_criterion".to_string(),
            weight: 1.0,
            description: "Test description".to_string(),
        }],
        output: OutputFormat {
            format: "json".to_string(),
            require_fields: vec!["scores".to_string()],
        },
    }
}

#[test]
fn test_build_judge_prompt_includes_tool_name() {
    let rubric = single_criterion_rubric();
    let prompt = build_judge_prompt("my-tool", "Do the task", "/path/to/transcript.txt", &rubric);

    assert!(prompt.contains("my-tool"));
    assert!(prompt.contains("how effectively an LLM agent used the CLI tool `my-tool`"));
}

#[test]
fn test_build_judge_prompt_includes_task_and_transcript() {
    let rubric = single_criterion_rubric();
    let prompt = build_judge_prompt("notes", "Create 3 notes", "/tmp/transcript.txt", &rubric);

    assert!(prompt.contains("Create 3 notes"));
    assert!(prompt.contains("/tmp/transcript.txt"));
}

#[test]
fn test_build_judge_prompt_includes_criteria() {
    let rubric = single_criterion_rubric();
    let prompt = build_judge_prompt("notes", "task", "/t.txt", &rubric);

    assert!(prompt.contains("test_criterion"));
    assert!(prompt.contains("Test description"));
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

    let prompt = build_judge_prompt("taskmgr", "Create tasks", "/tmp/t.txt", &rubric);

    assert!(prompt.contains("taskmgr"));
    assert!(prompt.contains("correctness"));
    assert!(prompt.contains("efficiency"));
    assert!(prompt.contains("0.40"));
    assert!(prompt.contains("0.60"));
}

#[test]
fn test_build_judge_prompt_requests_rationale() {
    let rubric = single_criterion_rubric();
    let prompt = build_judge_prompt("notes", "task", "/t.txt", &rubric);

    assert!(prompt.contains("rationale"));
}

#[test]
fn test_build_judge_prompt_requests_judge_result_envelope() {
    let rubric = single_criterion_rubric();
    let prompt = build_judge_prompt("notes", "task", "/t.txt", &rubric);

    assert!(prompt.contains("<judge_result>"));
    assert!(prompt.contains("</judge_result>"));
    assert!(prompt.contains("Do not put prose, markdown, or code fences inside <judge_result>"));
}

#[test]
fn test_build_judge_prompt_uses_tool_name_in_examples() {
    let rubric = single_criterion_rubric();
    let prompt = build_judge_prompt("qipu", "task", "/t.txt", &rubric);

    assert!(prompt.contains("`qipu create`"));
    assert!(prompt.contains("`qipu search`"));
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
