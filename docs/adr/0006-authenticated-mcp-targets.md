# Authenticated MCP Targets

Many real MCP servers are protected: the agent must present credentials before
it can call any tool. ax-eval must be able to evaluate agent use of those
servers, because "can an agent get useful work done against this server" is
exactly the question the tool exists to answer, and for hosted servers the
answer is inseparable from getting authenticated first.

The MCP authorization specification (2025-06-18) defines auth at the transport
level as **OAuth 2.1, for HTTP transports only**. stdio servers explicitly
**SHOULD NOT** use it and instead take credentials from their environment. The
full HTTP flow — discover the authorization server via RFC 9728 protected
resource metadata on a 401, register via RFC 7591 dynamic client registration,
run an interactive PKCE authorization-code flow in a browser, bind the token to
the server with an RFC 8707 resource indicator, then send `Authorization:
Bearer` on every request — is the responsibility of the **MCP client**. In
ax-eval's model the MCP client is the harness under test (opencode, claude-code,
codex), not ax-eval. All three already implement this flow and cache the
resulting tokens in host-global credential stores.

That division is the whole decision. **ax-eval does not implement OAuth and does
not orchestrate an interactive authorization flow.** Doing so would duplicate
machinery the harnesses own (violating the spirit of
[ADR-0001](0001-keep-adapter-normalization-with-adapters.md), which keeps
host-specific concerns in the adapter), and an interactive browser step is
fundamentally incompatible with ax-eval's headless, `AX_EVAL_ENABLED`-gated,
reproducible batch execution — a run that blocks on a human clicking "Allow" is
not a measurement.

Instead ax-eval supports the two authentication modes that *are* non-interactive
and reproducible, and declares the third an explicit non-goal:

1. **Static credential from the environment** (`bearer_env` / `headers`). The
   scenario names an environment variable; ax-eval resolves it at provision
   time and renders it into whichever credential mechanism the selected host
   understands — codex `bearer_token_env_var`, opencode/claude-code an
   `Authorization` header with automatic OAuth disabled. This covers API-key and
   personal-access-token servers and the "I already minted a bearer token" case.
   It is fully reproducible and needs no browser.
2. **Pre-authenticated host session** (`host_session`). The operator runs the
   host's own login once, out of band (`opencode mcp auth`, `codex mcp login`,
   claude-code `/mcp`); the token lands in the host's global store; ax-eval runs
   reuse it. ax-eval renders no token here — it relies on ambient host auth
   state, which is the same fixture-isolation tradeoff already accepted in
   `docs/tradeoffs.md` (global state leaks into the run). This is how OAuth
   servers get evaluated without ax-eval running OAuth.
3. **Interactive per-run OAuth (non-goal).** ax-eval will not drive the browser
   authorization flow as part of a run. If a target needs OAuth, the operator
   pre-authenticates it (mode 2). Automating the flow to mint a token *outside*
   a run is a possible future helper, not part of the evaluation path.

Two contracts follow and are non-negotiable:

- **Scenarios name secrets, never contain them.** The `auth` block references
  environment variable names; a literal token in scenario YAML is a validation
  error. Resolution happens at provision time, and resolved secrets **MUST NOT**
  be written to any results artifact — this extends the redaction sink the audit
  (`docs/audit-2026-07.md`) already calls for, and is the reason auth is not
  just "more `headers`."
- **Missing credentials fail fast, before spend.** When an `auth` mode's
  precondition is unmet — the named env var is unset, or `host_session` finds no
  cached token where the host exposes that state — ax-eval fails in preflight
  with a clear message, rather than launching a paid run that dies on the first
  tool call. Where a host cannot report auth state cheaply, ax-eval documents the
  limitation rather than guessing.

This keeps ax-eval's role narrow and honest: it renders the credential
configuration each host expects and guards the preconditions, while the harness
remains the OAuth client of record. `stdio` targets are unaffected — their
credentials continue to flow through `env`, exactly as the MCP spec directs.

The prerequisite and the schema, transport-specific rendering, secret handling,
and preflight behavior are specified in `docs/mcp-auth.md`.
