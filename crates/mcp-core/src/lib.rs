//! First-party runtime glue for LLM-authored MCP tools.
//!
//! A tool author writes one `#[tool]` function per operation and never touches
//! JSON Schema, the transport, or the WIT. This crate provides the types that
//! the [`tool`] macro targets: [`ToolHandle`], [`ToolError`], and [`Json`].
//!
//! See `AGENTS.md` in each template for the authoring rules.

pub use mcp_derive::tool;

/// MCP tool I/O is JSON; this is the working value type.
pub type Json = serde_json::Value;

/// A registered tool: its advertised metadata plus a typed dispatcher. The
/// `#[tool]` macro produces one of these via `fn <name>_tool()`.
pub struct ToolHandle {
    /// MCP tool name (the function name).
    pub name: &'static str,
    /// Tool description (the function doc comment).
    pub description: &'static str,
    /// JSON Schema for the arguments, derived from the argument types.
    pub input_schema: &'static str,
    /// Deserialize `arguments-json` into typed args and invoke the tool.
    pub call: fn(&Json) -> Result<Json, ToolError>,
}

/// A tool failure. Maps to MCP `isError: true` + the message.
#[derive(Debug, Clone)]
pub struct ToolError {
    pub message: String,
}

impl ToolError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl core::fmt::Display for ToolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ToolError {}

impl From<serde_json::Error> for ToolError {
    fn from(e: serde_json::Error) -> Self {
        ToolError::msg(e.to_string())
    }
}

/// Internal runtime support used by macro-generated code. Not a stable API.
#[doc(hidden)]
pub mod __rt {
    use crate::{Json, ToolError};

    /// Deserialize one argument by key. A missing key decodes as JSON `null`,
    /// so `Option<T>` arguments become `None` and required ones error.
    pub fn deserialize_arg<T: serde::de::DeserializeOwned>(
        args: &Json,
        key: &str,
    ) -> Result<T, ToolError> {
        let value = args.get(key).cloned().unwrap_or(Json::Null);
        serde_json::from_value(value)
            .map_err(|e| ToolError::msg(format!("argument `{key}`: {e}")))
    }
}

/// Outbound HTTP + config for tool bodies.
///
/// The wasm implementation (raw `wasi:http/outgoing-handler` + `wasi:config`,
/// per DESIGN §6 / D5) lands with the two-component authoring path (DECISIONS
/// D6). The signatures are stable now so `#[tool]` bodies compile against them.
pub mod http {
    use crate::{Json, ToolError};

    fn unwired(what: &str) -> ToolError {
        ToolError::msg(format!(
            "mcp_core::http::{what} is wired only inside the wasm tools component (see DECISIONS D6)"
        ))
    }

    /// Read a value from `wasi:config` (Kubernetes ConfigMap/Secret).
    pub fn config(_key: &str) -> Result<String, ToolError> {
        Err(unwired("config"))
    }

    /// HTTP GET, returning the response for `.json()` mapping.
    pub fn get(_url: &str) -> Result<Response, ToolError> {
        Err(unwired("get"))
    }

    /// HTTP POST with a JSON body.
    pub fn post(_url: &str, _body: &Json) -> Result<Response, ToolError> {
        Err(unwired("post"))
    }

    /// An upstream HTTP response.
    pub struct Response {
        pub status: u16,
        pub body: Vec<u8>,
    }

    impl Response {
        /// Parse the response body as JSON → MCP text content.
        pub fn json(&self) -> Result<Json, ToolError> {
            serde_json::from_slice(&self.body).map_err(Into::into)
        }
    }
}
