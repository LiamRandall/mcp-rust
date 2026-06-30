use mcp_core::{http, tool, Json, ToolError};

use crate::fred_url;

/// Search FRED for economic data series matching a query
/// (e.g. "unemployment", "GDP", "CPI").
#[tool]
pub fn search_series(
    /// Free-text search, e.g. "real gross domestic product".
    query: String,
) -> Result<Json, ToolError> {
    let url = fred_url("/fred/series/search", &[("search_text", &query), ("limit", "10")])?;
    let resp = http::get(&url)?.ok()?;
    Ok(resp.json()?)
}
