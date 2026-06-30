use mcp_core::{http, tool, Json, ToolError};

use crate::fred_url;

/// Fetch the observations (the actual time-series data points) for a FRED
/// series, most recent first.
#[tool]
pub fn get_series_observations(
    /// The FRED series ID, e.g. "UNRATE".
    series_id: String,
    /// Maximum number of observations to return (default 10).
    limit: Option<u32>,
) -> Result<Json, ToolError> {
    let limit = limit.unwrap_or(10).to_string();
    let url = fred_url(
        "/fred/series/observations",
        &[("series_id", &series_id), ("limit", &limit), ("sort_order", "desc")],
    )?;
    let resp = http::get(&url)?.ok()?;
    Ok(resp.json()?)
}
