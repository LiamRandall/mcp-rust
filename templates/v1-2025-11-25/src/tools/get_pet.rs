//! Example tool (from the Petstore OpenAPI). One operation = one file.

use mcp_core::{http, tool, Json, ToolError};

/// Fetch a pet by its ID.
#[tool]
pub fn get_pet(
    /// The pet's unique identifier.
    pet_id: u64,
) -> Result<Json, ToolError> {
    let base = http::config("API_BASE_URL")?;
    let resp = http::get(&format!("{base}/pets/{pet_id}"))?;
    Ok(resp.json()?)
}
