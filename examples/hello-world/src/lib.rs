//! hello-world — the smallest MCP tool server. One tool, no upstream API.
//! Everything wasm/transport is provided by `mcp-api-server`.

mod tools;

mcp_api_server::serve! {
    name: "hello-world",
    version: "0.1.0",
    instructions: Some("A minimal MCP server. Call `greet` with a name."),
    tools: || tools::all(),
}
