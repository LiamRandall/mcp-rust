use mcp_core::{http, tool, Json, ToolError};

use crate::base_url;

/// Find a pet by its ID.
#[tool]
pub fn get_pet_by_id(
    /// The ID of the pet to fetch.
    pet_id: i64,
) -> Result<Json, ToolError> {
    let url = format!("{}/pet/{pet_id}", base_url());
    let resp = http::get(&url)?.ok()?;
    Ok(resp.json()?)
}
