//! Scenario type definitions for LLM tool testing.
//!
//! This module defines all the data structures used to represent test scenarios,
//! including task definitions, evaluation gates, and tool configurations.

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

use crate::judge::Criterion;

/// A test scenario defining a complete LLM tool evaluation case.
///
/// Scenarios are loaded from YAML files and specify:
/// - A task prompt for the LLM tool
/// - Evaluation gates to verify success
/// - Optional setup commands and configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    /// Human-readable name for this scenario
    pub name: String,
    /// Detailed description of what this scenario tests
    pub description: String,
    /// Path to the template folder containing initial state
    pub template_folder: String,
    /// Configuration for the tool being evaluated
    pub target: TargetConfig,
    /// The task definition with prompt
    pub task: Task,
    /// Evaluation configuration with gates
    pub evaluation: Evaluation,
    /// Test tier level (default: 0)
    #[serde(default = "default_tier")]
    pub tier: usize,
    /// Optional tool/model matrix configuration
    #[serde(default)]
    pub tool_matrix: Option<Vec<ToolConfig>>,
    /// Optional setup commands to run before the task
    #[serde(default)]
    pub setup: Option<Setup>,
    /// Tags for categorizing scenarios
    #[serde(default)]
    pub tags: Vec<String>,
    /// Optional runtime configuration
    #[serde(default)]
    pub run: Option<RunConfig>,
    /// Optional scripts configuration for hooks and evaluators
    #[serde(default)]
    pub scripts: Option<ScriptsConfig>,
    /// Interaction evidence expectations for target-tool commands
    #[serde(default)]
    pub interaction: InteractionConfig,
}

/// Target tool configuration for a scenario.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TargetConfig {
    /// CLI binary target.
    Cli(CliTarget),
    /// MCP server target.
    Mcp(McpTarget),
}

/// CLI target configuration for a scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliTarget {
    /// Binary name for the tool under test.
    pub binary: String,
    /// Optional regex pattern for matching commands in transcripts.
    #[serde(default)]
    pub command_pattern: Option<String>,
    /// Optional command used to check tool health/availability.
    #[serde(default)]
    pub health_check: Option<String>,
    /// Optional environment variables to set when running the target.
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}

/// MCP server target configuration for a scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTarget {
    /// Server identity for evidence matching and judge prompts.
    pub name: String,
    /// Agent-agnostic connection description.
    pub transport: McpTransport,
    /// Declared tool surface for this server.
    pub tools: Vec<String>,
    /// Optional environment variables for the server process.
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    /// Optional shell command used to check server health/availability.
    #[serde(default)]
    pub health_check: Option<String>,
}

/// MCP transport configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpTransport {
    /// Local MCP server over stdin/stdout.
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    /// Streamable HTTP MCP server.
    Http {
        url: String,
        #[serde(default)]
        headers: Option<HashMap<String, String>>,
    },
}

impl TargetConfig {
    pub fn cli_target(binary: impl Into<String>) -> Self {
        Self::Cli(CliTarget {
            binary: binary.into(),
            command_pattern: None,
            health_check: None,
            env: None,
        })
    }

    pub fn cli(&self) -> Option<&CliTarget> {
        match self {
            Self::Cli(cli) => Some(cli),
            Self::Mcp(_) => None,
        }
    }

    pub fn mcp(&self) -> Option<&McpTarget> {
        match self {
            Self::Cli(_) => None,
            Self::Mcp(mcp) => Some(mcp),
        }
    }

    pub fn command_pattern(&self) -> Option<&str> {
        self.cli()
            .and_then(|target| target.command_pattern.as_deref())
    }

    pub fn health_check(&self) -> Option<&str> {
        match self {
            Self::Cli(target) => target.health_check.as_deref(),
            Self::Mcp(target) => target.health_check.as_deref(),
        }
    }

    pub fn env(&self) -> Option<&HashMap<String, String>> {
        match self {
            Self::Cli(target) => target.env.as_ref(),
            Self::Mcp(target) => target.env.as_ref(),
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Cli(target) => &target.binary,
            Self::Mcp(target) => &target.name,
        }
    }

    pub fn is_mcp(&self) -> bool {
        matches!(self, Self::Mcp(_))
    }
}

impl<'de> Deserialize<'de> for TargetConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("target must be a map"))?;

        match object.get("kind").and_then(serde_json::Value::as_str) {
            Some("cli") => serde_json::from_value::<CliTarget>(value)
                .map(Self::Cli)
                .map_err(serde::de::Error::custom),
            Some("mcp") => serde_json::from_value::<McpTarget>(value)
                .map(Self::Mcp)
                .map_err(serde::de::Error::custom),
            Some(kind) => Err(serde::de::Error::custom(unknown_target_kind_message(kind))),
            None => serde_json::from_value::<CliTarget>(value)
                .map(Self::Cli)
                .map_err(serde::de::Error::custom),
        }
    }
}

fn unknown_target_kind_message(kind: &str) -> String {
    match kind {
        "cl" | "clii" => format!("unknown target kind: {kind}, did you mean cli?"),
        "mp" | "mpc" => format!("unknown target kind: {kind}, did you mean mcp?"),
        _ => format!("unknown target kind: {kind}. Valid kinds: cli, mcp"),
    }
}

/// Runtime configuration for scenario execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunConfig {
    /// Optional timeout in seconds
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/// Setup commands to prepare the test environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setup {
    /// Shell commands to execute before running the task
    pub commands: Vec<String>,
}

fn default_tier() -> usize {
    0
}

/// Interaction evidence expectations for a scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionConfig {
    /// Whether completed runs are expected to include target-tool commands
    #[serde(default)]
    pub target_commands: TargetCommandPolicy,
}

impl Default for InteractionConfig {
    fn default() -> Self {
        Self {
            target_commands: TargetCommandPolicy::Required,
        }
    }
}

/// Policy for target-tool command evidence in completed runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetCommandPolicy {
    #[default]
    Required,
    Optional,
    Forbidden,
}

/// Configuration for a specific tool and its supported models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// Tool name (e.g., "opencode", "claude-code")
    pub tool: String,
    /// List of supported model names
    #[serde(default)]
    pub models: Vec<String>,
}

/// The task definition containing the prompt for the LLM tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// The prompt text to send to the LLM tool
    pub prompt: String,
}

/// Evaluation configuration defining how to assess task completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evaluation {
    /// List of evaluation gates that must pass
    pub gates: Vec<Gate>,
    /// Optional judge configuration for LLM-as-judge scoring
    #[serde(default)]
    pub judge: Option<JudgeConfig>,
    /// Optional composite scoring weights
    #[serde(default)]
    pub composite: Option<CompositeConfig>,
}

/// Configuration for LLM-as-judge evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeConfig {
    /// Whether judge evaluation is enabled
    pub enabled: bool,
    /// Optional CLI tool to use for judge evaluation
    #[serde(default)]
    pub tool: Option<String>,
    /// Optional path to the rubric YAML file
    #[serde(default)]
    pub rubric: Option<String>,
    /// Optional inline criteria. Used when rubric is not provided.
    #[serde(default)]
    pub criteria: Vec<Criterion>,
    /// Minimum score threshold to pass (0.0-1.0)
    pub pass_threshold: f64,
}

/// Configuration for composite scoring weights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeConfig {
    /// Weight for judge score (0.0-1.0)
    #[serde(default = "default_judge_weight")]
    pub judge_weight: f64,
    /// Deprecated migration field. Gate statistics were removed; validation
    /// rejects YAML that still configures this field with a migration hint.
    #[serde(default)]
    pub gate_weight: Option<f64>,
    /// Weight for interaction metrics (0.0-1.0)
    #[serde(default = "default_interaction_weight")]
    pub interaction_weight: f64,
}

fn default_judge_weight() -> f64 {
    0.55
}

fn default_interaction_weight() -> f64 {
    0.10
}

/// Evaluation gate types for verifying task completion.
///
/// Each gate represents a specific assertion about the resulting state
/// after the LLM tool has executed the task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Gate {
    /// Asserts a shell command succeeds
    CommandSucceeds {
        /// Shell command to execute
        command: String,
    },
    /// Asserts command stdout contains a substring
    CommandOutputContains {
        /// Shell command to execute
        command: String,
        /// Substring that must be present in stdout
        substring: String,
    },
    /// Asserts command stdout matches a regex pattern
    CommandOutputMatches {
        /// Shell command to execute
        command: String,
        /// Regex pattern that must match stdout
        pattern: String,
    },
    /// Asserts JSON output contains data matching a path assertion
    CommandJsonPath {
        /// Shell command to execute
        command: String,
        /// JSON path to evaluate
        path: String,
        /// Assertion expression to apply to resolved value
        assertion: String,
    },
    /// Asserts a file exists in the fixture directory
    FileExists {
        /// Relative path to the target file
        path: String,
    },
    /// Asserts file contents contain a substring
    FileContains {
        /// Relative path to the target file
        path: String,
        /// Substring to search for
        substring: String,
    },
    /// Asserts file contents match a regex pattern
    FileMatches {
        /// Relative path to the target file
        path: String,
        /// Regex pattern that must match file contents
        pattern: String,
    },
    /// Asserts no errors in the transcript
    NoTranscriptErrors,
    /// Asserts a script command passes and reports status
    Script {
        /// Shell command to execute
        command: String,
        /// Human-readable gate description
        description: String,
        /// Timeout in seconds (default: 30)
        #[serde(default = "default_script_timeout")]
        timeout_secs: u64,
    },
}

/// Scripts configuration for scenario execution hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptsConfig {
    /// Post-execution scripts to run after agent completes
    #[serde(default)]
    pub post: Vec<ScriptEntry>,
    /// Custom evaluator scripts for scoring
    #[serde(default)]
    pub evaluators: Vec<EvaluatorEntry>,
}

/// A script entry for post-execution hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptEntry {
    /// Shell command to execute
    pub command: String,
    /// Timeout in seconds (default: 30)
    #[serde(default = "default_script_timeout")]
    pub timeout_secs: u64,
}

/// A custom evaluator script entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorEntry {
    /// Shell command to execute
    pub command: String,
    /// Name of the evaluator for reporting
    pub name: String,
    /// Timeout in seconds (default: 60)
    #[serde(default = "default_evaluator_timeout")]
    pub timeout_secs: u64,
}

fn default_script_timeout() -> u64 {
    30
}

fn default_evaluator_timeout() -> u64 {
    60
}
