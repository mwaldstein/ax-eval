use crate::scenario::Scenario;
use std::path::Path;

/// Apply the Discovery-authored scenario contract in place.
///
/// Discovery scenarios are generated artifacts, not canonical project fixtures:
/// they must stay ungated and use Judge evaluation as their primary result.
pub fn apply_discovery_contract(path: &Path, scenario: &mut Scenario) -> Vec<String> {
    normalize_generated_paths(path, scenario);
    validate_discovery_contract(scenario)
}

fn validate_discovery_contract(scenario: &Scenario) -> Vec<String> {
    let mut diagnostics = Vec::new();

    if !scenario.evaluation.gates.is_empty() {
        diagnostics.push("Discovery scenarios must not include deterministic gates".to_string());
    }
    match &scenario.evaluation.judge {
        Some(judge) if judge.enabled => {}
        Some(_) => diagnostics.push("Discovery scenario judge must be enabled".to_string()),
        None => diagnostics.push("Discovery scenarios must include an enabled judge".to_string()),
    }

    diagnostics
}

fn normalize_generated_paths(path: &Path, scenario: &mut Scenario) {
    let Some(parent) = path.parent() else {
        return;
    };
    let template_folder = Path::new(&scenario.template_folder);
    if !template_folder.is_absolute() {
        let local = parent.join(template_folder);
        if local.exists() {
            scenario.template_folder = display_path(&local);
        }
    }
    if let Some(judge) = &mut scenario.evaluation.judge {
        if let Some(rubric) = &mut judge.rubric {
            let rubric_path = Path::new(rubric);
            if !rubric_path.is_absolute() {
                let local = parent.join(rubric_path);
                if local.exists() {
                    *rubric = display_path(&local);
                }
            }
        }
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{Evaluation, JudgeConfig, Scenario, TargetConfig, Task};

    fn discovery_scenario() -> Scenario {
        Scenario {
            name: "generated_goal".to_string(),
            description: "Generated discovery scenario".to_string(),
            template_folder: "templates/generated_goal".to_string(),
            target: TargetConfig::cli_target("qipu"),
            task: Task {
                prompt: "Use qipu well".to_string(),
            },
            evaluation: Evaluation {
                gates: vec![],
                judge: Some(JudgeConfig {
                    enabled: true,
                    tool: None,
                    rubric: Some("rubrics/generated_goal.yaml".to_string()),
                    criteria: vec![],
                    pass_threshold: 0.7,
                }),
                composite: None,
            },
            tier: 0,
            tool_matrix: None,
            setup: None,
            tags: vec!["discovery".to_string()],
            run: None,
            scripts: None,
            interaction: Default::default(),
        }
    }

    #[test]
    fn discovery_contract_accepts_ungated_judged_scenario_and_normalizes_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenario_path = dir.path().join("goal.yaml");
        std::fs::create_dir_all(dir.path().join("templates/generated_goal")).expect("template dir");
        std::fs::create_dir_all(dir.path().join("rubrics")).expect("rubrics dir");
        std::fs::write(
            dir.path().join("rubrics/generated_goal.yaml"),
            "criteria: []",
        )
        .expect("rubric");
        let mut scenario = discovery_scenario();

        let diagnostics = apply_discovery_contract(&scenario_path, &mut scenario);

        assert!(diagnostics.is_empty());
        assert!(Path::new(&scenario.template_folder).is_absolute());
        assert!(Path::new(
            scenario
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
    fn discovery_contract_rejects_gated_or_unjudged_scenarios() {
        let mut scenario = discovery_scenario();
        scenario
            .evaluation
            .gates
            .push(crate::scenario::Gate::CommandSucceeds {
                command: "true".to_string(),
            });
        scenario.evaluation.judge = None;

        let diagnostics = apply_discovery_contract(Path::new("goal.yaml"), &mut scenario);

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("must not include deterministic gates")));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("must include an enabled judge")));
    }
}
