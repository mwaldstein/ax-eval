use crate::scenario::{load, Scenario};
use crate::utils::resolve_fixtures_path;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ScenarioRecord {
    pub path: PathBuf,
    pub scenario: Scenario,
}

#[derive(Debug, Clone)]
pub struct ScenarioSummary {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ScenarioCatalog {
    fixtures_dir: PathBuf,
}

impl ScenarioCatalog {
    pub fn from_default_fixtures() -> Self {
        Self::new(resolve_fixtures_path(""))
    }

    pub fn new(fixtures_dir: PathBuf) -> Self {
        Self { fixtures_dir }
    }

    pub fn resolve_path(&self, path: &str) -> PathBuf {
        let requested = Path::new(path);
        if requested.is_absolute() || requested.exists() {
            return requested.to_path_buf();
        }

        let fixture_path = self.fixtures_dir.join(path);
        if fixture_path.exists() {
            fixture_path
        } else {
            self.fixtures_dir.join(format!("{}.yaml", path))
        }
    }

    pub fn load_one(&self, path: &str) -> anyhow::Result<ScenarioRecord> {
        let resolved_path = self.resolve_path(path);
        let scenario = load(&resolved_path)?;
        Ok(ScenarioRecord {
            path: resolved_path,
            scenario,
        })
    }

    pub fn select_all(&self, tags: &[String], tier: usize) -> anyhow::Result<Vec<ScenarioRecord>> {
        let mut selected = Vec::new();

        for path in self.scenario_paths() {
            let Ok(scenario) = load(&path) else {
                continue;
            };
            if scenario_matches(&scenario, tags, tier) {
                selected.push(ScenarioRecord { path, scenario });
            }
        }

        Ok(selected)
    }

    pub fn summaries(&self, tags: &[String], tier: usize) -> anyhow::Result<Vec<ScenarioSummary>> {
        let mut summaries = Vec::new();

        for path in self.scenario_paths() {
            let Ok(scenario) = load(&path) else {
                continue;
            };
            if scenario_matches(&scenario, tags, tier) {
                summaries.push(ScenarioSummary {
                    name: scenario.name,
                    description: scenario.description,
                    tags: scenario.tags,
                });
            }
        }

        Ok(summaries)
    }

    pub fn yaml_paths(&self) -> Vec<PathBuf> {
        if !self.fixtures_dir.exists() {
            return Vec::new();
        }

        let mut paths = Vec::new();
        collect_yaml_paths_excluding(&self.fixtures_dir, &mut paths, "templates");
        paths.sort();
        paths
    }

    fn scenario_paths(&self) -> Vec<PathBuf> {
        if !self.fixtures_dir.exists() {
            return Vec::new();
        }

        let mut paths = Vec::new();
        collect_yaml_paths(&self.fixtures_dir, &mut paths);
        paths.sort();
        paths
    }
}

fn collect_yaml_paths(dir: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_yaml_paths(&path, paths);
        } else if path.extension().is_some_and(|ext| ext == "yaml") {
            paths.push(path);
        }
    }
}

fn collect_yaml_paths_excluding(dir: &Path, paths: &mut Vec<PathBuf>, exclude: &str) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == exclude) {
                continue;
            }
            collect_yaml_paths_excluding(&path, paths, exclude);
        } else if path.extension().is_some_and(|ext| ext == "yaml") {
            paths.push(path);
        }
    }
}

fn scenario_matches(scenario: &Scenario, tags: &[String], tier: usize) -> bool {
    let tier_match = scenario.tier <= tier;
    let tags_match = tags.is_empty() || tags.iter().any(|tag| scenario.tags.contains(tag));

    tier_match && tags_match
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_scenario(path: &Path, name: &str, tier: usize, tags: &[&str]) {
        let tags_yaml = tags
            .iter()
            .map(|tag| format!("  - {}", tag))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(
            path,
            format!(
                r#"
name: {name}
description: "{name} description"
tier: {tier}
tags:
{tags_yaml}
template_folder: qipu
target:
  binary: qipu
task:
  prompt: "Test"
evaluation:
  gates:
    - type: command_succeeds
      command: "true"
"#
            ),
        )
        .expect("write scenario");
    }

    #[test]
    fn resolves_fixture_relative_names_with_yaml_extension() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scenario_path = dir.path().join("basic.yaml");
        write_scenario(&scenario_path, "basic", 0, &["smoke"]);

        let catalog = ScenarioCatalog::new(dir.path().to_path_buf());
        assert_eq!(catalog.resolve_path("basic"), scenario_path);
    }

    #[test]
    fn selects_scenarios_by_any_tag_and_tier() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_scenario(&dir.path().join("smoke.yaml"), "smoke", 0, &["smoke"]);
        write_scenario(
            &dir.path().join("integration.yaml"),
            "integration",
            1,
            &["integration"],
        );
        write_scenario(&dir.path().join("slow.yaml"), "slow", 2, &["slow"]);

        let catalog = ScenarioCatalog::new(dir.path().to_path_buf());
        let tags = vec!["smoke".to_string(), "integration".to_string()];
        let selected = catalog.select_all(&tags, 1).expect("select scenarios");
        let names = selected
            .iter()
            .map(|record| record.scenario.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["integration", "smoke"]);
    }

    #[test]
    fn discovery_skips_yaml_files_that_are_not_scenarios() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_scenario(&dir.path().join("scenario.yaml"), "scenario", 0, &["smoke"]);
        fs::write(dir.path().join("rubric.yaml"), "criteria: []\n").expect("write rubric");

        let catalog = ScenarioCatalog::new(dir.path().to_path_buf());
        let selected = catalog.select_all(&[], 0).expect("select scenarios");

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].scenario.name, "scenario");
    }

    #[test]
    fn yaml_paths_excludes_templates_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_scenario(&dir.path().join("scenario.yaml"), "scenario", 0, &["smoke"]);

        let templates_dir = dir.path().join("templates").join("sub");
        fs::create_dir_all(&templates_dir).expect("create templates dir");
        fs::write(templates_dir.join("rubric.yaml"), "criteria: []\n").expect("write rubric");

        let catalog = ScenarioCatalog::new(dir.path().to_path_buf());
        let paths = catalog.yaml_paths();

        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("scenario.yaml"));
    }
}
