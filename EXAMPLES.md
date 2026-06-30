# Examples

Each example is a complete, runnable MCP server compiled to a single WASI 0.2
component. They share the `#[tool]` authoring path and the `mcp-api-server`
transport glue — an example is just a few tool files plus one `serve!` call.

Build any example:

```sh
cd examples/<name>
cargo build --release --target wasm32-wasip2
# component at: target/wasm32-wasip2/release/<name>.wasm
```

…or build all of them from the repo root with `make examples`.

| Example | What it shows | Upstream API | Needs a key | Walkthrough |
|---|---|---|---|---|
| [`hello-world`](examples/hello-world/) | The smallest MCP server — one tool, no network | none | no | [README](examples/hello-world/README.md) |
| [`petstore`](examples/petstore/) | Bridging a REST API; outbound HTTP from tools | [Swagger Petstore](https://petstore3.swagger.io) | no | [README](examples/petstore/README.md) |
| [`fred`](examples/fred/) | A real public API with auth + query params | [Federal Reserve FRED](https://fred.stlouisfed.org/docs/api/fred/) | yes (free) | [README](examples/fred/README.md) |

There is also the **conformance reference server** (`crates/mcp-server-v1`) — not
an "example" but the full MCP `2025-11-25` server that the test suite gates on.
See [`docs/conformance.md`](docs/conformance.md).

## Which one should I read first?

- **New here?** Start with [`hello-world`](examples/hello-world/README.md) — it
  takes ~2 minutes and shows the whole shape.
- **Bridging your own REST API?** Read [`petstore`](examples/petstore/README.md),
  then [`docs/authoring-tools.md`](docs/authoring-tools.md) and
  [`docs/generate-from-openapi.md`](docs/generate-from-openapi.md).
- **Need auth / query params / secrets?** Read
  [`fred`](examples/fred/README.md).

## How the examples are built

```
examples/<name>/
  src/lib.rs            # base-url/auth helpers + one serve!{…} call
  src/tools/<op>.rs     # one #[tool] function per operation
  src/tools/mod.rs      # registry: one `pub mod` line + all()
  Cargo.toml            # cdylib, depends on mcp-api-server + mcp-core
```

All the wasm/transport machinery (HTTP entrypoint, JSON-RPC, outbound HTTP,
config) lives in `mcp-api-server` and `mcp-api-core`. You only write tools. See
[`docs/architecture.md`](docs/architecture.md).
