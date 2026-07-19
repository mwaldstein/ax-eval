# Authenticated MCP Targets

**Status: Stable for `bearer_env` and `headers`; partial for `host_session`**

## Purpose

Define how ax-eval evaluates agent use of *protected* MCP servers — those that
require credentials before any tool call — without implementing OAuth itself and
without breaking headless, reproducible execution.

This extends [mcp-targets.md](mcp-targets.md) (unauthenticated MCP support, now
Stable) and is governed by
[ADR-0006](adr/0006-authenticated-mcp-targets.md), which sets the boundary: the
**harness is the OAuth client**; ax-eval renders credential config and guards
preconditions. Read the ADR first — this document is the schema and mechanics.

---

## Background: where auth lives in MCP

The MCP authorization spec (revision 2025-06-18) places authentication at the
transport level, and the split matters for ax-eval:

| Transport | How credentials are supplied | ax-eval's involvement |
|---|---|---|
| `stdio` | From the server process environment. The spec says stdio **SHOULD NOT** use the OAuth flow. | `McpTarget.env` is rendered for the MCP child and excluded from the evaluated agent's process environment. |
| `http` | OAuth 2.1, or a static bearer/API-key header. | This document. |

For `http`, the spec-defined OAuth flow is entirely a **client** responsibility:
on a `401`, parse `WWW-Authenticate`, fetch RFC 9728 protected-resource-metadata
to find the authorization server, optionally self-register via RFC 7591 dynamic
client registration, run an interactive PKCE authorization-code flow, bind the
token to the server with an RFC 8707 `resource` indicator, and send
`Authorization: Bearer <token>` on every subsequent request. **ax-eval
implements none of this.** The harness under test does, and caches the token in
a host-global store.

### What each host already does

Confirmed against current harness docs (2026-07-18):

| Host | Interactive OAuth | Token store | Static bearer / header |
|---|---|---|---|
| opencode | Auto on `401` incl. dynamic client registration; `opencode mcp auth` to pre-login; `opencode mcp debug` reports status | `~/.local/share/opencode/mcp-auth.json` | `headers` + `"oauth": false` disables auto-OAuth; supports `{env:NAME}` interpolation |
| codex | Experimental (`experimental_use_rmcp_client` / `[features].rmcp_client = true`); `codex mcp login`; `codex mcp get` reports status | local credential store | `bearer_token_env_var` (names an env var), `http_headers` |
| claude-code | Built-in via `/mcp`; browser loopback `http://localhost:PORT/callback` (host hardcoded — breaks headless boxes); tokens auto-refresh | internal | `headers` in `.mcp.json` |

Two takeaways drive the design: (1) every host supports a **non-interactive
static-credential path**, and (2) every host can be **pre-authenticated out of
band** so a later run reuses the cached token. Those are exactly the two modes
ax-eval supports.

---

## Prerequisite: environment-variable expansion

Today `expand_target_env_value` (`src/target_env.rs`) expands only
`${AX_EVAL_FIXTURE_DIR}` and `${AX_EVAL_RESULTS_DIR}`. It does **not** expand
arbitrary environment variables, so a header value like `${GITHUB_PAT}` renders
literally — the current unauthenticated `headers` support cannot actually carry
a secret from the environment (a live bug in mcp-targets.md's own remote
example). Authenticated targets depend on fixing this.

**Decision.** Add an explicit, namespaced form `${env:NAME}` resolved against the
process environment, kept separate from the two run-directory placeholders so the
expansion surfaces stay legible and a scenario cannot accidentally interpolate a
path where a secret is meant. Bare `${NAME}` is **not** treated as an env
reference (avoids surprising expansion and collisions). An `${env:NAME}` whose
variable is unset is a provision-time error, not a silent empty string.

This is a small, self-contained change that also repairs the existing example;
it should land first.

---

## Scenario Schema

A new optional `auth` block on `McpTarget` (http transport only):

```yaml
# Static bearer token from the environment (API key / PAT / pre-minted token)
target:
  kind: mcp
  name: github
  transport:
    type: http
    url: "https://api.githubcopilot.com/mcp/"
  auth:
    type: bearer_env
    env: GITHUB_PAT            # env var NAME, never the value
  tools: [search_issues, get_file_contents]
```

```yaml
# Arbitrary static headers (e.g. non-Bearer API-key schemes)
target:
  kind: mcp
  name: search
  transport:
    type: http
    url: "https://mcp.example.com/mcp"
  auth:
    type: headers
    headers:
      X-API-Key: "${env:SEARCH_API_KEY}"
  tools: [query]
```

```yaml
# Pure-OAuth server: the operator runs the host's own login once, out of band
# (e.g. `opencode mcp auth linear`); the host caches and refreshes the token.
# ax-eval renders no credential. See "Choosing an Auth Mode".
target:
  kind: mcp
  name: linear
  transport:
    type: http
    url: "https://mcp.linear.app/mcp"
  auth:
    type: host_session
  tools: [list_issues, create_issue]
```

### `McpAuth`

| `type` | Fields | Meaning |
|---|---|---|
| `none` (default; may be omitted) | — | No credentials. Current behavior. |
| `bearer_env` | `env: string` | Bearer token read from the named env var at provision time, rendered to each host's Bearer mechanism. |
| `headers` | `headers: map<string,string>` | Static headers; values may use `${env:NAME}`. For non-Bearer schemes or multiple headers. |
| `host_session` | — | Use the credential the host cached from a prior out-of-band login. ax-eval renders no token. |

Rules:

- `auth` is valid only on `http` transports. On `stdio` it is a validation error
  ("stdio servers authenticate via `env`, not `auth`"), matching the MCP spec.
- `env` / `${env:NAME}` name environment variables. A literal token value in the
  YAML is a validation error (`looks like a secret; reference an env var
  instead`).
- `bearer_env` and `headers` are mutually exclusive framings of the same static
  path; `bearer_env` is sugar for the common Bearer case.

---

## Choosing an Auth Mode

The mode is determined by **what credential the server actually issues**, not by
whether the server "uses OAuth" — most OAuth servers also issue static tokens,
and that difference is what decides the mode.

The critical distinction: **an OAuth access token is not an env-var value.** A
static token (personal access token / API key) is a long-lived string you copy
once from the service's web UI. An OAuth access token is short-lived (often ~1
hour), obtained through an interactive browser redirect, and must be refreshed —
so it cannot live statically in an env var. `bearer_env` is for the former only.

| The server issues… | Mode | How the operator gets the credential |
|---|---|---|
| A personal access token / API key (most public servers offer this even when they also support OAuth) | `bearer_env` (or `headers` for non-Bearer schemes) | Create it once in the service's web UI (e.g. GitHub Settings → Developer settings → PAT; Linear API key; Notion integration token), then export it as the named env var. This is a manual settings step, **not** the OAuth redirect flow. |
| OAuth only (no static token), on a local/dev runner | `host_session` | Run the host's own login once, out of band: `opencode mcp auth <name>`, `codex mcp login <name>`, or claude-code `/mcp`. The browser flow runs, the **host** caches and auto-refreshes the token, and ax-eval reuses it. The operator never handles a token string. |
| OAuth only, on a headless CI runner | *(not cleanly supported)* | See the gap below. |

Guidance:

- **Reach for `bearer_env` first** when the target offers a PAT/API key — it is
  fully reproducible and needs no browser. Check the service's developer/
  integration settings; a static-token option is the common case (GitHub's
  remote MCP, Linear, Notion, Sentry, and most others provide one).
- **Use `host_session` for pure-OAuth servers.** ax-eval deliberately does not
  run OAuth (ADR-0006); `host_session` delegates the whole flow — discovery,
  consent, PKCE, refresh — to the harness, which is the OAuth client of record.
- **Do not hand-extract an OAuth access token into `bearer_env`.** It is
  technically possible (the token sits in the host's store) but wrong: it
  expires with no refresh, so the run breaks mid-session or between runs.

### The headless-CI gap

Fully automated CI against a server that is **OAuth-only with no PAT option** is
not cleanly supported today, and this is a direct, accepted consequence of
making interactive per-run OAuth a non-goal (ADR-0006):

- `bearer_env` needs a static token the server will not issue.
- `host_session` needs a *prior interactive* login plus a persistent, refreshable
  token store on the runner — and claude-code's localhost-only OAuth loopback
  makes even the one-time login awkward on a headless box.

In practice this rarely bites, because servers worth evaluating almost all offer
PATs (Case 1). When it does bite, the operator must pre-seed the host's token
store on the runner out of band; a future `mint-token` helper (Open Questions)
could smooth this, but automating the browser flow itself remains out of scope.

---

## Transport Provisioning

Extends the `provision_target` rendering table in mcp-targets.md. ax-eval renders
the declared `auth` into each host's native mechanism and **disables the host's
automatic OAuth** for the static modes, so behavior is deterministic rather than
"maybe a browser pops up".

| `auth.type` | opencode (`opencode.json`) | claude-code (`.mcp.json`) | codex (`config.toml`) |
|---|---|---|---|
| `bearer_env` | `"headers": { "Authorization": "Bearer <resolved>" }, "oauth": false` | `"headers": { "Authorization": "Bearer <resolved>" }` | `bearer_token_env_var = "<NAME>"` (codex resolves the env var itself) |
| `headers` | `"headers": { ... }, "oauth": false` | `"headers": { ... }` | `http_headers = { ... }` |
| `host_session` | entry with no headers, `oauth` left enabled — host uses its cached token | entry with no headers — host uses its cached/refreshed token | entry with no static token — host uses its credential store (requires `codex mcp login` beforehand and the rmcp client feature) |

Notes:

- **codex `bearer_env` passes the variable name, not the value** — codex reads
  the env var at its own launch, so ax-eval must ensure that variable is present
  in the codex process environment (it is, since ax-eval inherits and forwards
  the caller's environment) and must **not** inline the resolved token into
  `config.toml`. opencode and claude-code have no name-passing field, so ax-eval
  resolves the value and writes the header — which is why secret redaction of the
  written config and artifacts is mandatory (below).
- The RFC 8707 `resource` indicator and canonical server URI are derived by the
  host from the configured `url`; ax-eval does not set them. It should, however,
  render the `url` in the spec's canonical form (no trailing slash unless
  significant, no fragment) so host-side audience binding is predictable.

---

## Secret Handling

Non-negotiable, and the reason `auth` is a distinct feature rather than more
`headers`:

1. **Names in, values never.** Scenario YAML and the fixture only ever contain
   env-var *names*. Validation rejects anything that looks like a literal
   credential in an `auth` value.
2. **Resolve late.** Env vars resolve at provision time, in memory.
3. **Never persisted.** A resolved token must not appear in `run.json`,
   `events.jsonl`, transcripts, `report.md`, `evaluation.md`, or any provisioned
   config retained in the results tree. Where ax-eval writes a header value into
   a workspace config (opencode/claude-code), that file is necessary for the
   live run and lives under `results/fixture`; immediately after the agent exits,
   ax-eval scrubs the retained `.mcp.json` / `.opencode_config/opencode.json`
   copy so inspectable artifacts show `Bearer [REDACTED]` or `[REDACTED]`
   rather than the resolved value. The codex path avoids the problem by
   construction by writing only `bearer_token_env_var = "<NAME>"`.
4. **`host_session` stores nothing in ax-eval's tree at all** — the token lives
   only in the host's own store.

---

## Preflight

Money is spent the moment the agent runs, so credential problems must be caught
before that:

- `bearer_env` / `headers`: every referenced `${env:NAME}` / `env` must be set
  and non-empty. `validate` checks structure and literal-secret mistakes; the
  run path checks environment presence before `provision_target`, so an unset or
  empty variable fails before the agent launches.
- `host_session`: not preflighted in this cut. ax-eval renders no credential and
  cannot run its direct `tools/list` inspection because the credential exists
  only inside the host. It emits a warning; if the host is not already logged
  in, the run may fail on the first MCP tool call. Host status probes (`codex
  mcp get <name>`, `opencode mcp debug`) remain future work.
- A `401`/`403` observed *during* a run (token expired mid-run, insufficient
  scope) is captured as an MCP tool-call error (`is_error`) through the normal
  Stage 3/4 evidence path and surfaced in the profile; it is not retried by
  ax-eval.

---

## What ax-eval deliberately does not do

- **Implement OAuth** (discovery, DCR, PKCE, token exchange/refresh). The host
  does this.
- **Drive an interactive authorization flow inside a run.** Incompatible with
  headless batch execution (ADR-0006). Use `host_session` after logging in out
  of band.
- **Manage or store tokens.** No token cache, no refresh, no secret vault. Env
  vars and host stores only.
- **Model per-host auth exotica** beyond the three modes (e.g. claude-code
  `headersHelper` dynamic headers, codex `auth = "oauth"` chatgpt first-party
  mode). Revisit only if a concrete scenario needs one; the generic modes cover
  API-key and standard OAuth-via-host cases.

---

## Reproducibility caveats

Authenticated targets stress ax-eval's reproducibility promise (see
`strategic-review-2026-07.md`, "the instrument drifts"), and honesty requires
naming it:

- A `host_session` token can expire between runs, so a scenario that passed
  yesterday can fail today for reasons unrelated to the agent or the tool. The
  run record should capture *that* auth was used and by which mode, so a
  later-diverging result is attributable.
- Static tokens are stable but machine-local (in the operator's environment),
  so shared scenario packs (a roadmap item) can declare `auth` but never ship
  the credential — the pack declares the env-var *name* as part of its setup
  contract. Document this in the scenario-pack guidance when that work lands.

---

## Open Questions

- **Recording auth mode in results.** Should `run.json` carry `auth: {mode}`
  (never the secret) so authenticated runs are self-describing for trend
  analysis? Leaning yes; deferred to implementation once the record schema is
  touched.
- **A `mint-token` helper.** A future out-of-band `ax-eval mcp login <scenario>`
  that shells to the host's login so operators have one entry point. Explicitly
  *not* part of a run; parked until the run-path modes are proven.
- **claude-code headless OAuth.** claude-code's localhost-only loopback makes
  even out-of-band login awkward on headless CI boxes. Track upstream; for now
  `host_session` + claude-code is documented as "local dev only".

---

## In-System Documentation

Per [ADR-0003](adr/0003-keep-cli-self-documenting.md), auth behavior is kept in
lockstep across:

- `src/scenario/types.rs`: `McpAuth` enum on `McpTarget`; deserialization tests
  in `src/scenario/tests/`.
- `src/scenario/validation.rs`: http-only rule, literal-secret rejection,
  env-name presence.
- `src/target_env.rs`: `${env:NAME}` expansion + tests (the prerequisite).
- `src/adapter/{opencode,claude_code,codex}/mod.rs`: auth rendering in
  `provision_target`; golden-config tests; redaction of retained written header
  values.
- Redaction sink: resolved secrets are scrubbed from retained MCP config files
  and from transcript/report patterns.
- `docs/scenarios.md`, `docs/mcp-targets.md` (cross-link), `SCENARIO_TEMPLATE`,
  `docs/user-guide.md`, `docs/tradeoffs.md` (host_session global-state caveat),
  `CHANGELOG.md`.
- Static modes are Stable. `host_session` remains partial until host status
  preflight is proven.

---

## References

- [MCP authorization specification (2025-06-18)](https://modelcontextprotocol.io/specification/2025-06-18/basic/authorization)
- [RFC 9728 — OAuth 2.0 Protected Resource Metadata](https://datatracker.ietf.org/doc/html/rfc9728)
- [RFC 8707 — Resource Indicators for OAuth 2.0](https://www.rfc-editor.org/rfc/rfc8707.html)
- [RFC 7591 — OAuth 2.0 Dynamic Client Registration](https://datatracker.ietf.org/doc/html/rfc7591)
- [OAuth 2.1 draft](https://datatracker.ietf.org/doc/html/draft-ietf-oauth-v2-1-13)
- [opencode MCP servers](https://opencode.ai/docs/mcp-servers/)
- [Codex MCP](https://developers.openai.com/codex/mcp)
- [Claude Code MCP](https://code.claude.com/docs/en/mcp)
- [ADR-0006](adr/0006-authenticated-mcp-targets.md), [mcp-targets.md](mcp-targets.md)
