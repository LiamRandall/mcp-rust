//! Verifies `#[tool]` schema generation and dispatch against hand-written
//! expectations (DESIGN §11 Phase 1 gate).

use mcp_core::{tool, Json, ToolError};
use serde_json::json;

/// Fetch a pet by its ID.
#[tool]
pub fn get_pet(
    /// The pet's unique identifier.
    pet_id: u64,
) -> Result<Json, ToolError> {
    Ok(json!({ "pet_id": pet_id }))
}

/// Search with optional filters.
#[tool]
pub fn search(
    /// Free-text query.
    query: String,
    /// Tags to filter by.
    tags: Vec<String>,
    /// Maximum results.
    limit: Option<u32>,
    /// Include archived items.
    include_archived: bool,
) -> Result<Json, ToolError> {
    Ok(json!({
        "query": query,
        "tags": tags,
        "limit": limit,
        "include_archived": include_archived,
    }))
}

fn schema(s: &str) -> Json {
    serde_json::from_str(s).expect("schema is valid JSON")
}

#[test]
fn metadata_from_signature_and_docs() {
    let h = get_pet_tool();
    assert_eq!(h.name, "get_pet");
    assert_eq!(h.description, "Fetch a pet by its ID.");
}

#[test]
fn simple_schema_matches_handwritten() {
    let h = get_pet_tool();
    let expected = json!({
        "type": "object",
        "properties": {
            "pet_id": { "type": "integer", "description": "The pet's unique identifier." }
        },
        "required": ["pet_id"],
        "additionalProperties": false
    });
    assert_eq!(schema(h.input_schema), expected);
}

#[test]
fn complex_schema_handles_vec_and_option() {
    let h = search_tool();
    let expected = json!({
        "type": "object",
        "properties": {
            "query": { "type": "string", "description": "Free-text query." },
            "tags": { "type": "array", "items": { "type": "string" }, "description": "Tags to filter by." },
            "limit": { "type": "integer", "description": "Maximum results." },
            "include_archived": { "type": "boolean", "description": "Include archived items." }
        },
        // Option<T> (limit) is excluded from required.
        "required": ["query", "tags", "include_archived"],
        "additionalProperties": false
    });
    assert_eq!(schema(h.input_schema), expected);
}

#[test]
fn dispatch_deserializes_and_calls() {
    let h = get_pet_tool();
    let out = (h.call)(&json!({ "pet_id": 7 })).unwrap();
    assert_eq!(out, json!({ "pet_id": 7 }));
}

#[test]
fn dispatch_fills_optional_with_none() {
    let h = search_tool();
    let out = (h.call)(&json!({
        "query": "cats", "tags": ["cute"], "include_archived": false
    }))
    .unwrap();
    assert_eq!(out["limit"], Json::Null);
    assert_eq!(out["query"], json!("cats"));
}

#[test]
fn dispatch_errors_on_missing_required() {
    let h = get_pet_tool();
    let err = (h.call)(&json!({})).unwrap_err();
    assert!(err.message.contains("pet_id"), "got: {}", err.message);
}

#[test]
fn dispatch_errors_on_wrong_type() {
    let h = get_pet_tool();
    let err = (h.call)(&json!({ "pet_id": "not-a-number" })).unwrap_err();
    assert!(err.message.contains("pet_id"), "got: {}", err.message);
}
