use mcp_core::{http, tool, Json, ToolError};

use crate::base_url;

/// Find pets by status (available, pending, or sold).
#[tool]
pub fn find_pets_by_status(
    /// Status to filter by: available, pending, or sold.
    status: String,
) -> Result<Json, ToolError> {
    let url = format!("{}/pet/findByStatus?status={status}", base_url());
    let resp = http::get(&url)?.ok()?;
    Ok(resp.json()?)
}
