use mcp_core::{http, tool, Json, ToolError};

use crate::fred_url;

/// Fetch metadata for a FRED economic data series by its ID
/// (e.g. "UNRATE" for the unemployment rate, "GDP", "CPIAUCSL").
#[tool]
pub fn get_series(
    /// The FRED series ID, e.g. "UNRATE".
    series_id: String,
) -> Result<Json, ToolError> {
    let url = fred_url("/fred/series", &[("series_id", &series_id)])?;
    let resp = http::get(&url)?.ok()?;
    Ok(resp.json()?)
}
