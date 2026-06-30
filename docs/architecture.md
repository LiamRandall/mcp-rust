# Architecture

Two build targets share one authoring model:

1. **The conformance reference server** (`crates/mcp-server-v1`) — the full MCP
   `2025-11-25` server, the thing the test suite gates on.
2. **API-bridging tool servers** (the examples) — small servers that wrap a REST
   API, built from `#[tool]` files + the shared `mcp-api-server` glue.

Both produce a single WASI 0.2 component and both let you author tools the same
way. The deep DESIGN/DECISIONS rationale is in
[DESIGN.md](../DESIGN.md) / [DECISIONS.md](../DECISIONS.md); this is the map.

## Crates

```
crates/
  mcp-derive/      #[tool] proc-macro: arg types + doc comments -> JSON Schema
                   + a typed dispatcher. Build-time only.
  mcp-core/        ToolHandle / ToolError / Json, and http::* — a pluggable
                   Backend (config + fetch) that tool bodies call.
  mcp-api-core/    PURE, host-tested MCP router for tool servers
                   (initialize / ping / tools-list / tools-call -> JSON). No wasm.
  mcp-api-server/  Shared WASI glue for tool servers: HTTP entrypoint, body I/O,
                   outbound wasi:http, env/wasi:config backend, and the serve!
                   macro. The examples depend on this.
  mcp-server-v1/   The conformance reference server (self-contained component:
                   protocol + SSE + sampling/elicitation + fixtures).
generator/         OpenAPI 3.x -> #[tool] files (host binary).
examples/          hello-world, petstore, fred (each a cdylib component).
templates/         wash-new templates (v1 ready; v2 deferred).
```

## How a tool-server request flows

```
MCP client ──HTTP POST──▶ mcp-api-server (wasm: read body, install http backend)
                               │
                               ▼
                         mcp-api-core::handle(body, info, tools)   ← pure, tested
                               │  tools/call
                               ▼
                         ToolHandle.call(args)   ← #[tool]-generated dispatcher
                               │  http::get/post…
                               ▼
                         mcp-core::http Backend ──▶ wasi:http/outgoing-handler ──▶ upstream API
```

The split is deliberate: **all the testable logic is pure** (`mcp-api-core`,
`mcp-derive`, `mcp-core`) and unit-tested on the host; only the thin wasm I/O
lives in `mcp-api-server`. An example is therefore just tool files + one
`serve!` call.

## Why two server flavors?

The conformance **active suite is a full reference server** (resources, prompts,
completion, sampling/elicitation over SSE, …), not "tools-only". Implementing all
of that — including server→client requests correlated across HTTP requests via
`wasi:keyvalue` — is far simpler in one self-contained component, so
`mcp-server-v1` is single-purpose and owns the green gate.

Real API bridges don't need any of that surface; they need lifecycle + `tools/*`
over plain `application/json` (which every MCP client accepts). So the examples
use the lighter `mcp-api-core` + `mcp-api-server` path. Both reuse `#[tool]` /
`mcp-core`, so tools are portable between them. See
[DECISIONS.md](../DECISIONS.md) D6/D7 for the full reasoning.

## Platform contract

Every component imports only standard WASI 0.2 interfaces, satisfied by built-in
host plugins (`wasi:http`, `wasi:config`/env, `wasi:keyvalue`, `wasi:clocks`,
`wasi:random`). **Zero capability providers, no `wasmcloud:*` / wRPC.** CI's
`validate` step fails the build if a `wasmcloud:*` import ever appears.
