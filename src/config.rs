use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Configuration for a specific LLM tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// Tool name (e.g., "opencode", "claude-code")
    pub name: String,
    /// Command to execute the tool
    pub command: String,
    /// List of supported model names
    #[serde(default)]
    pub models: Vec<String>,
}

/// Configuration for a test profile defining a matrix of tool/model combinations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    /// Profile name
    pub name: String,
    /// List of tool names to include in this profile
    pub tools: Vec<String>,
    /// List of model names to include in this profile
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Tool configurations
    #[serde(default)]
    pub tools: HashMap<String, ToolConfig>,
    /// Profile configurations for test matrices
    #[serde(default)]
    pub profiles: HashMap<String, ProfileConfig>,
    #[serde(default)]
    pub fixtures_path: Option<String>,
    #[serde(default)]
    pub results_path: Option<String>,
}

impl Config {
    pub fn load_or_default() -> Self {
        let config_path = Path::new("ax-eval-config.toml");

        if config_path.exists() {
            match Self::load(config_path) {
                Ok(config) => return config,
                Err(e) => {
                    eprintln!("Warning: Failed to load config file: {}", e);
                    eprintln!("Using default configuration");
                }
            }
        }

        Self::default()
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read config file: {}", e))?;

        let config: Config =
            toml::from_str(&content).map_err(|e| format!("Failed to parse config file: {}", e))?;

        Ok(config)
    }

    pub fn get_fixtures_path(&self) -> &str {
        self.fixtures_path.as_deref().unwrap_or("ax-eval-fixtures")
    }

    pub fn get_results_path(&self) -> &str {
        self.results_path.as_deref().unwrap_or("ax-eval-results")
    }

    /// Get a tool configuration by name.
    pub fn get_tool(&self, name: &str) -> Option<&ToolConfig> {
        self.tools.get(name)
    }

    /// Get a profile configuration by name.
    pub fn get_profile(&self, name: &str) -> Option<&ProfileConfig> {
        self.profiles.get(name)
    }

    /// Build a matrix of tool-model combinations from a profile.
    /// Validates that each tool supports its assigned models.
    pub fn build_profile_matrix(
        &self,
        profile_name: &str,
    ) -> Result<Vec<(String, String)>, String> {
        let profile = self
            .get_profile(profile_name)
            .ok_or_else(|| format!("Profile '{}' not found in configuration", profile_name))?;

        let mut matrix = Vec::new();

        for tool_name in &profile.tools {
            let tool = self
                .get_tool(tool_name)
                .ok_or_else(|| format!("Tool '{}' not found in configuration", tool_name))?;

            for model_name in &profile.models {
                // Validate that the tool supports this model
                if !tool.models.is_empty() && !tool.models.contains(&model_name.to_string()) {
                    return Err(format!(
                        "Tool '{}' does not support model '{}'",
                        tool_name, model_name
                    ));
                }
                matrix.push((tool_name.clone(), model_name.clone()));
            }
        }

        Ok(matrix)
    }

    /// Validate that a tool supports a specific model.
    pub fn validate_tool_model(&self, tool_name: &str, model_name: &str) -> Result<(), String> {
        let tool = self
            .get_tool(tool_name)
            .ok_or_else(|| format!("Tool '{}' not found in configuration", tool_name))?;

        if !tool.models.is_empty() && !tool.models.contains(&model_name.to_string()) {
            return Err(format!(
                "Tool '{}' does not support model '{}'",
                tool_name, model_name
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.tools.is_empty());
        assert!(config.profiles.is_empty());
    }

    #[test]
    fn test_load_and_save() {
        let mut config = Config::default();
        config.tools.insert(
            "opencode".to_string(),
            ToolConfig {
                name: "opencode".to_string(),
                command: "opencode".to_string(),
                models: vec!["gpt-4o".to_string()],
            },
        );

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-config.toml");

        let content = toml::to_string_pretty(&config).unwrap();
        fs::write(&path, content).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.tools.len(), config.tools.len());
    }

    #[test]
    fn test_build_profile_matrix() {
        let mut config = Config::default();

        // Add a tool
        config.tools.insert(
            "opencode".to_string(),
            ToolConfig {
                name: "opencode".to_string(),
                command: "opencode".to_string(),
                models: vec!["gpt-4o".to_string(), "claude-sonnet".to_string()],
            },
        );

        // Add a profile
        config.profiles.insert(
            "standard".to_string(),
            ProfileConfig {
                name: "standard".to_string(),
                tools: vec!["opencode".to_string()],
                models: vec!["gpt-4o".to_string()],
            },
        );

        let matrix = config.build_profile_matrix("standard").unwrap();
        assert_eq!(matrix.len(), 1);
        assert_eq!(matrix[0], ("opencode".to_string(), "gpt-4o".to_string()));
    }

    #[test]
    fn test_build_profile_matrix_invalid_model() {
        let mut config = Config::default();

        // Add a tool with limited models
        config.tools.insert(
            "opencode".to_string(),
            ToolConfig {
                name: "opencode".to_string(),
                command: "opencode".to_string(),
                models: vec!["gpt-4o".to_string()],
            },
        );

        // Add a profile with unsupported model
        config.profiles.insert(
            "standard".to_string(),
            ProfileConfig {
                name: "standard".to_string(),
                tools: vec!["opencode".to_string()],
                models: vec!["unsupported-model".to_string()],
            },
        );

        let result = config.build_profile_matrix("standard");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not support"));
    }

    #[test]
    fn example_config_profiles_are_valid() {
        let config = Config::load(Path::new("ax-eval-config.example.toml")).unwrap();

        for profile_name in config.profiles.keys() {
            config
                .build_profile_matrix(profile_name)
                .unwrap_or_else(|err| panic!("profile '{profile_name}' should be valid: {err}"));
        }
    }

    #[test]
    fn test_validate_tool_model() {
        let mut config = Config::default();

        config.tools.insert(
            "opencode".to_string(),
            ToolConfig {
                name: "opencode".to_string(),
                command: "opencode".to_string(),
                models: vec!["gpt-4o".to_string()],
            },
        );

        // Valid model
        assert!(config.validate_tool_model("opencode", "gpt-4o").is_ok());

        // Invalid model
        assert!(config
            .validate_tool_model("opencode", "unsupported")
            .is_err());

        // Unknown tool
        assert!(config.validate_tool_model("unknown", "gpt-4o").is_err());
    }
}
