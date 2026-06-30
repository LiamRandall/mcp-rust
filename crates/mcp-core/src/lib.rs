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
/// Tools are plain `fn(&Json)`, so they reach the network through a process-wide
/// [`Backend`] that the host component installs once at startup via
/// [`set_backend`]. In wasm that backend is raw `wasi:http/outgoing-handler` +
/// `wasi:cli/environment` (see `mcp-api-server`); in host tests it is a mock.
pub mod http {
    use crate::{Json, ToolError};
    use std::cell::RefCell;

    /// Network + config provider for tool bodies. Implemented by the component.
    pub trait Backend {
        /// Read a config/secret value (env var / `wasi:config`).
        fn config(&self, key: &str) -> Option<String>;
        /// Perform an HTTP request. `body` is empty for GET/DELETE.
        fn fetch(
            &self,
            method: &str,
            url: &str,
            body: &[u8],
            headers: &[(String, String)],
        ) -> Result<Response, String>;
    }

    thread_local! {
        static BACKEND: RefCell<Option<Box<dyn Backend>>> = const { RefCell::new(None) };
    }

    /// Install the process's HTTP/config backend. Called once by the component.
    pub fn set_backend(backend: Box<dyn Backend>) {
        BACKEND.with(|slot| *slot.borrow_mut() = Some(backend));
    }

    fn with<R>(f: impl FnOnce(&dyn Backend) -> Result<R, ToolError>) -> Result<R, ToolError> {
        BACKEND.with(|slot| match slot.borrow().as_deref() {
            Some(b) => f(b),
            None => Err(ToolError::msg("http backend not configured")),
        })
    }

    /// Read a value from config (Kubernetes ConfigMap/Secret, surfaced as an
    /// env var / `wasi:config`). Errors if unset.
    pub fn config(key: &str) -> Result<String, ToolError> {
        with(|b| b.config(key).ok_or_else(|| ToolError::msg(format!("missing config: {key}"))))
    }

    /// HTTP GET.
    pub fn get(url: &str) -> Result<Response, ToolError> {
        with(|b| b.fetch("GET", url, &[], &[]).map_err(ToolError::msg))
    }

    /// HTTP DELETE.
    pub fn delete(url: &str) -> Result<Response, ToolError> {
        with(|b| b.fetch("DELETE", url, &[], &[]).map_err(ToolError::msg))
    }

    /// HTTP POST with a JSON body.
    pub fn post(url: &str, body: &Json) -> Result<Response, ToolError> {
        json_send("POST", url, body)
    }

    /// HTTP PUT with a JSON body.
    pub fn put(url: &str, body: &Json) -> Result<Response, ToolError> {
        json_send("PUT", url, body)
    }

    fn json_send(method: &str, url: &str, body: &Json) -> Result<Response, ToolError> {
        let bytes = serde_json::to_vec(body)?;
        let headers = [("content-type".to_string(), "application/json".to_string())];
        with(|b| b.fetch(method, url, &bytes, &headers).map_err(ToolError::msg))
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

        /// The response body as UTF-8 text.
        pub fn text(&self) -> String {
            String::from_utf8_lossy(&self.body).into_owned()
        }

        /// Error unless the status is 2xx.
        pub fn ok(self) -> Result<Self, ToolError> {
            if (200..300).contains(&self.status) {
                Ok(self)
            } else {
                Err(ToolError::msg(format!("upstream HTTP {}: {}", self.status, self.text())))
            }
        }
    }
}
