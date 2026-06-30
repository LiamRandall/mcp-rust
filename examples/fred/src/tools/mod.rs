//! Tools for the Federal Reserve Economic Data (FRED) API.
//! <https://fred.stlouisfed.org/docs/api/fred/>

use mcp_core::ToolHandle;

pub mod get_series;
pub mod get_series_observations;
pub mod search_series;

pub fn all() -> Vec<ToolHandle> {
    vec![
        search_series::search_series_tool(),
        get_series::get_series_tool(),
        get_series_observations::get_series_observations_tool(),
    ]
}
