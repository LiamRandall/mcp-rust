//! petstore — an MCP server bridging the Swagger Petstore REST API.
//! Demonstrates outbound HTTP from tool bodies via `mcp_core::http`.

mod tools;

/// Upstream base URL. Override with the `API_BASE_URL` env var / wasi:config;
/// defaults to the public Swagger Petstore so the example runs out of the box.
fn base_url() -> String {
    mcp_core::http::config("API_BASE_URL")
        .unwrap_or_else(|_| "https://petstore3.swagger.io/api/v3".to_string())
}

mcp_api_server::serve! {
    name: "petstore",
    version: "0.1.0",
    instructions: Some("Query the Swagger Petstore. Tools: get_pet_by_id, find_pets_by_status, get_inventory."),
    tools: || tools::all(),
}
