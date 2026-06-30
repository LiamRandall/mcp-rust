# mcp-rust

Turn an OpenAPI spec into a **strictly conformant MCP server**, compiled to a
single tiny **WebAssembly component** for **CNCF wasmCloud v2** — built and
verified entirely with Wasm-native tooling (`wasm32-wasip2`, `wit-bindgen`,
`wasm-tools`, `wac`, `wasmtime`, `wash`).

See [`DESIGN.md`](DESIGN.md) for the full spec and [`DECISIONS.md`](DECISIONS.md)
for resolved design decisions.

## Status

| Target | MCP spec | Conformance |
|---|---|---|
| **v1** (`crates/mcp-server-v1`) | `2025-11-25` | ✅ **30/30 scenarios, 39/39 checks, empty baseline** |
| v2 (`templates/v2-2026-07-28`) | `2026-07-28` | ⏸ deferred — spec version not yet in any conformance release ([D9](DECISIONS.md)) |

The v1 transport is a single WASI 0.2 component (~211 KB) implementing the full
MCP `2025-11-25` server surface: lifecycle, Streamable HTTP + SSE, tools,
resources, prompts, completion, logging, progress, and server→client
**sampling**/**elicitation** — plus DNS-rebinding protection. It runs unchanged
on `wasmtime serve`, `wash dev` (wasmCloud v2 host plugins), or any conformant
WASI P2 runtime. Zero capability providers, no `wasmcloud:*` / wRPC in the call
path.

## Quick start

```sh
make doctor        # verify the toolchain (DESIGN §5.1)
make build         # cargo build (wasm32-wasip2) -> dist/server.wasm, validate, size

# Run it and prove conformance, in two terminals:
make serve                       # wasmtime serve -Scli -Shttp -Skeyvalue
make conformance                 # official suite @ 2025-11-25, empty baseline -> exit 0
```

`make dev` runs the same component under the wasmCloud-native `wash dev` loop.

## Layout

```
crates/
  mcp-server-v1/   the fixed v1 transport (single WASI 0.2 component)
    wit/           world.wit + fetched wasi deps (wkg)
    src/           lib.rs (http), proto.rs (routing), sse.rs, fixtures.rs, kv.rs
  mcp-core/        first-party runtime glue for LLM-authored tools (milestone 4)
  mcp-derive/      first-party #[tool] proc-macro (milestone 4)
generator/         OpenAPI 3.x -> src/tools/*.rs (milestone 4)
templates/         wash-new templates (v1 ready; v2 deferred)
.github/workflows/ conformance CI
```

## Conformance loop

`make build && make serve &` then `make conformance`. The suite exercises the
server against `http://127.0.0.1:8080/mcp`. The baseline
([`conformance-baseline.yml`](conformance-baseline.yml)) ships **empty** — no
permitted known-failures.
