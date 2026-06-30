# Authoring tools

You build an MCP server by writing **tools**. One tool = one Rust function with
a `#[tool]` attribute, in one file. You never write JSON Schema or protocol code.

## The shape of a tool

```rust
use mcp_core::{http, tool, Json, ToolError};

/// Fetch a pet by its ID.            // ← becomes the MCP tool description
#[tool]
pub fn get_pet(
    /// The pet's unique identifier.  // ← becomes the argument's schema description
    pet_id: u64,
) -> Result<Json, ToolError> {
    let base = http::config("API_BASE_URL")?;       // config from env / wasi:config
    let resp = http::get(&format!("{base}/pets/{pet_id}"))?.ok()?;
    Ok(resp.json()?)                                // upstream JSON → MCP text content
}
```

The `#[tool]` macro generates, from the signature:

- the tool **name** (the function name),
- the **description** (the function doc comment),
- the **input JSON Schema** (from the argument types + their doc comments),
- a typed **dispatcher** that deserializes the incoming arguments and calls the
  function — so the schema and the code can never drift.

It produces a `get_pet_tool() -> ToolHandle` you add to the registry.

## Argument types → JSON Schema

| Rust type | JSON Schema |
|---|---|
| `u8`…`u64`, `i8`…`i64`, `usize`, `isize` | `{"type":"integer"}` |
| `f32`, `f64` | `{"type":"number"}` |
| `bool` | `{"type":"boolean"}` |
| `String` | `{"type":"string"}` |
| `Vec<T>` | `{"type":"array","items":<T>}` |
| `Option<T>` | schema of `T`, and **not** in `required` |

Doc comments on arguments become the property `description`. All non-`Option`
arguments are required. A missing required argument, or a wrong type, becomes a
clean tool error automatically.

## Return values

- `Ok(Json)` — success. A JSON **string** becomes the text content verbatim; any
  other JSON value is pretty-printed into the text content block.
- `Err(ToolError::msg("…"))` — a tool error (`isError: true` + the message).
  **Never panic.**

## Network and config

Tool bodies reach the outside world only through `mcp_core::http`:

- `http::get(url)`, `http::delete(url)`
- `http::post(url, &json)`, `http::put(url, &json)`
- `.ok()?` turns a non-2xx response into a `ToolError`; `.json()?` / `.text()`
  read the body.
- `http::config("KEY")` reads config/secrets (env var / `wasi:config`, backed by
  a Kubernetes `ConfigMap`/`Secret` in production). If `API_TOKEN` is set it is
  auto-attached as `Authorization: Bearer …`.

Never hardcode base URLs or secrets — read them from config.

## Registering a tool

`src/tools/mod.rs` is the registry — one `pub mod` line and one entry in `all()`:

```rust
use mcp_core::ToolHandle;

pub mod get_pet;
pub mod list_pets;

pub fn all() -> Vec<ToolHandle> {
    vec![get_pet::get_pet_tool(), list_pets::list_pets_tool()]
}
```

`src/lib.rs` wires the registry into the transport with one call:

```rust
mcp_api_server::serve! {
    name: "my-api",
    version: "0.1.0",
    instructions: Some("What this server does."),
    tools: || tools::all(),
}
```

## Conventions

- One operation per file; no shared state between tools.
- Names are `snake_case`, unique, and < 64 chars.
- Add no dependencies — use `mcp_core::http` for all I/O. (The runtime dependency
  policy is in [DESIGN.md](../DESIGN.md) §2.1.)

## Testing tools

Because dispatch is plain Rust, you can unit-test a tool's handle directly:

```rust
let h = get_pet_tool();
assert_eq!(h.name, "get_pet");
let out = (h.call)(&serde_json::json!({ "pet_id": 7 })).unwrap();
```

See `crates/mcp-core/tests/tool.rs` and `crates/mcp-api-core/src/lib.rs` for the
patterns used in this repo.

## Next

- [Generate tools from an OpenAPI spec](generate-from-openapi.md)
- [Deploy your server](deploying.md)
