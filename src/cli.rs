use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    author,
    version,
    arg_required_else_help = true,
    about = "Evaluate how coding agents use CLI tools",
    long_about = "ax-eval runs coding agents against reproducible CLI scenarios and writes evaluation profiles.\n\nUse it to improve CLI help, docs, and AGENTS.md guidance by seeing whether agents complete the task, how many wrong turns they take, what they spend, and which artifacts changed.\n\nCommon commands:\n  ax-eval scenarios\n  ax-eval template scenario > fixtures/my_scenario.yaml\n  ax-eval validate --scenario fixtures/my_scenario.yaml\n  ax-eval guidance list\n  ax-eval guidance start\n  AX_EVAL_ENABLED=1 ax-eval discover mytool --tool opencode\n  AX_EVAL_ENABLED=1 ax-eval run --scenario my_scenario --tool opencode\n  ax-eval show <run-id>\n\nUse `ax-eval template <kind>` for copyable schema examples.",
    after_help = "Common commands:\n  ax-eval scenarios\n  ax-eval template scenario > fixtures/my_scenario.yaml\n  ax-eval template config > ax-eval-config.toml\n  ax-eval validate --scenario fixtures/my_scenario.yaml\n  ax-eval guidance start\n  AX_EVAL_ENABLED=1 ax-eval discover mytool --tool opencode\n  AX_EVAL_ENABLED=1 ax-eval run --scenario my_scenario --tool opencode\n  ax-eval show <run-id>"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose output (or set RUST_LOG for fine-grained control)
    #[arg(long, short, global = true)]
    pub verbose: bool,
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
    #[command(
        long_about = "Run one scenario, all selected scenarios, or a configured tool/model matrix.\n\nReal agent execution is disabled unless AX_EVAL_ENABLED=1 is set, because real adapters may spend LLM API credits and execute agent-driven CLI commands. Use --dry-run without that environment variable to validate scenario loading, fixture setup, cache keys, and run planning without invoking an LLM agent.\n\nArtifacts are written under ax-eval-results/ by default, including reports, transcripts, metrics, and the isolated fixture workspace.",
        after_help = "Examples:\n  AX_EVAL_ENABLED=1 ax-eval run --scenario fixtures/my_scenario.yaml --tool opencode\n  PATH=\"$PWD/target/debug:$PATH\" AX_EVAL_ENABLED=1 ax-eval run --scenario fixtures/my_scenario.yaml --tool opencode\n  AX_EVAL_ENABLED=1 ax-eval run --all --tags smoke --tier 1 --tool claude-code\n  AX_EVAL_ENABLED=1 ax-eval run --scenario fixtures/my_scenario.yaml --profile quick\n  ax-eval run --scenario fixtures/my_scenario.yaml --dry-run\n\nStart with `ax-eval template scenario` for a copyable scenario schema."
    )]
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
    /// Discover how well a target CLI describes itself to LLM agents
    #[command(
        long_about = "Run an all-in-one discovery workflow for a target executable. Discovery asks an LLM agent to understand the target command, author five complex goal-oriented scenarios, run the generated scenario batch, judge usage quality, and summarize the results.\n\nReal agent execution is disabled unless AX_EVAL_ENABLED=1 is set, because discovery may spend LLM API credits and execute agent-driven CLI commands.",
        after_help = "Example:\n  AX_EVAL_ENABLED=1 ax-eval discover mytool --tool opencode\n\nUse --discover-tool/--discover-model when the agent authoring the discovery artifacts should differ from the evaluated scenario-run agent."
    )]
    Discover {
        /// Target executable binary or command to discover
        target: String,

        /// Agent tool to evaluate in generated scenarios
        #[arg(long, default_value = "opencode")]
        tool: String,

        /// Model to evaluate in generated scenarios
        #[arg(long)]
        model: Option<String>,

        /// Agent tool used for inspect, fixture authoring, and final summary
        #[arg(long)]
        discover_tool: Option<String>,

        /// Agent model used for inspect, fixture authoring, and final summary
        #[arg(long)]
        discover_model: Option<String>,

        /// Judge model for LLM-as-judge evaluation
        #[arg(long)]
        judge_model: Option<String>,

        /// Tool to use for LLM-as-judge evaluation
        #[arg(long)]
        judge_tool: Option<String>,

        /// Maximum execution time in seconds per agent command
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
    /// Show details for a saved run
    Show {
        /// Run ID to look up
        #[arg(required = true)]
        id: String,
    },
    /// Clean cache and legacy transcript artifacts
    Clean {
        /// Clean artifacts older than duration (e.g., "30d", "7d", "1h")
        #[arg(long)]
        older_than: Option<String>,
    },
    /// Show guidance for building LLM-usable tools and docs
    Guidance {
        #[command(subcommand)]
        command: GuidanceCommand,
    },
    /// Validate scenario YAML without running
    #[command(
        long_about = "Validate one or more scenario files for schema correctness.\n\nChecks YAML syntax, required fields, gate configuration, regex compilation, and judge setup. No fixture setup, no LLM spend, no agent execution.\n\nWith --scenario, the given file is always validated regardless of whether it looks like a scenario.\nWith --all, YAML files are scanned recursively and a lightweight heuristic (at least two distinctive scenario keys: name, target, task, evaluation, or template_folder) is used to skip non-scenario files such as rubrics. This matches the discovery logic used by `run --all`.",
        after_help = "Examples:\n  ax-eval validate --scenario fixtures/my_scenario.yaml\n  ax-eval validate --all\n  ax-eval validate --scenario fixtures/my_scenario.yaml --verbose"
    )]
    Validate {
        /// Path to scenario file or name (always validated, even if not scenario-like)
        #[arg(long, short)]
        scenario: Option<String>,

        /// Validate all scenario-like YAML files in fixtures directory
        #[arg(long)]
        all: bool,
    },
    /// Print copyable scenario, config, and script templates
    Template {
        /// Template to print
        #[arg(value_enum)]
        kind: TemplateKind,
    },
}

#[derive(Subcommand)]
pub enum GuidanceCommand {
    /// List available guidance topics
    List,
    /// Show one or more guidance topics
    Show {
        /// Topic slug(s) to display
        #[arg(required = true)]
        topics: Vec<String>,
    },
    /// Show one or more topic slugs directly, e.g. `guidance start`
    #[command(external_subcommand)]
    Topics(Vec<String>),
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum TemplateKind {
    /// Scenario YAML with target, task, setup, scripts, gates, judge, and matrix fields
    Scenario,
    /// ax-eval-config.toml with supported config fields and valid profiles
    Config,
    /// Shell script gate that reports pass/fail JSON
    ScriptGate,
    /// Custom evaluator script that reports metrics, score, and summary JSON
    Evaluator,
}
