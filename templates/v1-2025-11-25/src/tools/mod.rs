//! Tool registry. One `pub mod` line per tool file. The transport calls
//! `all()` for `tools/list` and dispatches `tools/call` by name.

use mcp_core::ToolHandle;

pub mod get_pet;

/// Every tool advertised by this server.
pub fn all() -> Vec<ToolHandle> {
    vec![
        get_pet::get_pet_tool(),
        // Add one line per new tool file.
    ]
}
