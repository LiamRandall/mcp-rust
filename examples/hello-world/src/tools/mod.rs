use mcp_core::ToolHandle;

pub mod greet;

pub fn all() -> Vec<ToolHandle> {
    vec![greet::greet_tool()]
}
