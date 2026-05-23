# ax-eval Context

## Glossary

**Adapter** — A module that launches an LLM coding-agent CLI (opencode, claude-code, codex) in an isolated workspace, captures its interaction, and normalizes the output into canonical run artifacts. Adapters do not invoke the target CLI directly; the agent does.

**Scenario** — A YAML file defining a task for an LLM agent to attempt using a specific CLI tool, the fixture environment it runs in, and the evaluation criteria applied after the agent exits. The unit of evaluation.

**Fixture** — A directory of files (AGENTS.md, README, seed data, scripts) copied into an isolated workspace before a scenario runs. Referenced by `template_folder` in the scenario YAML.

**Gate** — A deterministic, binary pass/fail assertion evaluated after the agent completes a task. Gates catch catastrophic outcome failures (file missing, command failed). They are guardrails, not the primary evaluation signal.

**Evaluation Profile** — The primary output of a run: a set of quantitative and qualitative measurements across three layers (interaction metrics, gate results, judge scores) that enable comparison across models, tools, and documentation variants.

**Interaction Metrics** — Quantitative measures derived from interaction evidence: command count, error rate, retry rate, help invocations, first-try success rate, iteration ratio, and completion status.

**Interaction Evidence** — The input used to build interaction metrics. Structured tool-call events from the adapter are the canonical source. Transcript regex analysis is fallback evidence only for adapters that cannot expose structured tool calls.

**Judge** — An optional LLM-as-judge evaluation that scores the quality of an agent's tool usage against a rubric. Produces a weighted score, rationale, confidence, issues, and highlights. Encodes the tool author's intent about how the tool should be used.

**Composite Score** — An optional single-number summary combining gate, judge, and interaction layer scores with explicit weights. Not reported by default; scenario authors must define weights.

**Discovery** — A top-level workflow (`ax-eval discover <target>`) that asks an agent to inspect a target executable, produce an understanding document, author five goal-oriented scenarios, run them, and summarize findings. Evaluates how self-describing a CLI is. See `specs/llm-user-validation.md` for architecture and `docs/user-guide.md` for usage.

**Tool Event Normalization** — The conversion from an adapter's raw execution output into canonical run artifacts: transcript text, command events, token usage, cost, and exit status. Adapter-specific parsing lives in `src/adapter/<name>/normalize.rs`; shared helpers live in `src/adapter/normalize.rs`.

**Run Status** — A human-facing triage label derived from execution completion, guardrail state, and judge threshold. Examples: `completed; judge threshold met`, `guardrail attention: 2/3 gates`, `agent did not complete`. Not an absolute pass/fail verdict.
