//! Copy this file to `src/tools/<operation_id>.rs`, then add
//! `pub mod <operation_id>;` to `src/tools/mod.rs`. See AGENTS.md.

use mcp_core::{http, tool, Json, ToolError};

/// One-line description of what this tool does.   // ← MCP tool description
#[tool]
pub fn operation_id(
    /// What this argument means.                   // ← property description
    example_arg: u64,
    /// An optional argument (omit from `required`).
    filter: Option<String>,
) -> Result<Json, ToolError> {
    // Base URL + auth come from wasi:config (ConfigMap/Secret) — never hardcoded.
    let base = http::config("API_BASE_URL")?;
    let _ = filter;
    let resp = http::get(&format!("{base}/things/{example_arg}"))?;
    Ok(resp.json()?) // returned JSON → MCP text content automatically
}
