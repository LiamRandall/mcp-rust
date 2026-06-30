# v2 — MCP 2026-07-28 (deferred)

Placeholder. The stateless v2 transport is **not yet implemented**: no released
MCP conformance tooling knows spec version `2026-07-28` (the suite's newest date
version is `2025-11-25`), so v2 cannot be conformance-validated today. See
[DECISIONS.md](../../DECISIONS.md) **D9**.

When a conformance release ships `2026-07-28`, fork `crates/mcp-server-v1` into
`crates/mcp-server-v2`: drop the session id / `wasi:keyvalue` session path, move
to per-request `_meta`, and keep the same tool surface unchanged (DESIGN §11
Phase 4). The `mcp-core` / `mcp-derive` authoring path is version-agnostic and
carries over as-is.
