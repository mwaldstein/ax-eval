# Keep Adapter Normalization With Adapters

Adapter-specific raw-output parsing lives next to the adapter invocation code in
`src/adapter/<name>/normalize.rs`, while `src/adapter/normalize.rs` only holds
canonical construction helpers and cross-adapter behavior expectations. We chose
this over a single central normalizer because each LLM CLI owns a distinct raw
event schema; keeping that schema knowledge with the adapter improves locality
while preserving shared `ToolRunOutput` construction.
