//! Pure MCP JSON-RPC router for **API-bridging tool servers** — the kind the
//! generator and `#[tool]` authoring path produce. No wasm, no transport: it
//! takes a request body + a tool registry and returns bytes, so it is fully
//! unit-testable on the host. The wasm glue (`mcp-api-server`) wraps it.
//!
//! This is the lighter sibling of `mcp-server-v1` (the conformance reference
//! server): tool servers only need lifecycle + `tools/list` + `tools/call`, all
//! over plain `application/json` (which every MCP Streamable-HTTP client
//! accepts). No SSE, resources, prompts, sampling, or elicitation.

use mcp_core::ToolHandle;
use serde_json::{json, Value};

pub const PROTOCOL_VERSION: &str = "2025-11-25";
pub const SUPPORTED_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];

/// Identity advertised in the `initialize` handshake.
pub struct ServerInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub instructions: Option<&'static str>,
}

/// What the HTTP layer should send back.
pub struct ApiResponse {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl ApiResponse {
    fn json(value: &Value) -> Self {
        ApiResponse {
            status: 200,
            content_type: "application/json",
            body: serde_json::to_vec(value).unwrap_or_default(),
        }
    }
    fn accepted() -> Self {
        ApiResponse { status: 202, content_type: "application/json", body: Vec::new() }
    }
}

/// Route a request body against `tools`, producing the HTTP response bytes.
pub fn handle(body: &[u8], info: &ServerInfo, tools: &[ToolHandle]) -> ApiResponse {
    let msg: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return ApiResponse::json(&error(Value::Null, -32700, "Parse error")),
    };

    let method = match msg.get("method").and_then(|m| m.as_str()) {
        Some(m) => m,
        // A response/notification from the client — nothing to answer.
        None => return ApiResponse::accepted(),
    };
    let id = match msg.get("id").cloned() {
        Some(id) => id,
        None => return ApiResponse::accepted(), // notification
    };

    let result = match method {
        "initialize" => initialize(&msg, info),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tool_defs(tools) }),
        "tools/call" => return ApiResponse::json(&ok(id, call_tool(&msg, tools))),
        "notifications/initialized" => return ApiResponse::accepted(),
        _ => return ApiResponse::json(&error(id, -32601, "Method not found")),
    };
    ApiResponse::json(&ok(id, result))
}

fn initialize(msg: &Value, info: &ServerInfo) -> Value {
    let requested = msg
        .pointer("/params/protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or(PROTOCOL_VERSION);
    let version = if SUPPORTED_VERSIONS.contains(&requested) {
        requested
    } else {
        PROTOCOL_VERSION
    };
    let mut result = json!({
        "protocolVersion": version,
        "serverInfo": { "name": info.name, "version": info.version },
        "capabilities": { "tools": { "listChanged": false } },
    });
    if let Some(instructions) = info.instructions {
        result["instructions"] = json!(instructions);
    }
    result
}

fn tool_defs(tools: &[ToolHandle]) -> Value {
    let defs: Vec<Value> = tools
        .iter()
        .map(|t| {
            let schema: Value =
                serde_json::from_str(t.input_schema).unwrap_or_else(|_| json!({ "type": "object" }));
            json!({ "name": t.name, "description": t.description, "inputSchema": schema })
        })
        .collect();
    Value::Array(defs)
}

/// Dispatch `tools/call` and shape the result as MCP content.
fn call_tool(msg: &Value, tools: &[ToolHandle]) -> Value {
    let name = msg.pointer("/params/name").and_then(|v| v.as_str()).unwrap_or("");
    let args = msg.pointer("/params/arguments").cloned().unwrap_or(json!({}));

    let Some(tool) = tools.iter().find(|t| t.name == name) else {
        return error_content(format!("Unknown tool: {name}"));
    };
    match (tool.call)(&args) {
        Ok(value) => json!({ "content": [ text_block(&value) ], "isError": false }),
        Err(e) => error_content(e.message),
    }
}

/// JSON values become text content: strings verbatim, everything else pretty.
fn text_block(value: &Value) -> Value {
    let text = match value {
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    };
    json!({ "type": "text", "text": text })
}

fn error_content(message: impl Into<String>) -> Value {
    json!({ "isError": true, "content": [ { "type": "text", "text": message.into() } ] })
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcp_core::{tool, Json, ToolError};

    /// Echo the message back.
    #[tool]
    fn echo(
        /// Text to echo.
        message: String,
    ) -> Result<Json, ToolError> {
        if message == "boom" {
            return Err(ToolError::msg("explosion"));
        }
        Ok(json!({ "echoed": message }))
    }

    fn info() -> ServerInfo {
        ServerInfo { name: "test", version: "0.1.0", instructions: Some("hi") }
    }
    fn tools() -> Vec<ToolHandle> {
        vec![echo_tool()]
    }
    fn call(body: Value) -> Value {
        let r = handle(&serde_json::to_vec(&body).unwrap(), &info(), &tools());
        serde_json::from_slice(&r.body).unwrap_or(Value::Null)
    }

    #[test]
    fn initialize_advertises_tools_and_instructions() {
        let r = call(json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-11-25" } }));
        assert_eq!(r["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(r["result"]["capabilities"]["tools"]["listChanged"], false);
        assert_eq!(r["result"]["instructions"], "hi");
        assert_eq!(r["result"]["serverInfo"]["name"], "test");
    }

    #[test]
    fn tools_list_includes_schema() {
        let r = call(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }));
        let tools = r["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "echo");
        assert_eq!(tools[0]["inputSchema"]["properties"]["message"]["type"], "string");
    }

    #[test]
    fn tools_call_returns_text_content() {
        let r = call(json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "echo", "arguments": { "message": "hello" } } }));
        assert_eq!(r["result"]["isError"], false);
        let text = r["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"echoed\": \"hello\""), "got {text}");
    }

    #[test]
    fn tool_error_maps_to_is_error() {
        let r = call(json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "echo", "arguments": { "message": "boom" } } }));
        assert_eq!(r["result"]["isError"], true);
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("explosion"));
    }

    #[test]
    fn unknown_tool_is_error_content() {
        let r = call(json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": { "name": "nope", "arguments": {} } }));
        assert_eq!(r["result"]["isError"], true);
    }

    #[test]
    fn ping_and_notifications() {
        let r = call(json!({ "jsonrpc": "2.0", "id": 6, "method": "ping" }));
        assert_eq!(r["result"], json!({}));
        let n = handle(
            &serde_json::to_vec(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
                .unwrap(),
            &info(),
            &tools(),
        );
        assert_eq!(n.status, 202);
    }

    #[test]
    fn unknown_method_errors() {
        let r = call(json!({ "jsonrpc": "2.0", "id": 7, "method": "frobnicate" }));
        assert_eq!(r["error"]["code"], -32601);
    }
}
