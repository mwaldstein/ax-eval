use std::path::Path;

use super::{Gate, Scenario};
use crate::judge::load_rubric;
use crate::utils::resolve_fixtures_path;

#[derive(Debug)]
pub struct ValidationWarning {
    pub field: String,
    pub message: String,
}

#[derive(Debug)]
pub struct ValidationResult {
    pub name: String,
    pub path: String,
    pub warnings: Vec<ValidationWarning>,
}

pub fn validate_scenario_file(path: &Path) -> anyhow::Result<ValidationResult> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {e}", path.display()))?;

    let scenario: Scenario = yaml_serde::from_str(&content).map_err(|e| {
        let msg = format_yaml_error(&content, &e.to_string());
        anyhow::anyhow!("{path}: {msg}", path = path.display())
    })?;

    let warnings = validate_scenario(&scenario);

    if let Some(ref judge) = scenario.evaluation.judge {
        if let Some(ref rubric) = judge.rubric {
            let rubric_path = resolve_rubric_path_for_validation(rubric, path);
            if !rubric_path.exists() {
                anyhow::bail!(
                    "rubric file not found: {} (resolved from {})",
                    rubric_path.display(),
                    rubric
                );
            }
            if let Err(e) = load_rubric(&rubric_path) {
                anyhow::bail!("invalid rubric {}: {}", rubric_path.display(), e);
            }
        }
    }

    Ok(ValidationResult {
        name: scenario.name.clone(),
        path: path.display().to_string(),
        warnings,
    })
}

fn resolve_rubric_path_for_validation(rubric: &str, scenario_path: &Path) -> std::path::PathBuf {
    let rubric_path = Path::new(rubric);
    if rubric_path.is_absolute() {
        return rubric_path.to_path_buf();
    }
    if let Some(scenario_dir) = scenario_path.parent() {
        let scenario_relative = scenario_dir.join(rubric_path);
        if scenario_relative.exists() {
            return scenario_relative;
        }
    }
    resolve_fixtures_path(rubric)
}

fn format_yaml_error(content: &str, raw_error: &str) -> String {
    let error = raw_error.to_lowercase();

    if let Some(pos) = error.find("missing field") {
        let field_name = extract_quoted_after(&error[pos..], "`")
            .or_else(|| extract_quoted_after(&error[pos..], "'"))
            .unwrap_or("unknown");
        return format!("missing required field: {field_name}");
    }

    if error.contains("unknown variant") {
        if let Some(variant) = extract_quoted_after(&error, "`") {
            let known = [
                "command_succeeds",
                "command_output_contains",
                "command_output_matches",
                "command_json_path",
                "file_exists",
                "file_contains",
                "file_matches",
                "no_transcript_errors",
                "script",
            ];
            if let Some(suggestion) = find_similar(variant, &known) {
                return format!("unknown gate type: {variant}, did you mean {suggestion}?");
            }
            return format!(
                "unknown gate type: {variant}. Valid types: {}",
                known.join(", ")
            );
        }
    }

    if error.contains("invalid type") {
        if let Some(found) = extract_quoted_after(&error, "got ") {
            return format!("wrong type: expected a different structure ({found})");
        }
    }

    if let Some(line_hint) = try_extract_line_number(&error, content) {
        return format!("line {line_hint}: {raw_error}");
    }

    raw_error.to_string()
}

fn extract_quoted_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let start = text.find(marker)?;
    let rest = &text[start + marker.len()..];
    let end = rest.find(marker).unwrap_or(rest.len());
    Some(&rest[..end])
}

fn find_similar<'a>(input: &str, candidates: &[&'a str]) -> Option<&'a str> {
    let input_lower = input.to_lowercase();
    let mut best: Option<(&str, usize)> = None;
    for candidate in candidates {
        let dist = levenshtein_distance(&input_lower, candidate);
        if dist > 0 && dist <= 3 {
            match best {
                None => best = Some((candidate, dist)),
                Some((_, best_dist)) if dist < best_dist => best = Some((candidate, dist)),
                _ => {}
            }
        }
    }
    best.map(|(c, _)| c)
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    let b_chars: Vec<char> = b.chars().collect();
    let a_chars: Vec<char> = a.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr: Vec<usize> = vec![0; b_len + 1];

    for i in 1..=a_len {
        curr[0] = i;
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b_len]
}

fn try_extract_line_number(error: &str, content: &str) -> Option<usize> {
    for line in error.lines() {
        let line = line.to_lowercase();
        if let Some(pos) = line.find("at line ") {
            if let Some(num_str) = extract_digits_after(&line[pos + 8..]) {
                if let Ok(n) = num_str.parse::<usize>() {
                    return Some(n);
                }
            }
        }
        if let Some(pos) = line.find("at byte ") {
            if let Ok(byte_offset) = extract_digits_after(&line[pos + 8..])
                .unwrap_or("")
                .parse::<usize>()
            {
                let line_num = content[..byte_offset.min(content.len())].lines().count();
                if line_num > 0 {
                    return Some(line_num);
                }
            }
        }
    }
    None
}

fn extract_digits_after(s: &str) -> Option<&str> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if end > 0 {
        Some(&s[..end])
    } else {
        None
    }
}

pub fn validate_scenario(scenario: &Scenario) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();

    if scenario.task.prompt.trim().is_empty() {
        warnings.push(ValidationWarning {
            field: "task.prompt".to_string(),
            message: "prompt is empty — the agent will have no task to perform".to_string(),
        });
    }

    if scenario.evaluation.gates.is_empty() {
        warnings.push(ValidationWarning {
            field: "evaluation.gates".to_string(),
            message: "no gates defined — the scenario has no pass/fail assertions".to_string(),
        });
    }

    for (i, gate) in scenario.evaluation.gates.iter().enumerate() {
        match gate {
            Gate::CommandOutputMatches { pattern, .. } => {
                if let Err(e) = regex::Regex::new(pattern) {
                    warnings.push(ValidationWarning {
                        field: format!("evaluation.gates[{i}].pattern"),
                        message: format!("invalid regex: {e}"),
                    });
                }
            }
            Gate::FileMatches { pattern, .. } => {
                if let Err(e) = regex::Regex::new(pattern) {
                    warnings.push(ValidationWarning {
                        field: format!("evaluation.gates[{i}].pattern"),
                        message: format!("invalid regex: {e}"),
                    });
                }
            }
            _ => {}
        }
    }

    if let Some(ref judge) = scenario.evaluation.judge {
        if judge.enabled && judge.rubric.is_none() && judge.criteria.is_empty() {
            warnings.push(ValidationWarning {
                field: "evaluation.judge".to_string(),
                message:
                    "judge is enabled but has no rubric or criteria — scoring will use defaults"
                        .to_string(),
            });
        }

        if !(0.0..=1.0).contains(&judge.pass_threshold) {
            warnings.push(ValidationWarning {
                field: "evaluation.judge.pass_threshold".to_string(),
                message: format!("pass_threshold {} is outside 0.0–1.0", judge.pass_threshold),
            });
        }
    }

    if let Some(ref setup) = scenario.setup {
        if setup.commands.is_empty() {
            warnings.push(ValidationWarning {
                field: "setup.commands".to_string(),
                message: "setup block exists but has no commands".to_string(),
            });
        }
    }

    if let Some(ref target_pattern) = scenario.target.command_pattern {
        if let Err(e) = regex::Regex::new(target_pattern) {
            warnings.push(ValidationWarning {
                field: "target.command_pattern".to_string(),
                message: format!("invalid regex: {e}"),
            });
        }
    }

    if let Some(ref composite) = scenario.evaluation.composite {
        let total = composite.judge_weight + composite.gate_weight + composite.interaction_weight;
        if (total - 1.0).abs() > f64::EPSILON * 100.0 {
            warnings.push(ValidationWarning {
                field: "evaluation.composite".to_string(),
                message: format!(
                    "composite weights sum to {total:.2}, expected 1.0 (judge={} gate={} interaction={})",
                    composite.judge_weight, composite.gate_weight, composite.interaction_weight
                ),
            });
        }
    }

    if let Some(ref scripts) = scenario.scripts {
        for (i, evaluator) in scripts.evaluators.iter().enumerate() {
            if evaluator.name.trim().is_empty() {
                warnings.push(ValidationWarning {
                    field: format!("scripts.evaluators[{i}].name"),
                    message: "evaluator name is empty — it will be hard to identify in results"
                        .to_string(),
                });
            }
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_yaml() -> String {
        r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#
        .to_string()
    }

    #[test]
    fn valid_minimal_scenario_has_no_warnings() {
        let scenario: Scenario = yaml_serde::from_str(&minimal_yaml()).unwrap();
        let warnings = validate_scenario(&scenario);
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn empty_prompt_generates_warning() {
        let yaml = r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "   "
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
        let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();
        let warnings = validate_scenario(&scenario);
        assert!(warnings.iter().any(|w| w.field == "task.prompt"));
    }

    #[test]
    fn no_gates_generates_warning() {
        let yaml = r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
evaluation:
  gates: []
"#;
        let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();
        let warnings = validate_scenario(&scenario);
        assert!(warnings.iter().any(|w| w.field == "evaluation.gates"));
    }

    #[test]
    fn invalid_gate_regex_generates_warning() {
        let yaml = r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: file_matches
      path: out.txt
      pattern: "[invalid"
"#;
        let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();
        let warnings = validate_scenario(&scenario);
        assert!(
            warnings
                .iter()
                .any(|w| w.field == "evaluation.gates[0].pattern"),
            "expected regex warning, got: {warnings:?}"
        );
    }

    #[test]
    fn invalid_command_pattern_generates_warning() {
        let yaml = r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
  command_pattern: "[invalid"
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
        let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();
        let warnings = validate_scenario(&scenario);
        assert!(
            warnings.iter().any(|w| w.field == "target.command_pattern"),
            "expected command_pattern warning, got: {warnings:?}"
        );
    }

    #[test]
    fn judge_enabled_without_rubric_or_criteria_warns() {
        let yaml = r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
  judge:
    enabled: true
    pass_threshold: 0.7
"#;
        let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();
        let warnings = validate_scenario(&scenario);
        assert!(warnings.iter().any(|w| w.field == "evaluation.judge"));
    }

    #[test]
    fn judge_with_criteria_does_not_warn() {
        let yaml = r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
  judge:
    enabled: true
    pass_threshold: 0.7
    criteria:
      - id: task_completion
        weight: 1.0
        description: "Agent completed the task"
"#;
        let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();
        let warnings = validate_scenario(&scenario);
        assert!(
            !warnings.iter().any(|w| w.field == "evaluation.judge"),
            "unexpected judge warning: {warnings:?}"
        );
    }

    #[test]
    fn judge_threshold_out_of_range_warns() {
        let yaml = r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
  judge:
    enabled: false
    pass_threshold: 1.5
"#;
        let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();
        let warnings = validate_scenario(&scenario);
        assert!(
            warnings
                .iter()
                .any(|w| w.field == "evaluation.judge.pass_threshold"),
            "expected threshold warning, got: {warnings:?}"
        );
    }

    #[test]
    fn composite_weights_not_summing_to_one_warns() {
        let yaml = r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
  composite:
    judge_weight: 0.5
    gate_weight: 0.3
    interaction_weight: 0.1
"#;
        let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();
        let warnings = validate_scenario(&scenario);
        assert!(
            warnings.iter().any(|w| w.field == "evaluation.composite"),
            "expected composite warning, got: {warnings:?}"
        );
    }

    #[test]
    fn empty_setup_commands_warns() {
        let yaml = r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
setup:
  commands: []
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#;
        let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();
        let warnings = validate_scenario(&scenario);
        assert!(warnings.iter().any(|w| w.field == "setup.commands"));
    }

    #[test]
    fn empty_evaluator_name_warns() {
        let yaml = r#"
name: test
description: "Test"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
scripts:
  evaluators:
    - command: "./score.sh"
      name: "   "
"#;
        let scenario: Scenario = yaml_serde::from_str(yaml).unwrap();
        let warnings = validate_scenario(&scenario);
        assert!(
            warnings
                .iter()
                .any(|w| w.field == "scripts.evaluators[0].name"),
            "expected evaluator name warning, got: {warnings:?}"
        );
    }

    #[test]
    fn format_error_missing_field() {
        let error = format_yaml_error("", "missing field `target`");
        assert!(error.contains("missing required field: target"));
    }

    #[test]
    fn format_error_unknown_gate_type_with_suggestion() {
        let error = format_yaml_error("", "unknown variant `file_exits`, expected one of ..");
        assert!(error.contains("did you mean file_exists"));
    }

    #[test]
    fn format_error_unknown_gate_type_without_suggestion() {
        let error = format_yaml_error("", "unknown variant `totally_wrong`, expected one of ..");
        assert!(error.contains("unknown gate type: totally_wrong"));
        assert!(error.contains("command_succeeds"));
    }

    #[test]
    fn validate_file_missing_target() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.yaml");
        std::fs::write(
            &path,
            r#"
name: test
description: "Missing target"
template_folder: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#,
        )
        .unwrap();

        let result = validate_scenario_file(&path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("missing required field"),
            "expected helpful error, got: {err}"
        );
    }

    #[test]
    fn validate_file_unknown_gate_type() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.yaml");
        std::fs::write(
            &path,
            r#"
name: test
description: "Bad gate"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: file_exits
      path: out.txt
"#,
        )
        .unwrap();

        let result = validate_scenario_file(&path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("did you mean file_exists"),
            "expected suggestion, got: {err}"
        );
    }

    #[test]
    fn validate_file_valid_scenario() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("good.yaml");
        std::fs::write(&path, minimal_yaml()).unwrap();

        let result = validate_scenario_file(&path).unwrap();
        assert_eq!(result.name, "test");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn validate_file_missing_rubric() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scenario.yaml");
        std::fs::write(
            &path,
            r#"
name: test
description: "Missing rubric"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
  judge:
    enabled: true
    rubric: rubrics/missing.yaml
    pass_threshold: 0.7
"#,
        )
        .unwrap();

        let result = validate_scenario_file(&path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("rubric file not found"),
            "expected rubric not found error, got: {err}"
        );
    }

    #[test]
    fn validate_file_invalid_rubric_weights() {
        let dir = tempfile::tempdir().unwrap();
        let rubrics_dir = dir.path().join("rubrics");
        std::fs::create_dir_all(&rubrics_dir).unwrap();
        std::fs::write(
            rubrics_dir.join("bad.yaml"),
            r#"
criteria:
  - id: test
    weight: 0.5
    description: "Bad weights"
output:
  format: json
  require_fields: [scores, rationale]
"#,
        )
        .unwrap();

        let path = dir.path().join("scenario.yaml");
        std::fs::write(
            &path,
            r#"
name: test
description: "Invalid rubric"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
  judge:
    enabled: true
    rubric: rubrics/bad.yaml
    pass_threshold: 0.7
"#,
        )
        .unwrap();

        let result = validate_scenario_file(&path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Rubric criterion weights must sum to 1.0"),
            "expected weight validation error, got: {err}"
        );
    }

    #[test]
    fn validate_file_valid_rubric() {
        let dir = tempfile::tempdir().unwrap();
        let rubrics_dir = dir.path().join("rubrics");
        std::fs::create_dir_all(&rubrics_dir).unwrap();
        std::fs::write(
            rubrics_dir.join("good.yaml"),
            r#"
criteria:
  - id: test
    weight: 1.0
    description: "Good weights"
output:
  format: json
  require_fields: [scores, rationale]
"#,
        )
        .unwrap();

        let path = dir.path().join("scenario.yaml");
        std::fs::write(
            &path,
            r#"
name: test
description: "Valid rubric"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Do something"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
  judge:
    enabled: true
    rubric: rubrics/good.yaml
    pass_threshold: 0.7
"#,
        )
        .unwrap();

        let result = validate_scenario_file(&path).unwrap();
        assert_eq!(result.name, "test");
    }
}
