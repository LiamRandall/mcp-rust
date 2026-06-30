# petstore — walkthrough

An MCP server that bridges the [Swagger Petstore](https://petstore3.swagger.io)
REST API. This is the canonical "wrap an existing API as MCP tools" example —
the tool bodies make **real outbound HTTP calls**.

## What it does

Three tools, each mapping to one Petstore endpoint:

| Tool | Endpoint |
|---|---|
| `get_pet_by_id(pet_id)` | `GET /pet/{petId}` |
| `find_pets_by_status(status)` | `GET /pet/findByStatus?status=…` |
| `get_inventory()` | `GET /store/inventory` |

## Prerequisites

- Rust + `wasm32-wasip2`, `wasmtime` 39+, `curl` (see `make doctor`).
- Outbound network access (the tools call `petstore3.swagger.io` over HTTPS).

## 1. Build & run

```sh
cd examples/petstore
cargo build --release --target wasm32-wasip2
wasmtime serve -Scli -Shttp target/wasm32-wasip2/release/petstore.wasm \
  --addr 127.0.0.1:8091
```

The base URL defaults to `https://petstore3.swagger.io/api/v3`. Override it with
the `API_BASE_URL` env var (e.g. to point at your own Petstore):

```sh
wasmtime serve -Scli -Shttp --env API_BASE_URL=https://my-petstore.example.com/api/v3 \
  target/wasm32-wasip2/release/petstore.wasm --addr 127.0.0.1:8091
```

## 2. Call it

```sh
# List the tools
curl -s -X POST http://127.0.0.1:8091/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'

# Find available pets
curl -s -X POST http://127.0.0.1:8091/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"find_pets_by_status","arguments":{"status":"available"}}}'

# Fetch one pet
curl -s -X POST http://127.0.0.1:8091/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_pet_by_id","arguments":{"pet_id":1}}}'
```

A successful call returns the upstream JSON wrapped as MCP text content:

```json
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{ …pet JSON… }"}],"isError":false}}
```

> **Note:** the public Swagger Petstore demo is frequently overloaded and may
> return `HTTP 500`. That is the upstream server, not this component — the tool
> faithfully surfaces it as an error result (`isError: true`). To see clean 2xx
> responses regardless, point `API_BASE_URL` at a stable Petstore deployment, or
> at an echo service such as `https://httpbin.org/anything` to inspect the exact
> request this server makes.

## How it's built

Each tool is one file under `src/tools/`. For example
`src/tools/get_pet_by_id.rs`:

```rust
/// Find a pet by its ID.
#[tool]
pub fn get_pet_by_id(
    /// The ID of the pet to fetch.
    pet_id: i64,
) -> Result<Json, ToolError> {
    let url = format!("{}/pet/{pet_id}", base_url());
    let resp = http::get(&url)?.ok()?;   // outbound HTTPS via mcp_core::http
    Ok(resp.json()?)                     // upstream JSON -> MCP text content
}
```

`mcp_core::http::{get, post, put, delete}` perform outbound HTTP through the
host's `wasi:http/outgoing-handler` plugin. `base_url()` reads `API_BASE_URL`
from config (env / `wasi:config`) with a sensible default. No HTTP client crate,
no protocol code.

`src/lib.rs` registers the three tools with one `serve!{…}` call.

## Generate this from the OpenAPI spec

The Petstore publishes an OpenAPI document, so you can generate the tool files
instead of writing them — see
[`docs/generate-from-openapi.md`](../../docs/generate-from-openapi.md):

```sh
cargo run -p generator -- petstore-openapi.json --out examples/petstore/src/tools/
```

## Deploy

See [`docs/deploying.md`](../../docs/deploying.md). On wasmCloud, set
`API_BASE_URL` via the workload's config and list `petstore3.swagger.io` in the
outbound `allowedHosts` (deny-all by default).
