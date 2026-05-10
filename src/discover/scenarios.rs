use super::manifest::{display_path, GeneratedScenarioManifest};
use crate::scenario::{self, Scenario};
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) struct ValidScenario {
    pub(super) path: PathBuf,
    pub(super) scenario: Scenario,
}

pub(super) fn validate_generated_scenarios(
    scenarios_dir: &Path,
) -> Result<(Vec<GeneratedScenarioManifest>, Vec<ValidScenario>)> {
    let mut paths = Vec::new();
    collect_scenario_yaml_paths(scenarios_dir, &mut paths)?;
    paths.sort();

    let mut manifests = Vec::new();
    let mut valid_scenarios = Vec::new();

    for path in paths {
        let mut diagnostics = Vec::new();
        match scenario::load(&path) {
            Ok(mut scenario) => {
                diagnostics.extend(scenario::discovery::apply_discovery_contract(
                    &path,
                    &mut scenario,
                ));
                let valid = diagnostics.is_empty();
                manifests.push(GeneratedScenarioManifest {
                    path: display_path(&path),
                    valid,
                    name: Some(scenario.name.clone()),
                    diagnostics,
                });
                if valid {
                    valid_scenarios.push(ValidScenario { path, scenario });
                }
            }
            Err(error) => {
                manifests.push(GeneratedScenarioManifest {
                    path: display_path(&path),
                    valid: false,
                    name: None,
                    diagnostics: vec![format!("{error:#}")],
                });
            }
        }
    }

    Ok((manifests, valid_scenarios))
}

fn collect_scenario_yaml_paths(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|name| name.to_str());
            if matches!(name, Some("rubrics" | "templates")) {
                continue;
            }
            collect_scenario_yaml_paths(&path, paths)?;
        } else if path
            .extension()
            .is_some_and(|ext| ext == "yaml" || ext == "yml")
        {
            paths.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_discovery_scenario_contract_and_normalizes_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenarios = dir.path().join("scenarios");
        fs::create_dir_all(scenarios.join("templates/goal")).expect("template dir");
        fs::create_dir_all(scenarios.join("rubrics")).expect("rubrics dir");
        fs::write(
            scenarios.join("rubrics/goal.yaml"),
            r#"
criteria:
  - id: quality
    weight: 1.0
    description: "Quality"
output:
  format: json
  require_fields: [scores, weighted_score, confidence, issues, highlights]
"#,
        )
        .expect("rubric");
        fs::write(
            scenarios.join("goal.yaml"),
            r#"
name: goal
description: "Goal"
template_folder: templates/goal
target:
  binary: qipu
task:
  prompt: "Use qipu well"
evaluation:
  gates: []
  judge:
    enabled: true
    rubric: rubrics/goal.yaml
    pass_threshold: 0.70
"#,
        )
        .expect("scenario");

        let (manifests, valid) =
            validate_generated_scenarios(&scenarios).expect("validate scenarios");

        assert_eq!(manifests.len(), 1);
        assert!(manifests[0].valid);
        assert_eq!(valid.len(), 1);
        assert!(Path::new(&valid[0].scenario.template_folder).is_absolute());
        assert!(Path::new(
            valid[0]
                .scenario
                .evaluation
                .judge
                .as_ref()
                .expect("judge")
                .rubric
                .as_ref()
                .expect("rubric")
        )
        .is_absolute());
    }

    #[test]
    fn validates_discovery_scenario_with_default_judge_rubric() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenarios = dir.path().join("scenarios");
        fs::create_dir_all(scenarios.join("templates/goal")).expect("template dir");
        fs::write(
            scenarios.join("goal.yaml"),
            r#"
name: goal
description: "Goal"
template_folder: templates/goal
target:
  binary: qipu
task:
  prompt: "Use qipu well"
evaluation:
  gates: []
  judge:
    enabled: true
    pass_threshold: 0.70
"#,
        )
        .expect("scenario");

        let (manifests, valid) =
            validate_generated_scenarios(&scenarios).expect("validate scenarios");

        assert_eq!(manifests.len(), 1);
        assert!(manifests[0].valid);
        assert_eq!(valid.len(), 1);
        assert!(Path::new(&valid[0].scenario.template_folder).is_absolute());
        assert_eq!(
            valid[0]
                .scenario
                .evaluation
                .judge
                .as_ref()
                .expect("judge")
                .rubric,
            None
        );
    }

    #[test]
    fn discovery_contract_rejects_gated_or_unjudged_scenarios() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenarios = dir.path().join("scenarios");
        fs::create_dir_all(&scenarios).expect("scenarios dir");
        fs::write(
            scenarios.join("bad.yaml"),
            r#"
name: bad
description: "Bad"
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Use qipu"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#,
        )
        .expect("scenario");

        let (manifests, valid) =
            validate_generated_scenarios(&scenarios).expect("validate scenarios");

        assert!(valid.is_empty());
        assert!(!manifests[0].valid);
        assert!(manifests[0]
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("must not include deterministic gates")));
        assert!(manifests[0]
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("must include an enabled judge")));
    }
}
