//! JSON-RPC routing for MCP `2025-11-25`. Decides, per request, whether the
//! reply is a single JSON response, a `202 Accepted` (notifications / client
//! responses), or an SSE stream (notifications + server→client requests).

use serde_json::{json, Value};

use crate::fixtures;
use crate::kv;
use crate::sse::{Kind, Note, Plan};

pub enum Reply {
    /// A JSON-RPC response object, plus an optional `Mcp-Session-Id` to set.
    Json(Value, Option<String>),
    /// `202 Accepted` with no body (notifications and client→server responses).
    Accepted,
    /// A streamed Server-Sent-Events response.
    Sse(Plan),
    /// A raw HTTP status (error paths).
    Status(u16, Option<Value>),
}

pub fn error_envelope(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn ok(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.clone(), "result": result })
}

pub fn route(msg: &Value) -> Reply {
    // Batches are not used by the conformance client; acknowledge them.
    if msg.is_array() {
        return Reply::Accepted;
    }

    let method = msg.get("method").and_then(|m| m.as_str());

    // A client→server *response* (sampling / elicitation reply): no method,
    // has id + result/error. Correlate it for the waiting SSE handler.
    if method.is_none() {
        if let Some(idv) = msg.get("id") {
            if msg.get("result").is_some() || msg.get("error").is_some() {
                let corr = idv
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| idv.to_string());
                kv::put(
                    &format!("mcpresp:{corr}"),
                    &serde_json::to_vec(msg).unwrap_or_default(),
                );
            }
        }
        return Reply::Accepted;
    }
    let method = method.unwrap();

    // Notifications have no id.
    let id = match msg.get("id").cloned() {
        Some(id) => id,
        None => return Reply::Accepted,
    };

    match method {
        "initialize" => {
            let requested = msg
                .pointer("/params/protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or(fixtures::PROTOCOL_VERSION);
            let version = if fixtures::SUPPORTED_VERSIONS.contains(&requested) {
                requested
            } else {
                fixtures::PROTOCOL_VERSION
            };
            let result = json!({
                "protocolVersion": version,
                "serverInfo": fixtures::server_info(),
                "capabilities": fixtures::server_capabilities(),
            });
            Reply::Json(ok(&id, result), Some(kv::unique_id()))
        }
        "ping" => Reply::Json(ok(&id, json!({})), None),
        "logging/setLevel" => Reply::Json(ok(&id, json!({})), None),
        "completion/complete" => Reply::Json(ok(&id, fixtures::completion_result()), None),
        "resources/list" => {
            Reply::Json(ok(&id, json!({ "resources": fixtures::resources_list() })), None)
        }
        "resources/templates/list" => Reply::Json(
            ok(&id, json!({ "resourceTemplates": fixtures::resource_templates_list() })),
            None,
        ),
        "resources/read" => {
            let uri = msg.pointer("/params/uri").and_then(|v| v.as_str()).unwrap_or("");
            Reply::Json(ok(&id, fixtures::resource_read(uri)), None)
        }
        "resources/subscribe" | "resources/unsubscribe" => {
            Reply::Json(ok(&id, json!({})), None)
        }
        "prompts/list" => {
            Reply::Json(ok(&id, json!({ "prompts": fixtures::prompts_list() })), None)
        }
        "prompts/get" => {
            let name = msg.pointer("/params/name").and_then(|v| v.as_str()).unwrap_or("");
            let args = msg.pointer("/params/arguments").cloned().unwrap_or(json!({}));
            match fixtures::prompt_get(name, &args) {
                Some(r) => Reply::Json(ok(&id, r), None),
                None => Reply::Json(error_envelope(id, -32602, "Unknown prompt"), None),
            }
        }
        "tools/list" => Reply::Json(ok(&id, json!({ "tools": fixtures::tools_list() })), None),
        "tools/call" => route_tool_call(&id, msg),
        _ => Reply::Json(error_envelope(id, -32601, "Method not found"), None),
    }
}

fn json_tool(id: &Value, result: Value) -> Reply {
    Reply::Json(ok(id, result), None)
}

fn route_tool_call(id: &Value, msg: &Value) -> Reply {
    let name = msg.pointer("/params/name").and_then(|v| v.as_str()).unwrap_or("");
    let args = msg.pointer("/params/arguments").cloned().unwrap_or(json!({}));
    let progress_token = msg.pointer("/params/_meta/progressToken").cloned();

    match name {
        "test_simple_text" => json_tool(
            id,
            json!({ "content": [ { "type": "text", "text": "This is a simple text response for testing." } ] }),
        ),
        "test_image_content" => json_tool(
            id,
            json!({ "content": [ { "type": "image", "data": fixtures::PNG_1X1, "mimeType": "image/png" } ] }),
        ),
        "test_audio_content" => json_tool(
            id,
            json!({ "content": [ { "type": "audio", "data": fixtures::WAV_TINY, "mimeType": "audio/wav" } ] }),
        ),
        "test_embedded_resource" => json_tool(
            id,
            json!({ "content": [ { "type": "resource", "resource": { "uri": "test://embedded-resource", "mimeType": "text/plain", "text": "This is an embedded resource content." } } ] }),
        ),
        "test_multiple_content_types" => json_tool(
            id,
            json!({ "content": [
                { "type": "text", "text": "Multiple content types test:" },
                { "type": "image", "data": fixtures::PNG_1X1, "mimeType": "image/png" },
                { "type": "resource", "resource": { "uri": "test://mixed-content-resource", "mimeType": "application/json", "text": "{\"test\":\"data\",\"value\":123}" } }
            ] }),
        ),
        "test_error_handling" => json_tool(
            id,
            json!({ "isError": true, "content": [ { "type": "text", "text": "This tool intentionally returns an error for testing" } ] }),
        ),
        "test_tool_with_logging" => Reply::Sse(Plan::Notify {
            id: id.clone(),
            notes: vec![
                Note::Log { level: "info".into(), data: json!("Tool execution started") },
                Note::Delay(50),
                Note::Log { level: "info".into(), data: json!("Tool processing data") },
                Note::Delay(50),
                Note::Log { level: "info".into(), data: json!("Tool execution completed") },
            ],
            result: json!({ "content": [ { "type": "text", "text": "Tool with logging executed" } ] }),
        }),
        "test_tool_with_progress" => {
            let token = progress_token.unwrap_or(json!("progress"));
            Reply::Sse(Plan::Notify {
                id: id.clone(),
                notes: vec![
                    Note::Progress { token: token.clone(), progress: 0.0, total: 100.0 },
                    Note::Delay(50),
                    Note::Progress { token: token.clone(), progress: 50.0, total: 100.0 },
                    Note::Delay(50),
                    Note::Progress { token, progress: 100.0, total: 100.0 },
                ],
                result: json!({ "content": [ { "type": "text", "text": "Tool with progress executed" } ] }),
            })
        }
        "test_sampling" => {
            let prompt = args.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
            let corr = kv::unique_id();
            let request = json!({
                "jsonrpc": "2.0", "id": corr, "method": "sampling/createMessage",
                "params": {
                    "messages": [ { "role": "user", "content": { "type": "text", "text": prompt } } ],
                    "maxTokens": 100
                }
            });
            Reply::Sse(Plan::Callback { id: id.clone(), request, corr_id: corr, kind: Kind::Sampling })
        }
        "test_elicitation" => {
            let message = args.get("message").and_then(|v| v.as_str()).unwrap_or("");
            let corr = kv::unique_id();
            let request = json!({
                "jsonrpc": "2.0", "id": corr, "method": "elicitation/create",
                "params": {
                    "message": message,
                    "requestedSchema": { "type": "object", "properties": {
                        "username": { "type": "string", "description": "User's response" },
                        "email": { "type": "string", "description": "User's email address" }
                    }, "required": ["username", "email"] }
                }
            });
            Reply::Sse(Plan::Callback { id: id.clone(), request, corr_id: corr, kind: Kind::Elicitation })
        }
        "test_elicitation_sep1034_defaults" => {
            let corr = kv::unique_id();
            let request = json!({
                "jsonrpc": "2.0", "id": corr, "method": "elicitation/create",
                "params": {
                    "message": "Please provide your information",
                    "requestedSchema": { "type": "object", "properties": {
                        "name": { "type": "string", "default": "John Doe" },
                        "age": { "type": "integer", "default": 30 },
                        "score": { "type": "number", "default": 95.5 },
                        "status": { "type": "string", "enum": ["active", "inactive", "pending"], "default": "active" },
                        "verified": { "type": "boolean", "default": true }
                    } }
                }
            });
            Reply::Sse(Plan::Callback { id: id.clone(), request, corr_id: corr, kind: Kind::Elicitation })
        }
        "test_elicitation_sep1330_enums" => {
            let corr = kv::unique_id();
            let request = json!({
                "jsonrpc": "2.0", "id": corr, "method": "elicitation/create",
                "params": {
                    "message": "Please choose options",
                    "requestedSchema": { "type": "object", "properties": {
                        "untitledSingle": { "type": "string", "enum": ["option1", "option2", "option3"] },
                        "titledSingle": { "type": "string", "oneOf": [
                            { "const": "value1", "title": "First Option" },
                            { "const": "value2", "title": "Second Option" },
                            { "const": "value3", "title": "Third Option" }
                        ] },
                        "legacyEnum": { "type": "string", "enum": ["opt1", "opt2", "opt3"], "enumNames": ["Option One", "Option Two", "Option Three"] },
                        "untitledMulti": { "type": "array", "items": { "type": "string", "enum": ["option1", "option2", "option3"] } },
                        "titledMulti": { "type": "array", "items": { "anyOf": [
                            { "const": "value1", "title": "First Choice" },
                            { "const": "value2", "title": "Second Choice" },
                            { "const": "value3", "title": "Third Choice" }
                        ] } }
                    } }
                }
            });
            Reply::Sse(Plan::Callback { id: id.clone(), request, corr_id: corr, kind: Kind::Elicitation })
        }
        _ => json_tool(
            id,
            json!({ "isError": true, "content": [ { "type": "text", "text": format!("Unknown tool: {name}") } ] }),
        ),
    }
}
