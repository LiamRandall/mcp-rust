//! Static reference content the MCP conformance suite expects (tools, resources,
//! prompts, completion). For a real OpenAPI server these are replaced by the
//! generated tool set; the resource/prompt fixtures are template-only.

use serde_json::{json, Value};

// Any valid base64 satisfies the conformance checks (presence of `data`); these
// are real minimal assets so the output is also usable by a real client.
pub const PNG_1X1: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
pub const WAV_TINY: &str =
    "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAIA+AAACABAAZGF0YQAAAAA=";

pub const PROTOCOL_VERSION: &str = "2025-11-25";
pub const SUPPORTED_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];

pub fn server_capabilities() -> Value {
    json!({
        "tools": { "listChanged": false },
        "resources": { "subscribe": true, "listChanged": false },
        "prompts": { "listChanged": false },
        "logging": {},
        "completions": {}
    })
}

pub fn server_info() -> Value {
    json!({ "name": "mcp-rust-reference-server", "version": env!("CARGO_PKG_VERSION") })
}

/// `tools/list`. Schemas are JSON Schema objects; descriptions are required by
/// the conformance `tools-list` check.
pub fn tools_list() -> Value {
    let no_args = json!({ "type": "object", "properties": {}, "additionalProperties": false });
    json!([
        { "name": "test_simple_text", "description": "Returns simple text content.", "inputSchema": no_args },
        { "name": "test_image_content", "description": "Returns image content.", "inputSchema": no_args },
        { "name": "test_audio_content", "description": "Returns audio content.", "inputSchema": no_args },
        { "name": "test_embedded_resource", "description": "Returns embedded resource content.", "inputSchema": no_args },
        { "name": "test_multiple_content_types", "description": "Returns multiple content types.", "inputSchema": no_args },
        { "name": "test_tool_with_logging", "description": "Sends log notifications during execution.", "inputSchema": no_args },
        { "name": "test_error_handling", "description": "Always returns a tool error.", "inputSchema": no_args },
        { "name": "test_tool_with_progress", "description": "Reports progress notifications.", "inputSchema": no_args },
        {
            "name": "test_sampling",
            "description": "Requests LLM sampling from the client.",
            "inputSchema": { "type": "object", "properties": { "prompt": { "type": "string", "description": "The prompt to send to the LLM." } }, "required": ["prompt"] }
        },
        {
            "name": "test_elicitation",
            "description": "Requests user input (elicitation) from the client.",
            "inputSchema": { "type": "object", "properties": { "message": { "type": "string", "description": "The message to show the user." } }, "required": ["message"] }
        },
        { "name": "test_elicitation_sep1034_defaults", "description": "Elicitation with default values for all primitive types.", "inputSchema": no_args },
        { "name": "test_elicitation_sep1330_enums", "description": "Elicitation with all five enum variants.", "inputSchema": no_args }
    ])
}

pub fn resources_list() -> Value {
    json!([
        { "uri": "test://static-text", "name": "Static Text Resource", "description": "A static text resource.", "mimeType": "text/plain" },
        { "uri": "test://static-binary", "name": "Static Binary Resource", "description": "A static binary resource.", "mimeType": "image/png" },
        { "uri": "test://watched-resource", "name": "Watched Resource", "description": "A subscribable resource.", "mimeType": "text/plain" },
        { "uri": "test://example-resource", "name": "Example Resource", "description": "An example resource.", "mimeType": "text/plain" }
    ])
}

pub fn resource_templates_list() -> Value {
    json!([
        { "uriTemplate": "test://template/{id}/data", "name": "Templated Resource", "description": "Resource addressed by id.", "mimeType": "application/json" }
    ])
}

/// `resources/read` for a concrete URI (templates substituted).
pub fn resource_read(uri: &str) -> Value {
    // test://template/{id}/data
    if let Some(rest) = uri.strip_prefix("test://template/") {
        if let Some(id) = rest.strip_suffix("/data") {
            let text = format!(
                "{{\"id\":\"{id}\",\"templateTest\":true,\"data\":\"Data for ID: {id}\"}}"
            );
            return json!({ "contents": [ { "uri": uri, "mimeType": "application/json", "text": text } ] });
        }
    }
    match uri {
        "test://static-binary" => json!({
            "contents": [ { "uri": uri, "mimeType": "image/png", "blob": PNG_1X1 } ]
        }),
        "test://static-text" => json!({
            "contents": [ { "uri": uri, "mimeType": "text/plain", "text": "This is the content of the static text resource." } ]
        }),
        _ => json!({
            "contents": [ { "uri": uri, "mimeType": "text/plain", "text": "This is the content of the static text resource." } ]
        }),
    }
}

pub fn prompts_list() -> Value {
    json!([
        { "name": "test_simple_prompt", "description": "A simple prompt with no arguments." },
        {
            "name": "test_prompt_with_arguments",
            "description": "A parameterized prompt.",
            "arguments": [
                { "name": "arg1", "description": "First test argument", "required": true },
                { "name": "arg2", "description": "Second test argument", "required": true }
            ]
        },
        {
            "name": "test_prompt_with_embedded_resource",
            "description": "A prompt with an embedded resource.",
            "arguments": [ { "name": "resourceUri", "description": "URI of the resource to embed", "required": true } ]
        },
        { "name": "test_prompt_with_image", "description": "A prompt with image content." }
    ])
}

/// `prompts/get`. Returns `Some(result)` for known prompts.
pub fn prompt_get(name: &str, args: &Value) -> Option<Value> {
    let arg = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("");
    match name {
        "test_simple_prompt" => Some(json!({
            "messages": [ { "role": "user", "content": { "type": "text", "text": "This is a simple prompt for testing." } } ]
        })),
        "test_prompt_with_arguments" => Some(json!({
            "messages": [ { "role": "user", "content": { "type": "text",
                "text": format!("Prompt with arguments: arg1='{}', arg2='{}'", arg("arg1"), arg("arg2")) } } ]
        })),
        "test_prompt_with_embedded_resource" => {
            let uri = {
                let u = arg("resourceUri");
                if u.is_empty() { "test://example-resource".to_string() } else { u.to_string() }
            };
            Some(json!({
                "messages": [
                    { "role": "user", "content": { "type": "resource", "resource": { "uri": uri, "mimeType": "text/plain", "text": "Embedded resource content for testing." } } },
                    { "role": "user", "content": { "type": "text", "text": "Please process the embedded resource above." } }
                ]
            }))
        }
        "test_prompt_with_image" => Some(json!({
            "messages": [
                { "role": "user", "content": { "type": "image", "data": PNG_1X1, "mimeType": "image/png" } },
                { "role": "user", "content": { "type": "text", "text": "Please analyze the image above." } }
            ]
        })),
        _ => None,
    }
}

pub fn completion_result() -> Value {
    json!({ "completion": { "values": ["test_completion"], "total": 1, "hasMore": false } })
}
