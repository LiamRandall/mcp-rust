use mcp_core::{http, tool, Json, ToolError};

use crate::base_url;

/// Return pet inventory counts by status.
#[tool]
pub fn get_inventory() -> Result<Json, ToolError> {
    let url = format!("{}/store/inventory", base_url());
    let resp = http::get(&url)?.ok()?;
    Ok(resp.json()?)
}
