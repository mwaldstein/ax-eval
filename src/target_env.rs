use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TargetEnvironment {
    vars: HashMap<String, String>,
}

impl TargetEnvironment {
    pub fn expanded_from_config(
        target_env: Option<&HashMap<String, String>>,
        fixture_dir: &Path,
        results_dir: &Path,
    ) -> Self {
        let vars = target_env
            .map(|vars| {
                vars.iter()
                    .map(|(key, value)| {
                        (
                            key.clone(),
                            expand_target_env_value(value, fixture_dir, results_dir),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        Self { vars }
    }

    pub fn as_map(&self) -> &HashMap<String, String> {
        &self.vars
    }

    pub fn to_session_env(&self) -> Vec<(String, String)> {
        self.vars
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }

    pub fn append_to_session_env(&self, env: &mut Vec<(String, String)>) {
        env.extend(self.to_session_env());
    }
}

fn expand_target_env_value(value: &str, fixture_dir: &Path, results_dir: &Path) -> String {
    let fixture_dir = absolute_path(fixture_dir);
    let results_dir = absolute_path(results_dir);

    value
        .replace(
            "${LLM_TOOL_TEST_FIXTURE_DIR}",
            &fixture_dir.to_string_lossy(),
        )
        .replace(
            "${LLM_TOOL_TEST_RESULTS_DIR}",
            &results_dir.to_string_lossy(),
        )
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_fixture_and_results_placeholders() {
        let dir = tempfile::tempdir().expect("tempdir");
        let fixture_dir = dir.path().join("results").join("fixture");
        let results_dir = dir.path().join("results");

        let mut target_env = HashMap::new();
        target_env.insert(
            "MYTOOL_ROOT_DIR".to_string(),
            "${LLM_TOOL_TEST_FIXTURE_DIR}".to_string(),
        );
        target_env.insert(
            "MYTOOL_EXPORT".to_string(),
            "${LLM_TOOL_TEST_RESULTS_DIR}/export.json".to_string(),
        );

        let expanded =
            TargetEnvironment::expanded_from_config(Some(&target_env), &fixture_dir, &results_dir);

        assert_eq!(
            expanded.as_map().get("MYTOOL_ROOT_DIR"),
            Some(&fixture_dir.to_string_lossy().to_string())
        );
        assert_eq!(
            expanded.as_map().get("MYTOOL_EXPORT"),
            Some(&format!("{}/export.json", results_dir.to_string_lossy()))
        );
    }

    #[test]
    fn expands_relative_fixture_and_results_placeholders_to_absolute_paths() {
        let unique = format!(
            "target/target-env-abs-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let results_dir = Path::new(&unique).join("results");
        let fixture_dir = results_dir.join("fixture");
        std::fs::create_dir_all(&fixture_dir).expect("create fixture dir");

        let mut target_env = HashMap::new();
        target_env.insert(
            "MYTOOL_ROOT_DIR".to_string(),
            "${LLM_TOOL_TEST_FIXTURE_DIR}".to_string(),
        );
        target_env.insert(
            "MYTOOL_EXPORT".to_string(),
            "${LLM_TOOL_TEST_RESULTS_DIR}/export.json".to_string(),
        );

        let expanded =
            TargetEnvironment::expanded_from_config(Some(&target_env), &fixture_dir, &results_dir);

        let expected_fixture = std::env::current_dir()
            .expect("current dir")
            .join(&fixture_dir);
        let expected_results = std::env::current_dir()
            .expect("current dir")
            .join(&results_dir);

        assert_eq!(
            expanded.as_map().get("MYTOOL_ROOT_DIR"),
            Some(&expected_fixture.to_string_lossy().to_string())
        );
        assert_eq!(
            expanded.as_map().get("MYTOOL_EXPORT"),
            Some(&format!(
                "{}/export.json",
                expected_results.to_string_lossy()
            ))
        );
    }

    #[test]
    fn converts_to_session_env() {
        let mut target_env = HashMap::new();
        target_env.insert("TARGET_ENV_TEST".to_string(), "works".to_string());
        target_env.insert("ANOTHER_VAR".to_string(), "also works".to_string());

        let session_env = TargetEnvironment::expanded_from_config(
            Some(&target_env),
            Path::new(""),
            Path::new(""),
        )
        .to_session_env();

        assert!(session_env.contains(&("TARGET_ENV_TEST".to_string(), "works".to_string())));
        assert!(session_env.contains(&("ANOTHER_VAR".to_string(), "also works".to_string())));
    }
}
