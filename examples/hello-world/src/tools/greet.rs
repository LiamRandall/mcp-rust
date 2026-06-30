use mcp_core::{tool, Json, ToolError};
use serde_json::json;

/// Greet someone by name.
#[tool]
pub fn greet(
    /// Who to greet.
    name: String,
) -> Result<Json, ToolError> {
    Ok(json!(format!("Hello, {name}! 👋")))
}
