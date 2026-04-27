use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Args, Clone, Debug)]
#[group(required = false, multiple = false)]
pub struct ToolModelArgs {
    /// Tool to test (e.g., claude-code, opencode)
    #[arg(long, default_value = "opencode")]
    pub tool: String,

    /// Model to use with the tool (e.g., claude-sonnet-4-20250514, gpt-4o)
    #[arg(long)]
    pub model: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run a test scenario
    Run {
        /// Path to scenario file or name
        #[arg(long, short)]
        scenario: Option<String>,

        /// Run all scenarios in fixtures directory
        #[arg(long)]
        all: bool,

        /// Filter scenarios by tags
        #[arg(long)]
        tags: Vec<String>,

        /// Filter scenarios by tier (0=smoke, 1=quick, 2=standard, 3=comprehensive)
        #[arg(long, default_value = "0")]
        tier: usize,

        /// Tool to test (e.g., claude-code, opencode)
        #[arg(long)]
        tool: Option<String>,

        /// Model to use with the tool (e.g., claude-sonnet-4-20250514, gpt-4o)
        #[arg(long)]
        model: Option<String>,

        /// Profile to use for matrix run (defined in config)
        #[arg(long)]
        profile: Option<String>,

        /// Dry run (don't execute LLM calls)
        #[arg(long)]
        dry_run: bool,

        /// Disable caching
        #[arg(long)]
        no_cache: bool,

        /// Judge model for LLM-as-judge evaluation
        #[arg(long)]
        judge_model: Option<String>,

        /// Tool to use for LLM-as-judge evaluation (defaults to judge config or opencode)
        #[arg(long)]
        judge_tool: Option<String>,

        /// Disable LLM-as-judge evaluation
        #[arg(long)]
        no_judge: bool,

        /// Maximum execution time in seconds per command
        #[arg(long, default_value = "300")]
        timeout_secs: u64,
    },
    /// List available scenarios
    Scenarios {
        /// Filter by tags
        #[arg(long)]
        tags: Vec<String>,

        /// Filter scenarios by tier (0=smoke, 1=quick, 2=standard, 3=comprehensive)
        #[arg(long, default_value = "0")]
        tier: usize,
    },
    /// Show details of a scenario
    Show {
        /// Name of the scenario
        #[arg(required = true)]
        name: String,
    },
    /// Clean up artifacts
    Clean {
        /// Clean artifacts older than duration (e.g., "30d", "7d", "1h")
        #[arg(long)]
        older_than: Option<String>,
    },
}
