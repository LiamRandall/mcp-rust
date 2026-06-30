# Getting started

This project builds **MCP servers** as single **WebAssembly components** for
wasmCloud v2. You write tools in Rust; the framework handles the MCP protocol.

## 1. Install the toolchain

- **Rust** with the WASI component target:
  ```sh
  rustup target add wasm32-wasip2
  ```
- **wasmtime** 39+ — run components locally (`wasmtime serve`).
- **wasm-tools** 1.243+ — validate/inspect components.
- **Node** 20+ — only to run the conformance suite (`npx`), not a build dep.
- Optional: **wash** 2.0+ (wasmCloud inner loop / deploy), **wkg** (fetch WIT
  deps), **wac**/**binaryen** (composition/size follow-ups).

Verify everything:

```sh
make doctor
```

## 2. Run an example in 2 minutes

```sh
cd examples/hello-world
cargo build --release --target wasm32-wasip2
wasmtime serve -Scli -Shttp target/wasm32-wasip2/release/hello_world.wasm --addr 127.0.0.1:8090
```

In another terminal:

```sh
curl -s -X POST http://127.0.0.1:8090/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"greet","arguments":{"name":"world"}}}'
# {"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"Hello, world! 👋"}],"isError":false}}
```

See the full [hello-world walkthrough](../examples/hello-world/README.md), then
[`petstore`](../examples/petstore/README.md) (bridging a REST API) and
[`fred`](../examples/fred/README.md) (auth + query params).

## 3. Build your own server

The fastest start is to copy an example directory and edit its `src/tools/`.
Each tool is one file; see [authoring-tools](authoring-tools.md). If your API has
an OpenAPI spec, [generate the tools](generate-from-openapi.md).

## 4. Connect an MCP client

Every server here speaks **Streamable HTTP** and is accepted by standard MCP
clients (Claude Desktop, IDE extensions, the MCP Inspector). Point the client at
the server URL (e.g. `http://127.0.0.1:8090/`) and its tools appear.

## 5. Deploy

Publish the component and run it on wasmCloud v2 — see [deploying](deploying.md).

## Where to go next

- [Authoring tools](authoring-tools.md) — the `#[tool]` model in depth.
- [Generate from OpenAPI](generate-from-openapi.md) — bulk-start from a spec.
- [Architecture](architecture.md) — how the crates fit together.
- [Conformance](conformance.md) — the protocol test suite and the green loop.
- [Deploying](deploying.md) — wasmCloud v2 packaging and config/secrets.
- [Examples index](../EXAMPLES.md)
