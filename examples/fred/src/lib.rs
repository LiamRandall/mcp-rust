//! fred — an MCP server bridging the Federal Reserve Economic Data (FRED) API.
//!
//! FRED requires a free API key. Provide it as the `FRED_API_KEY` env var /
//! wasi:config / Kubernetes Secret. Get one at
//! <https://fred.stlouisfed.org/docs/api/api_key.html>.

mod tools;

use mcp_core::{http, ToolError};

/// Base URL, overridable via `API_BASE_URL`.
fn base_url() -> String {
    http::config("API_BASE_URL").unwrap_or_else(|_| "https://api.stlouisfed.org".to_string())
}

/// Build a FRED URL with the API key and `file_type=json` appended, plus the
/// given query parameters (values are simple and URL-safe for this demo).
fn fred_url(path: &str, params: &[(&str, &str)]) -> Result<String, ToolError> {
    let key = http::config("FRED_API_KEY").map_err(|_| {
        ToolError::msg(
            "FRED_API_KEY is not set. Get a free key at \
             https://fred.stlouisfed.org/docs/api/api_key.html and set it via env/wasi:config.",
        )
    })?;
    let mut url = format!("{}{path}?api_key={key}&file_type=json", base_url());
    for (k, v) in params {
        url.push('&');
        url.push_str(k);
        url.push('=');
        url.push_str(&encode(v));
    }
    Ok(url)
}

/// Minimal percent-encoding for query values (spaces and a few reserved chars).
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

mcp_api_server::serve! {
    name: "fred",
    version: "0.1.0",
    instructions: Some(
        "Federal Reserve economic data (FRED). Requires FRED_API_KEY. Tools: \
         search_series, get_series, get_series_observations."
    ),
    tools: || tools::all(),
}
