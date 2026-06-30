# hello-world — walkthrough

The smallest possible MCP server: one tool, no upstream API. If you read one
example, read this one — it shows the whole shape in ~2 minutes.

## What it does

Exposes a single MCP tool, `greet(name)`, that returns `Hello, <name>! 👋`.

## Prerequisites

- Rust with the `wasm32-wasip2` target: `rustup target add wasm32-wasip2`
- `wasmtime` 39+ (to run it locally)
- `curl` (to call it)

From the repo root you can check everything with `make doctor`.

## 1. Build

```sh
cd examples/hello-world
cargo build --release --target wasm32-wasip2
```

Output: `target/wasm32-wasip2/release/hello_world.wasm` (~140 KB, a standard
WASI 0.2 component).

## 2. Run

```sh
wasmtime serve -Scli -Shttp target/wasm32-wasip2/release/hello_world.wasm \
  --addr 127.0.0.1:8090
```

The server now speaks MCP Streamable HTTP at `http://127.0.0.1:8090/`.

## 3. Call it

Initialize the session:

```sh
curl -s -X POST http://127.0.0.1:8090/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}'
```

```json
{"jsonrpc":"2.0","id":1,"result":{
  "protocolVersion":"2025-11-25",
  "serverInfo":{"name":"hello-world","version":"0.1.0"},
  "capabilities":{"tools":{"listChanged":false}},
  "instructions":"A minimal MCP server. Call `greet` with a name."}}
```

List tools:

```sh
curl -s -X POST http://127.0.0.1:8090/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
```

```json
{"jsonrpc":"2.0","id":2,"result":{"tools":[
  {"name":"greet","description":"Greet someone by name.",
   "inputSchema":{"type":"object","properties":{"name":{"type":"string","description":"Who to greet."}},
                  "required":["name"],"additionalProperties":false}}]}}
```

Call the tool:

```sh
curl -s -X POST http://127.0.0.1:8090/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"greet","arguments":{"name":"Ada"}}}'
```

```json
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"Hello, Ada! 👋"}],"isError":false}}
```

## 4. Use it from an MCP client

Any MCP client that speaks Streamable HTTP can use it. For example, in Claude
Desktop's MCP settings add an HTTP server pointing at `http://127.0.0.1:8090/`.
The `greet` tool then appears in the client.

## How it's built

The entire server is three small files:

- `src/tools/greet.rs` — the tool. The doc comments become the MCP description
  and the argument schema; the `#[tool]` macro derives the JSON Schema and a
  typed dispatcher.

  ```rust
  /// Greet someone by name.
  #[tool]
  pub fn greet(
      /// Who to greet.
      name: String,
  ) -> Result<Json, ToolError> {
      Ok(json!(format!("Hello, {name}! 👋")))
  }
  ```

- `src/tools/mod.rs` — the registry: one `pub mod greet;` line and `all()`.
- `src/lib.rs` — one `serve!{…}` call wiring the registry into the transport.

Everything else (HTTP, JSON-RPC framing, the `application/json` responses) is
provided by `mcp-api-server`. You never touch it.

## Add a second tool

1. Copy `src/tools/greet.rs` to `src/tools/<name>.rs` and edit it.
2. Add `pub mod <name>;` and `<name>::<name>_tool(),` to `src/tools/mod.rs`.
3. Rebuild. That's it — see [`docs/authoring-tools.md`](../../docs/authoring-tools.md).

## Deploy

Publish the component and run it on wasmCloud v2 — see
[`docs/deploying.md`](../../docs/deploying.md).
