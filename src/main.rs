//! LLM tool test runner
//!
//! Evaluates LLM tools against test scenarios with automatic judging.

mod adapter;
mod cli;
mod command_execution;
mod commands;
mod config;
mod eval_helpers;
#[cfg(test)]
mod eval_tests_score;
mod evaluation;
mod fixture;
#[cfg(test)]
mod fixture_tests;
mod interaction_profile;
mod judge;
mod output;
mod results;
mod run;
mod scenario;
mod script_runner;
mod session;
mod target_env;
mod transcript;
mod utils;

use clap::{CommandFactory, Parser};
use cli::Cli;
use cli::Commands;
use results::{Cache, ResultsDB};
use scenario::ToolConfig as ScenarioToolConfig;

/// Build a matrix of tool-model configurations from CLI args, profile, or scenario config
pub fn build_tool_matrix(
    cli_tool: &Option<String>,
    cli_model: &Option<String>,
    cli_profile: &Option<String>,
    config: &config::Config,
    scenario_matrix: &Option<Vec<ScenarioToolConfig>>,
) -> anyhow::Result<Vec<output::ToolModelConfig>> {
    // If profile is specified, expand from config
    if let Some(profile_name) = cli_profile {
        let matrix = config
            .build_profile_matrix(profile_name)
            .map_err(|e| anyhow::anyhow!(e))?;
        return Ok(matrix
            .into_iter()
            .map(|(tool, model)| output::ToolModelConfig { tool, model })
            .collect());
    }

    // Single tool/model mode takes precedence over scenario matrix when
    // explicitly specified via CLI
    if let Some(tool) = cli_tool {
        let model = cli_model.as_deref().unwrap_or("default");
        if let Err(e) = config.validate_tool_model(tool, model) {
            if config.get_tool(tool).is_some() {
                return Err(anyhow::anyhow!(e));
            }
        }
        return Ok(vec![output::ToolModelConfig {
            tool: tool.to_string(),
            model: model.to_string(),
        }]);
    }

    // If scenario has a tool_matrix, use it
    if let Some(scenario_matrix) = scenario_matrix {
        let mut matrix = Vec::new();
        for config in scenario_matrix {
            let models = if config.models.is_empty() {
                vec!["default".to_string()]
            } else {
                config.models.clone()
            };
            for model in models {
                matrix.push(output::ToolModelConfig {
                    tool: config.tool.clone(),
                    model,
                });
            }
        }
        return Ok(matrix);
    }

    // Default to "opencode" if no tool specified
    let tool = "opencode";

    // Validate that the tool supports the model if tool is configured
    if let Some(model) = cli_model {
        if let Err(e) = config.validate_tool_model(tool, model) {
            // Only error if the tool exists in config and doesn't support the model
            // If tool not in config, we allow any model (backwards compatibility)
            if config.get_tool(tool).is_some() {
                return Err(anyhow::anyhow!(e));
            }
        }
    }

    Ok(vec![output::ToolModelConfig {
        tool: tool.to_string(),
        model: cli_model.as_deref().unwrap_or("default").to_string(),
    }])
}

fn main() -> anyhow::Result<()> {
    if std::env::args_os().len() == 1 {
        Cli::command().print_long_help()?;
        println!();
        return Ok(());
    }

    let cli = Cli::parse();

    // Use the configured results path for cache and database
    // Resolve to absolute path to avoid issues with nested creation
    let config = crate::config::Config::load_or_default();
    let base_dir = std::path::PathBuf::from(config.get_results_path());
    let base_dir = base_dir.canonicalize().unwrap_or_else(|_| {
        // If canonicalize fails (path doesn't exist), create it and try again
        std::fs::create_dir_all(&base_dir).ok();
        base_dir.canonicalize().unwrap_or(base_dir)
    });
    let results_db = ResultsDB::new(&base_dir);
    let cache = Cache::new(&base_dir);

    match &cli.command {
        Commands::Run {
            scenario,
            all,
            tags,
            tier,
            tool,
            model,
            profile,
            dry_run,
            no_cache,
            judge_model,
            judge_tool,
            no_judge,
            timeout_secs,
        } => {
            // Safety check: only real LLM executions require explicit consent.
            if !dry_run && std::env::var("LLM_TOOL_TEST_ENABLED").as_deref() != Ok("1") {
                anyhow::bail!(
                    "Real LLM tool runs require LLM_TOOL_TEST_ENABLED=1 as an explicit safety consent.\n\
                     This prevents accidental expensive LLM API calls and arbitrary agent-driven CLI execution.\n\
                     \n\
                     To validate selection and setup without an LLM, use --dry-run.\n\
                     To run a real agent evaluation, set:\n\
                     export LLM_TOOL_TEST_ENABLED=1"
                );
            }

            // Validate that profile and single tool/model are not both specified
            if profile.is_some() && (model.is_some() || tool.is_some()) {
                anyhow::bail!(
                    "Cannot specify both --profile and --tool/--model. \
                     Use --profile for matrix runs or --tool/--model for single runs."
                );
            }

            let selection = commands::ScenarioSelection {
                scenario: scenario.clone(),
                all: *all,
                tags: tags.clone(),
                tier: *tier,
            };

            let exec_config = commands::ExecutionConfig {
                tool: tool.clone(),
                model: model.clone(),
                profile: profile.clone(),
                dry_run: *dry_run,
                no_cache: *no_cache,
                timeout_secs: *timeout_secs,
                judge_model: judge_model.clone(),
                judge_tool: judge_tool.clone(),
                no_judge: *no_judge,
            };

            let ctx = commands::ExecutionContext {
                results_db: &results_db,
                cache: &cache,
            };

            if selection.scenario.is_some() || selection.all {
                commands::handle_run_command(&selection, &exec_config, &ctx, &config)?;
            } else {
                println!("No scenario specified. Use --scenario <path> or --all");
            }
        }
        Commands::Scenarios { tags, tier } => {
            commands::handle_list_command(tags, tier, &results_db)?;
        }
        Commands::Show { name } => {
            commands::handle_show_command(name, &results_db)?;
        }
        Commands::Clean { older_than } => {
            commands::handle_clean_command(&cache, older_than, &base_dir)?;
        }
        Commands::Template { kind } => {
            commands::handle_template_command(*kind);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use output::ToolModelConfig;
    use scenario::ToolConfig as ScenarioToolConfig;

    fn assert_matrix_contains(matrix: &[ToolModelConfig], tool: &str, model: &str) {
        assert!(
            matrix.iter().any(|c| c.tool == tool && c.model == model),
            "Matrix should contain ({}, {}), got: {:?}",
            tool,
            model,
            matrix
        );
    }

    #[test]
    fn test_build_tool_matrix_single_tool_default_model() {
        let config = config::Config::default();
        let result = build_tool_matrix(&None, &None, &None, &config, &None).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tool, "opencode");
        assert_eq!(result[0].model, "default");
    }

    #[test]
    fn test_build_tool_matrix_single_tool_with_model() {
        let config = config::Config::default();
        let result = build_tool_matrix(
            &Some("opencode".to_string()),
            &Some("claude-sonnet".to_string()),
            &None,
            &config,
            &None,
        )
        .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tool, "opencode");
        assert_eq!(result[0].model, "claude-sonnet");
    }

    #[test]
    fn test_build_tool_matrix_scenario_matrix_with_models() {
        let config = config::Config::default();
        let scenario_matrix = vec![
            ScenarioToolConfig {
                tool: "opencode".to_string(),
                models: vec!["gpt-4o".to_string(), "claude-sonnet".to_string()],
            },
            ScenarioToolConfig {
                tool: "claude-code".to_string(),
                models: vec!["default".to_string()],
            },
        ];

        let result =
            build_tool_matrix(&None, &None, &None, &config, &Some(scenario_matrix)).unwrap();

        assert_eq!(result.len(), 3);
        assert_matrix_contains(&result, "opencode", "gpt-4o");
        assert_matrix_contains(&result, "opencode", "claude-sonnet");
        assert_matrix_contains(&result, "claude-code", "default");
    }

    #[test]
    fn test_build_tool_matrix_scenario_matrix_empty_models() {
        let config = config::Config::default();
        let scenario_matrix = vec![
            ScenarioToolConfig {
                tool: "opencode".to_string(),
                models: vec![],
            },
            ScenarioToolConfig {
                tool: "claude-code".to_string(),
                models: vec![],
            },
        ];

        let result =
            build_tool_matrix(&None, &None, &None, &config, &Some(scenario_matrix)).unwrap();

        assert_eq!(result.len(), 2);
        assert_matrix_contains(&result, "opencode", "default");
        assert_matrix_contains(&result, "claude-code", "default");
    }

    #[test]
    fn test_build_tool_matrix_scenario_matrix_empty() {
        let config = config::Config::default();
        let scenario_matrix: Vec<ScenarioToolConfig> = vec![];

        let result =
            build_tool_matrix(&None, &None, &None, &config, &Some(scenario_matrix)).unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_build_tool_matrix_profile() {
        let mut config = config::Config::default();

        // Add a tool
        config.tools.insert(
            "opencode".to_string(),
            config::ToolConfig {
                name: "opencode".to_string(),
                command: "opencode".to_string(),
                models: vec!["gpt-4o".to_string(), "claude-sonnet".to_string()],
            },
        );

        // Add a profile
        config.profiles.insert(
            "standard".to_string(),
            config::ProfileConfig {
                name: "standard".to_string(),
                tools: vec!["opencode".to_string()],
                models: vec!["gpt-4o".to_string()],
            },
        );

        let result =
            build_tool_matrix(&None, &None, &Some("standard".to_string()), &config, &None).unwrap();

        assert_eq!(result.len(), 1);
        assert_matrix_contains(&result, "opencode", "gpt-4o");
    }

    #[test]
    fn test_build_tool_matrix_profile_multi() {
        let mut config = config::Config::default();

        // Add tools
        config.tools.insert(
            "opencode".to_string(),
            config::ToolConfig {
                name: "opencode".to_string(),
                command: "opencode".to_string(),
                models: vec!["gpt-4o".to_string(), "claude-sonnet".to_string()],
            },
        );
        config.tools.insert(
            "claude-code".to_string(),
            config::ToolConfig {
                name: "claude-code".to_string(),
                command: "claude-code".to_string(),
                models: vec!["claude-sonnet".to_string(), "gpt-4o".to_string()],
            },
        );

        // Add a profile with multiple tools/models
        config.profiles.insert(
            "full".to_string(),
            config::ProfileConfig {
                name: "full".to_string(),
                tools: vec!["opencode".to_string(), "claude-code".to_string()],
                models: vec!["gpt-4o".to_string(), "claude-sonnet".to_string()],
            },
        );

        let result =
            build_tool_matrix(&None, &None, &Some("full".to_string()), &config, &None).unwrap();

        assert_eq!(result.len(), 4);
        assert_matrix_contains(&result, "opencode", "gpt-4o");
        assert_matrix_contains(&result, "opencode", "claude-sonnet");
        assert_matrix_contains(&result, "claude-code", "gpt-4o");
        assert_matrix_contains(&result, "claude-code", "claude-sonnet");
    }

    #[test]
    fn test_build_tool_matrix_profile_invalid_model() {
        let mut config = config::Config::default();

        // Add a tool with limited models
        config.tools.insert(
            "opencode".to_string(),
            config::ToolConfig {
                name: "opencode".to_string(),
                command: "opencode".to_string(),
                models: vec!["gpt-4o".to_string()],
            },
        );

        // Add a profile with unsupported model
        config.profiles.insert(
            "bad".to_string(),
            config::ProfileConfig {
                name: "bad".to_string(),
                tools: vec!["opencode".to_string()],
                models: vec!["unsupported-model".to_string()],
            },
        );

        let result = build_tool_matrix(&None, &None, &Some("bad".to_string()), &config, &None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not support"));
    }

    #[test]
    fn test_build_tool_matrix_validation_tool_not_in_config() {
        // When tool is not in config, we should allow any model (backwards compat)
        let config = config::Config::default();
        let result = build_tool_matrix(
            &Some("unknown-tool".to_string()),
            &Some("any-model".to_string()),
            &None,
            &config,
            &None,
        )
        .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tool, "unknown-tool");
        assert_eq!(result[0].model, "any-model");
    }
}
