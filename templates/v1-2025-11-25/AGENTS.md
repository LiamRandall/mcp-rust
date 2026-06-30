# Building an MCP server from an OpenAPI spec

You edit ONE thing: files in `src/tools/`. Never edit `wit/`, the transport
crate (`mcp-server-v1`), or `mcp-derive`.

## Add a tool
1. Copy `src/tools/_template.rs` to `src/tools/<operation_id>.rs`.
2. Set the doc comment (becomes the tool description), the args (become the
   input schema), and the body (call the upstream API via `http::get/post/...`).
3. Add `pub mod <operation_id>;` to `src/tools/mod.rs`.

## Rules
- One operation = one file. No shared state between tools.
- Return `Ok(Json)` for success, `Err(ToolError::msg(..))` for tool errors.
  Never panic.
- Do not add dependencies. Use `mcp_core::http` for all network calls.
- Read base URLs/tokens via `http::config("KEY")` (`wasi:config`) — never
  hardcode them.
- Names must be snake_case and unique; keep them < 64 chars.
- Argument types map to JSON Schema automatically: integers/numbers/bool/
  String map to the obvious type, `Vec<T>` to an array, `Option<T>` makes the
  argument optional. Doc comments on arguments become property descriptions.

## Verify (loop until green)
    make build && make serve &
    make conformance
Read the conformance output; fix failing tools; repeat. Do not edit the
baseline — `conformance-baseline.yml` ships empty.

## Generate from OpenAPI (bulk start)
    cargo run -p generator -- path/to/openapi.json --out src/tools/
Then refine descriptions/auth per tool and run the conformance loop.
