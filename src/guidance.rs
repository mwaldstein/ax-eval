pub struct GuidanceTopic {
    pub slug: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub related: &'static [&'static str],
    pub body: &'static str,
}

const TOPICS: &[GuidanceTopic] = &[
    GuidanceTopic {
        slug: "start",
        title: "Start Here",
        summary: "A short capsule index for the highest-priority guidance topics.",
        related: &[
            "workflow-commands",
            "structured-output",
            "typed-errors",
            "agent-instructions",
            "mcp-server-design",
            "test-usage",
            "scenario-authoring",
        ],
        body: r#"# Start Here

Use this as the first page when authoring or evaluating a tool for LLM agents.

- `workflow-commands`: design task-level commands that match real user goals.
- `structured-output`: expose JSON for state, search, status, and export.
- `typed-errors`: make failures recoverable with stable codes, retryability, and next actions.
- `agent-instructions`: put the happy path, constraints, and recovery table in AGENTS.md or CLAUDE.md.
- `mcp-server-design`: if the tool is an MCP server, design tools, descriptions, and errors for agents.
- `test-usage`: give agents realistic goals and measure whether they infer the tool's role and use it well.
- `scenario-authoring`: evaluate outcomes, then use interaction metrics to improve tool ergonomics.

If a run already exists, start with `evaluation-signals` to interpret the profile before changing the tool or docs.
"#,
    },
    GuidanceTopic {
        slug: "cli-design",
        title: "LLM-First CLI Design",
        summary: "Keep commands stable, explicit, scriptable, and non-interactive.",
        related: &[
            "stdout-stderr",
            "structured-output",
            "typed-errors",
            "workflow-commands",
        ],
        body: r#"# LLM-First CLI Design

Treat the CLI as an agent-facing protocol. Agents recover better when every operation has a stable command, explicit arguments, predictable exit codes, bounded output, and a non-interactive mode.

Do:

- Avoid blocking prompts unless stdin is a TTY; provide flags for every required value.
- Keep output bounded with `--limit`, `--fields`, `--summary`, or pagination flags.
- Return stable identifiers that can be reused in later commands.
- Keep command and flag names aligned with user-facing task vocabulary.

Sources:
- The CLI Spec: https://clispec.dev/
- Anthropic tool guidance: https://platform.claude.com/docs/en/agents-and-tools/tool-use/define-tools
"#,
    },
    GuidanceTopic {
        slug: "help-output",
        title: "Help Output For Agents",
        summary: "Make `--help` a compact map of examples, flags, formats, and recovery paths.",
        related: &["capability-discovery", "workflow-commands", "typed-errors"],
        body: r#"# Help Output For Agents

Agents frequently consult `--help` after a failed attempt. Good help output should compress the tool's operating model into examples and constraints, not only list flags.

Include:

- One-line purpose and the main workflow.
- Copyable examples for common successful paths.
- Required setup or initialization commands.
- Output format options and examples of JSON shape.
- Exit code meanings and common error recovery commands.
- Terms that appear in task prompts, mapped to command names.

Prefer short examples that can be executed as written. If the help text is long, add topic-specific help such as `mytool help export` or `mytool help workflows`.

Sources:
- The CLI Spec: https://clispec.dev/
- OpenAI function calling guide: https://developers.openai.com/api/docs/guides/function-calling
"#,
    },
    GuidanceTopic {
        slug: "stdout-stderr",
        title: "Stdout And Stderr",
        summary: "Keep machine data on stdout and diagnostics on stderr.",
        related: &["structured-output", "cli-design", "typed-errors"],
        body: r#"# Stdout And Stderr

Agents and scripts need to know which stream is safe to parse. Put data on stdout. Put progress, warnings, and diagnostics on stderr.

Do:

- Emit only the requested data format on stdout in JSON mode.
- Send spinners, logs, warnings, and retry messages to stderr.
- Exit non-zero on failure and keep error details parseable when possible.
- Avoid mixing banners, update notices, or analytics prompts into command output.

This is a small implementation detail with a large agent-experience payoff: it lets the agent pipe output into validators, JSONPath gates, and follow-up commands without brittle text cleanup.

Sources:
- The CLI Spec: https://clispec.dev/
"#,
    },
    GuidanceTopic {
        slug: "structured-output",
        title: "Structured Output",
        summary: "Expose JSON output for state, search, status, and export commands.",
        related: &[
            "stdout-stderr",
            "capability-discovery",
            "state-inspection",
            "scenario-authoring",
        ],
        body: r#"# Structured Output

Agents are strongest when they can inspect state rather than parse prose. Any command that lists, searches, exports, or reports status should have a structured output mode.

Recommended commands and flags:

- `status --output json`
- `list --output json`
- `show <id> --output json`
- `export --output json`
- `schema` or `help --json` for command and output discovery

The JSON should be stable, compact, and semantic. Include identifiers, names, statuses, timestamps, and next-action fields when useful.

Sources:
- The CLI Spec: https://clispec.dev/
- OpenAI function calling guide: https://developers.openai.com/api/docs/guides/function-calling
"#,
    },
    GuidanceTopic {
        slug: "typed-errors",
        title: "Typed Errors",
        summary: "Return errors with stable codes, retryability, and next actions.",
        related: &["stdout-stderr", "recovery-guidance", "bounded-autonomy"],
        body: r#"# Typed Errors

Agents recover poorly from vague failures. Error output should say what failed, why it failed, whether retrying is useful, and what command to run next.

Include:

- Stable error code or kind.
- Human message.
- Retryability: `retryable`, `not_retryable`, or `needs_user_action`.
- Suggested next command when there is an obvious recovery.
- Relevant identifiers and field names.

In JSON error mode, keep the shape stable. In text mode, still include the same facts.

Sources:
- Anthropic tool guidance: https://platform.claude.com/docs/en/agents-and-tools/tool-use/define-tools
- Agent Experience Design: https://axd.md/
"#,
    },
    GuidanceTopic {
        slug: "recovery-guidance",
        title: "Recovery Guidance",
        summary: "Document common failures and the exact corrective commands.",
        related: &["typed-errors", "agent-instructions", "help-output"],
        body: r#"# Recovery Guidance

Recovery guidance belongs in both error messages and agent instruction files. The agent should not have to infer the fix from a stack trace or a generic usage line.

For each common failure, document:

- Symptom or error code.
- Cause.
- Corrective command.
- Whether the command is safe to retry.
- How to verify recovery.

This is especially important for initialization, missing auth, duplicate state, invalid identifiers, stale caches, and partial writes.

Sources:
- Agent Experience Design: https://axd.md/
- Effect Solutions setup guidance pattern: https://github.com/kitlangton/effect-solutions
"#,
    },
    GuidanceTopic {
        slug: "agent-instructions",
        title: "Agent Instructions",
        summary: "Author AGENTS.md/CLAUDE.md as operational guidance, not marketing copy.",
        related: &["workflow-commands", "recovery-guidance", "capability-discovery"],
        body: r#"# Agent Instructions

Agent instruction files such as `AGENTS.md` and `CLAUDE.md` should tell the agent how to succeed with the tool inside this repo. Keep them concrete, command-oriented, and easy to skim.

Include:

- Required first command, such as initialization or health checks.
- The happy-path workflow for common tasks.
- Command examples with placeholders clearly marked.
- Constraints that are easy to violate, such as ID capture or ordering rules.
- Known failure messages and the exact recovery action.
- Where generated state is stored and how to inspect it.

Avoid vague instructions like "use the CLI as needed." A useful file lets the agent complete the task without exploratory trial and error.

Sources:
- Effect Solutions agent-guided setup pattern: https://github.com/kitlangton/effect-solutions
- Agent-Friendly Documentation Spec: https://www.agentdocsspec.com/spec/
"#,
    },
    GuidanceTopic {
        slug: "capability-discovery",
        title: "Capability Discovery",
        summary: "Give agents a small map of available commands, schemas, and docs.",
        related: &[
            "help-output",
            "structured-output",
            "agent-instructions",
            "mcp-tool-descriptions",
        ],
        body: r#"# Capability Discovery

Agents need a cheap way to discover what the tool can do before spending turns on trial and error.

Useful surfaces:

- `help --json` for command metadata.
- `schema` for data shapes.
- `topics`, `docs`, or `examples` commands for local docs.
- `llms.txt` or a compact agent-entrypoint doc for web documentation.
- MCP resources or tools when the product exposes an MCP server.

Discovery output should be bounded and task-oriented. A list of every internal operation is less useful than a short index of supported workflows and where to get details.

Sources:
- Agent Experience Design: https://axd.md/
- Model Context Protocol introduction: https://modelcontextprotocol.io/docs/getting-started/intro
"#,
    },
    GuidanceTopic {
        slug: "workflow-commands",
        title: "Workflow Commands",
        summary: "Prefer task-level commands over endpoint-shaped command collections.",
        related: &["cli-design", "agent-instructions", "state-inspection"],
        body: r#"# Workflow Commands

Agents perform better when tools match user workflows. A thin CLI wrapper around every internal endpoint can force the agent to plan too much and hold too much state in context.

Prefer commands that complete meaningful steps:

- `project init --name X` instead of requiring several setup subcommands.
- `export summary --output json` instead of a chain of list/show/format commands.
- `repair --from-error <code>` when recovery is a known workflow.

Keep lower-level commands available when useful, but document the recommended workflow command first.

Sources:
- Anthropic, Writing tools for agents: https://www.anthropic.com/engineering/writing-tools-for-agents
"#,
    },
    GuidanceTopic {
        slug: "state-inspection",
        title: "State Inspection",
        summary: "Expose status, list, show, search, and export commands for agent feedback loops.",
        related: &["structured-output", "workflow-commands", "long-running-work"],
        body: r#"# State Inspection

Agents need to observe the current state after mutations. Silent success is hard to use because the agent must guess what changed.

Provide:

- `status` for overall state and readiness.
- `list` and `show <id>` for created resources.
- `search` when names or text are user-facing.
- `export --output json` for scenario gates and external validation.
- Mutation responses that include the resulting object or stable ID.

This lets agents verify work without scraping prose or rerunning expensive workflows.

Sources:
- Agent Experience Design: https://axd.md/
- OpenAI function calling guide: https://developers.openai.com/api/docs/guides/function-calling
"#,
    },
    GuidanceTopic {
        slug: "bounded-autonomy",
        title: "Bounded Autonomy",
        summary: "Mark operations as read-only, mutating, destructive, reversible, or confirmation-required.",
        related: &["typed-errors", "agent-auth", "cli-design"],
        body: r#"# Bounded Autonomy

Agents need to know which actions they can take independently and which require user confirmation.

Make risk visible:

- Mark read-only commands.
- Mark mutating commands.
- Mark destructive commands and provide `--dry-run`.
- Explain reversibility and undo commands.
- Use scoped credentials and clear permission errors.
- In machine-readable metadata, include safety annotations when available.

This reduces both over-cautious agents that ask unnecessary questions and over-eager agents that mutate state without enough context.

Sources:
- Agent Experience Design: https://axd.md/
- Model Context Protocol tool annotations: https://modelcontextprotocol.io/specification/2025-06-18/server/tools
"#,
    },
    GuidanceTopic {
        slug: "long-running-work",
        title: "Long-Running Work",
        summary: "Support job IDs, progress, status checks, idempotency, and resume.",
        related: &["state-inspection", "structured-output", "typed-errors"],
        body: r#"# Long-Running Work

Agents should not have to hold a terminal open indefinitely or guess whether a slow command is stuck.

For long-running operations, provide:

- Immediate job ID.
- `status <job-id> --output json`.
- Progress and terminal states.
- Idempotency keys or safe retry semantics.
- Resume or cleanup commands for interrupted work.
- Clear timeout behavior and retry guidance.

This also makes scenarios more reliable because tests can poll explicit state instead of sleeping.

Sources:
- Agent Experience Design: https://axd.md/
"#,
    },
    GuidanceTopic {
        slug: "agent-auth",
        title: "Agent Authentication",
        summary: "Support non-browser auth, scoped tokens, and clear permission failures.",
        related: &[
            "bounded-autonomy",
            "typed-errors",
            "agent-instructions",
            "mcp-auth",
        ],
        body: r#"# Agent Authentication

Agents often run in terminals, CI, sandboxes, or remote machines where browser login is unavailable.

Provide:

- Non-interactive token configuration through environment variables or config files.
- A `whoami` or `auth status --output json` command.
- Scoped credentials for read vs write operations.
- Permission errors that name the missing scope and how to fix it.
- Redaction guidance for logs and transcripts.

Auth failures should be quick to diagnose and safe to share in evaluation artifacts.

Sources:
- Agent Experience Design: https://axd.md/
"#,
    },
    GuidanceTopic {
        slug: "mcp-server-design",
        title: "MCP Server Design",
        summary: "Design MCP tools around agent goals: few well-named tools, bounded output, predictable side effects.",
        related: &[
            "mcp-tool-descriptions",
            "mcp-errors",
            "mcp-auth",
            "cli-design",
            "workflow-commands",
        ],
        body: r#"# MCP Server Design

An MCP server is an agent-facing protocol like a CLI, but the agent calls typed tools over a transport instead of shelling out. The same instinct applies — design for goals, not for your internal API shape.

Do:

- Prefer a small set of task-level tools over one tool per internal endpoint. Granularity should match what a user is trying to accomplish, not your database schema.
- Name each tool for the action and object in the agent's vocabulary. The name is often the first thing the agent sees when choosing a tool.
- Return structured content and stable identifiers the agent can reuse in later calls.
- Bound output. Context is scarce; support paging or limits rather than returning everything.
- Make side effects predictable and annotate them (read-only vs destructive/idempotent) where the host supports hints, so agents and hosts can reason about safety.
- Treat the input schema as a contract: mark required vs optional explicitly and validate.

Choosing MCP vs a CLI is itself a design decision: MCP fits when the host manages connection and discovery and you want structured, typed calls; a CLI fits when shell composition and ad-hoc scripting matter. ax-eval can evaluate the same task against both to compare.

Sources:
- Model Context Protocol introduction: https://modelcontextprotocol.io/docs/getting-started/intro
- Anthropic tool guidance: https://platform.claude.com/docs/en/agents-and-tools/tool-use/define-tools
- Agent Experience Design: https://axd.md/
"#,
    },
    GuidanceTopic {
        slug: "mcp-tool-descriptions",
        title: "MCP Tool Descriptions",
        summary: "Tool names, descriptions, and input schemas are the docs the agent reads — write them for agents.",
        related: &[
            "mcp-server-design",
            "capability-discovery",
            "agent-instructions",
            "help-output",
        ],
        body: r#"# MCP Tool Descriptions

For a CLI, help text and error messages document usage. For an MCP server, the tool names, descriptions, and JSON input schemas the server advertises through `tools/list` are that documentation — the host injects them into the agent's context automatically. That authored metadata is the usage surface under evaluation.

Write it for the agent:

- Make each description answer when the agent should reach for this tool, and when it should not.
- Put preconditions, sequencing, and units in the description or schema, not in an external doc the agent may never read.
- Use per-field schema descriptions and enums. Mark required vs optional explicitly and give an example where a format is non-obvious.
- Use tool annotations or hints (read-only, destructive, idempotent) where the host supports them.
- Use server-level instructions for cross-tool workflow that no single tool owns.

If an agent needs an external document to learn that a tool exists or how to call it, that is a gap in the descriptions, not a reason to write more external docs.

Sources:
- Model Context Protocol introduction: https://modelcontextprotocol.io/docs/getting-started/intro
- Agent Experience Design: https://axd.md/
"#,
    },
    GuidanceTopic {
        slug: "mcp-errors",
        title: "MCP Tool Errors",
        summary: "Signal failure with structured, actionable tool results — MCP has no exit codes.",
        related: &["typed-errors", "mcp-server-design", "recovery-guidance"],
        body: r#"# MCP Tool Errors

MCP has no exit codes. A tool call fails either by returning a result flagged `isError` or by a protocol-level error, and the error content is what the agent reads to recover. Apply the same discipline as typed CLI errors.

Do:

- Flag failures with `isError`. Do not return a success result whose content merely describes an error in prose the agent must interpret.
- Make error content actionable: what went wrong, which argument or precondition caused it, and the next action to take.
- Distinguish user-recoverable errors (bad argument, missing precondition) from transient ones, and say which are worth retrying.
- Keep error content bounded and free of secrets. Evaluation artifacts may capture it.

ax-eval maps `isError` to a failed action in the interaction profile, so honest error signaling is what makes error-rate and retry metrics meaningful for an MCP target.

Sources:
- Model Context Protocol introduction: https://modelcontextprotocol.io/docs/getting-started/intro
- Agent Experience Design: https://axd.md/
"#,
    },
    GuidanceTopic {
        slug: "mcp-auth",
        title: "MCP Server Authentication",
        summary: "Let agents authenticate without a browser; evaluate protected servers via env credentials or a host session.",
        related: &["agent-auth", "mcp-server-design", "mcp-errors", "scenario-authoring"],
        body: r#"# MCP Server Authentication

The MCP authorization spec puts auth on HTTP transports (stdio servers take
credentials from their process environment). The OAuth flow — discovery,
dynamic client registration, PKCE, token refresh — is the client's job; in an
evaluation the client is the agent's host, not the harness.

If you author a protected server, make it usable by non-interactive agents:

- Offer a static-credential path (personal access token or API key via a
  header) alongside OAuth. Terminals, CI, and sandboxes cannot run a browser
  login, so an OAuth-only server is unevaluable and unusable there.
- If you support OAuth, implement RFC 9728 protected-resource-metadata
  discovery and dynamic client registration so hosts self-register; issue
  short-lived tokens with refresh; scope read vs write.
- Return `401` with `WWW-Authenticate`; make `403` scope errors name the
  missing scope. Keep tokens out of server logs.

To evaluate a protected server with ax-eval (see docs/mcp-auth.md, ADR-0006):

- Declare `target.auth`: `bearer_env` (token from a named env var) or `headers`
  for a static credential; `host_session` to reuse a token the host cached from
  an out-of-band login. ax-eval renders the credential config; it never runs
  OAuth itself.
- Scenario YAML names environment variables, never literal secrets. Resolved
  secrets are read at preflight and kept out of results artifacts.
- Pick the mode by what the server issues: a PAT/API key → `bearer_env`; pure
  OAuth → `host_session` after logging in once with the host's own command.

Sources:
- MCP authorization specification: https://modelcontextprotocol.io/specification/2025-06-18/basic/authorization
- Agent Experience Design: https://axd.md/
"#,
    },
    GuidanceTopic {
        slug: "scenario-authoring",
        title: "Scenario Authoring",
        summary: "Write scenarios that evaluate outcomes and expose interaction quality.",
        related: &[
            "test-usage",
            "structured-output",
            "state-inspection",
            "evaluation-signals",
            "mcp-server-design",
        ],
        body: r#"# Scenario Authoring

An LLM-tool scenario should describe a realistic user goal and verify the resulting state. It should not overfit to the exact sequence of commands an agent happens to run.

Good scenarios:

- Use outcome gates for required files, records, exports, or statuses.
- Use script gates for domain-specific assertions that generic gates cannot express.
- Configure `target.command_pattern` when transcript fallback needs help identifying target-tool commands.
- Keep the task prompt goal-based and avoid embedding every command in the prompt.
- Add rich fixture guidance when the experiment is about tool capability rather than documentation discovery.
- Compare minimal vs rich guidance when the experiment is about documentation quality.

For MCP targets, a few instincts change: fixture guidance describes when and why to use each tool rather than call syntax (the server advertises its own descriptions); declare only the tools the task needs; verify server state with a script gate and probe since it is usually not workspace files; and prefer read-only or fixture-backed servers, because ax-eval cannot undo writes a remote server makes outside the fixture. For a protected server, declare `target.auth` (see `mcp-auth`) and reference credentials by environment-variable name, never as literals. See docs/scenarios.md, "Authoring MCP scenarios".

Use gates for catastrophic failures. Use the interaction profile to understand whether the tool's role, workflow, and state model were legible to the agent.

Sources:
- ax-eval evaluation spec: docs/evaluation.md
- ax-eval guidance examples: docs/guidance-testing.md
"#,
    },
    GuidanceTopic {
        slug: "test-usage",
        title: "Test Usage Quality",
        summary: "Use goal-based tasks to evaluate whether agents understand the tool's role and can use it well.",
        related: &[
            "scenario-authoring",
            "evaluation-signals",
            "agent-instructions",
            "workflow-commands",
            "mcp-tool-descriptions",
        ],
        body: r#"# Test Usage Quality

Use ax-eval to evaluate how well an agent can discover, understand, and use a tool. The scenario should usually give the agent a goal, not a command recipe.

Goal-based prompts stress whether the agent understands the role the tool serves. Prescriptive prompts mostly test whether the agent can follow mechanics already supplied by the test author. This matters even more for an MCP target: naming the tools and call order in the prompt bypasses the very thing under test — whether the server's own tool descriptions lead the agent to the right tool.

Prefer:

- "Organize these notes into a project summary and export the result."
- "Find stale tasks, close the ones that are done, and report what remains."
- "Prepare a release checklist from this repository state."

Avoid:

- "Run `mytool init`, then `mytool add`, then `mytool export`."
- "Use exactly these flags unless the purpose of the scenario is flag handling."
- "Assert that the agent ran command X" when the real question is whether the tool solved the user's goal.

Good usage tests ask:

- Did the agent infer the tool's role from the goal, docs, and CLI surface?
- Did it find an appropriate workflow without excessive help-seeking?
- Did examples and AGENTS.md guidance reduce wrong turns?
- Did error messages help the agent recover?
- Did structured output let the agent inspect state without parsing prose?
- Did the agent complete the task with a reasonable number of commands?
- Did richer guidance improve the interaction profile across repeated runs?

Use gates as fail-fast guardrails for catastrophic correctness failures. Use interaction metrics, transcripts, evaluator scripts, and judge rubrics to understand usage quality and discoverability.

Avoid testing "the agent ran command X" unless exact command use is the behavior under evaluation. Prefer goal prompts, outcome gates, and profile interpretation.

Sources:
- ax-eval evaluation spec: docs/evaluation.md
- ax-eval guidance examples: docs/guidance-testing.md
"#,
    },
    GuidanceTopic {
        slug: "evaluation-signals",
        title: "Evaluation Signals",
        summary: "Translate interaction metrics into concrete tool and documentation improvements.",
        related: &[
            "help-output",
            "typed-errors",
            "workflow-commands",
            "test-usage",
            "scenario-authoring",
        ],
        body: r#"# Evaluation Signals

The evaluation profile is a feedback loop for tool authors.

Common interpretations:

- High help-seeking: put the workflow in AGENTS.md and top-level help.
- High retry count: improve typed errors, recovery guidance, and idempotency.
- High error count: validate inputs earlier and show corrective examples.
- Low first-try success: the command surface or docs are misleading.
- High command count with passing gates: the workflow may need a task-level command.
- No target commands in a completed run: the scenario may not identify the target tool, or the task/guidance let the agent bypass it.

Treat gates as safety checks. Treat metrics as authoring feedback.

Sources:
- ax-eval evaluation spec: docs/evaluation.md
- Anthropic tool guidance: https://platform.claude.com/docs/en/agents-and-tools/tool-use/define-tools
"#,
    },
];

pub fn render_topic_list() -> String {
    let mut output = String::from("Available guidance topics:\n");
    for topic in TOPICS {
        output.push_str(&format!(
            "  {} - {}: {}\n    Related: {}\n",
            topic.slug,
            topic.title,
            topic.summary,
            topic.related.join(", ")
        ));
    }
    output
}

pub fn render_topics(slugs: &[String]) -> anyhow::Result<String> {
    let mut output = String::new();
    for slug in slugs {
        let topic =
            find_topic(slug).ok_or_else(|| anyhow::anyhow!("Unknown guidance topic: {slug}"))?;
        if !output.is_empty() {
            output.push_str("\n---\n\n");
        }
        output.push_str(topic.body.trim());
        output.push_str(&format!(
            "\n\nRelated topics: {}\n",
            topic.related.join(", ")
        ));
    }
    Ok(output)
}

fn find_topic(slug: &str) -> Option<&'static GuidanceTopic> {
    TOPICS.iter().find(|topic| topic.slug == slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_topics() {
        let output = render_topic_list();
        assert!(output.contains("cli-design"));
        assert!(output.contains("test-usage"));
        assert!(output.contains("typed-errors"));
        assert!(output.contains("mcp-server-design"));
        assert!(output.contains("mcp-tool-descriptions"));
        assert!(output.contains("mcp-errors"));
        assert!(output.contains("mcp-auth"));
        assert!(output.contains("Related:"));
    }

    #[test]
    fn unknown_topic_errors() {
        let err = render_topics(&["missing".to_string()]).unwrap_err();
        assert!(err.to_string().contains("Unknown guidance topic"));
    }

    #[test]
    fn every_related_topic_exists() {
        for topic in TOPICS {
            for related in topic.related {
                assert!(
                    find_topic(related).is_some(),
                    "{} references missing related topic {}",
                    topic.slug,
                    related
                );
            }
        }
    }
}
