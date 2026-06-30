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

## Examples

Three runnable example MCP servers (build with `make examples`, or `cd` in and
`cargo build --release --target wasm32-wasip2`). Full list and details in
[`EXAMPLES.md`](EXAMPLES.md).

| Example | What it shows | Walkthrough |
|---|---|---|
| [`hello-world`](examples/hello-world/) | smallest MCP server — one tool, no network | [walkthrough](examples/hello-world/README.md) |
| [`petstore`](examples/petstore/) | bridging a REST API; outbound HTTP from tools | [walkthrough](examples/petstore/README.md) |
| [`fred`](examples/fred/) | Federal Reserve API with auth + query params | [walkthrough](examples/fred/README.md) |

## Documentation

- [Getting started](docs/getting-started.md) — install, run an example, build your own
- [Authoring tools](docs/authoring-tools.md) — the `#[tool]` model in depth
- [Generate from OpenAPI](docs/generate-from-openapi.md) — bulk-start from a spec
- [Architecture](docs/architecture.md) — how the crates fit together
- [Conformance](docs/conformance.md) — the test suite and the green loop
- [Deploying](docs/deploying.md) — wasmCloud v2 packaging, config & secrets
- [`AGENTS.md`](AGENTS.md) — terse rules for an LLM building a server
- [`DESIGN.md`](DESIGN.md) · [`DECISIONS.md`](DECISIONS.md) — spec & resolved decisions

## Layout

```
crates/
  mcp-server-v1/   the conformance reference server (single WASI 0.2 component)
    wit/           world.wit + fetched wasi deps (wkg)
    src/           lib.rs (http), proto.rs (routing), sse.rs, fixtures.rs, kv.rs
  mcp-derive/      first-party #[tool] proc-macro (schema gen + dispatch)
  mcp-core/        ToolHandle/ToolError/Json + pluggable http backend
  mcp-api-core/    pure, host-tested MCP router for tool servers
  mcp-api-server/  shared WASI glue for tool servers (outbound http + serve! macro)
generator/         OpenAPI 3.x -> src/tools/*.rs (host binary)
examples/          hello-world, petstore, fred
templates/         wash-new templates (v1 ready; v2 deferred)
docs/              howtos (getting-started, authoring-tools, deploying, …)
.github/workflows/ conformance + unit + examples CI
```

## Conformance loop

`make build && make serve &` then `make conformance`. The suite exercises the
server against `http://127.0.0.1:8080/mcp`. The baseline
([`conformance-baseline.yml`](conformance-baseline.yml)) ships **empty** — no
permitted known-failures.
