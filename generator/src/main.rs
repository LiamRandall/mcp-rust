//! OpenAPI 3.x → one `src/tools/<operationId>.rs` per operation, in the
//! `#[tool]` shape from DESIGN §6/§10. Runs on the host (not in wasm).
//!
//!   cargo run -p generator -- path/to/openapi.json --out src/tools/
//!
//! Emits declarative per-tool files only — there is no protocol code to write.
//! Refine descriptions/auth per tool afterwards and run the conformance loop.

use serde_json::Value;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut spec_path: Option<String> = None;
    let mut out = PathBuf::from("src/tools");
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => out = PathBuf::from(args.next().unwrap_or_else(|| "src/tools".into())),
            "-h" | "--help" => {
                eprintln!("usage: generator <openapi.json> [--out <dir>]");
                return;
            }
            other => spec_path = Some(other.to_string()),
        }
    }
    let spec_path = match spec_path {
        Some(p) => p,
        None => {
            eprintln!("error: missing <openapi.json>. Try --help.");
            std::process::exit(2);
        }
    };

    let text = std::fs::read_to_string(&spec_path).unwrap_or_else(|e| {
        eprintln!("error: cannot read {spec_path}: {e}");
        std::process::exit(1);
    });
    let spec: Value = serde_json::from_str(&text).unwrap_or_else(|e| {
        eprintln!("error: {spec_path} is not JSON ({e}). Convert YAML specs to JSON first.");
        std::process::exit(1);
    });

    let tools = generate_tools(&spec);
    if tools.is_empty() {
        eprintln!("no operations found in {spec_path}");
        return;
    }
    std::fs::create_dir_all(&out).unwrap();
    let mut mods = Vec::new();
    for t in &tools {
        let path = out.join(format!("{}.rs", t.name));
        std::fs::write(&path, &t.source).unwrap();
        mods.push(t.name.clone());
        println!("wrote {}", path.display());
    }
    let modrs = render_mod(&mods);
    std::fs::write(out.join("mod.rs"), modrs).unwrap();
    println!("wrote {} ({} tools)", out.join("mod.rs").display(), mods.len());
}

struct GenTool {
    name: String,
    source: String,
}

const HTTP_METHODS: &[&str] = &["get", "post", "put", "delete", "patch"];

fn generate_tools(spec: &Value) -> Vec<GenTool> {
    let base = spec
        .pointer("/servers/0/url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string();

    let mut out = Vec::new();
    let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) else {
        return out;
    };

    for (path, item) in paths {
        let Some(item) = item.as_object() else { continue };
        for method in HTTP_METHODS {
            let Some(op) = item.get(*method).and_then(|o| o.as_object()) else {
                continue;
            };
            let op_id = op
                .get("operationId")
                .and_then(|v| v.as_str())
                .map(snake_case)
                .unwrap_or_else(|| snake_case(&format!("{method}_{path}")));

            let desc = op
                .get("summary")
                .or_else(|| op.get("description"))
                .and_then(|v| v.as_str())
                .unwrap_or("Generated tool.")
                .lines()
                .next()
                .unwrap_or("Generated tool.")
                .to_string();

            let params = collect_params(op.get("parameters"));
            let has_body = op.get("requestBody").is_some();

            out.push(GenTool {
                source: render_tool(&op_id, &desc, &base, path, method, &params, has_body),
                name: op_id,
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

struct Param {
    name: String,
    /// Original spec name, used for `{placeholder}` substitution in the path.
    orig: String,
    rust_ty: String,
    description: String,
    in_path: bool,
    required: bool,
}

fn collect_params(params: Option<&Value>) -> Vec<Param> {
    let mut out = Vec::new();
    let Some(arr) = params.and_then(|p| p.as_array()) else {
        return out;
    };
    for p in arr {
        let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let loc = p.get("in").and_then(|v| v.as_str()).unwrap_or("query");
        if loc != "path" && loc != "query" {
            continue; // header/cookie params are not surfaced as tool args
        }
        let required = p.get("required").and_then(|v| v.as_bool()).unwrap_or(loc == "path");
        let base_ty = rust_type(p.get("schema"));
        let rust_ty = if required { base_ty } else { format!("Option<{base_ty}>") };
        out.push(Param {
            name: snake_case(name),
            orig: name.to_string(),
            rust_ty,
            description: p
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            in_path: loc == "path",
            required,
        });
    }
    out
}

fn rust_type(schema: Option<&Value>) -> String {
    let Some(schema) = schema else { return "String".into() };
    match schema.get("type").and_then(|v| v.as_str()) {
        Some("integer") => "i64".into(),
        Some("number") => "f64".into(),
        Some("boolean") => "bool".into(),
        Some("array") => format!("Vec<{}>", rust_type(schema.get("items"))),
        _ => "String".into(),
    }
}

fn render_tool(
    op_id: &str,
    desc: &str,
    base: &str,
    path: &str,
    method: &str,
    params: &[Param],
    has_body: bool,
) -> String {
    let mut s = String::new();
    s.push_str("//! Generated from OpenAPI. Refine the description/auth as needed.\n\n");
    s.push_str("use mcp_core::{http, tool, Json, ToolError};\n\n");
    s.push_str(&format!("/// {desc}\n#[tool]\npub fn {op_id}(\n"));
    for p in params {
        if !p.description.is_empty() {
            s.push_str(&format!("    /// {}\n", p.description));
        }
        s.push_str(&format!("    {}: {},\n", p.name, p.rust_ty));
    }
    if has_body {
        s.push_str("    /// JSON request body.\n    body: Json,\n");
    }
    s.push_str(") -> Result<Json, ToolError> {\n");
    s.push_str("    let base = http::config(\"API_BASE_URL\")?;\n");
    if !base.is_empty() {
        s.push_str(&format!("    // OpenAPI servers[0].url: {base}\n"));
    }

    // Build the path with {param} substitutions.
    let mut url_expr = format!("\"{path}\"");
    for p in params.iter().filter(|p| p.in_path) {
        url_expr = format!("{url_expr}.replace(\"{{{0}}}\", &{1}.to_string())", p.orig, p.name);
    }
    s.push_str(&format!("    let path = {url_expr};\n"));
    s.push_str("    let url = format!(\"{base}{path}\");\n");

    match method {
        "get" | "delete" => s.push_str(&format!("    let resp = http::{method}(&url)?;\n")),
        _ => {
            if has_body {
                s.push_str(&format!("    let resp = http::{method}(&url, &body)?;\n"));
            } else {
                s.push_str(&format!("    let resp = http::{method}(&url, &Json::Null)?;\n"));
            }
        }
    }
    s.push_str("    Ok(resp.json()?)\n}\n");
    let _ = params.iter().filter(|p| p.required).count(); // required already encoded in type
    s
}

fn render_mod(mods: &[String]) -> String {
    let mut s = String::new();
    s.push_str("//! Generated tool registry. Regenerate with the generator, or edit by hand.\n\n");
    s.push_str("use mcp_core::ToolHandle;\n\n");
    for m in mods {
        s.push_str(&format!("pub mod {m};\n"));
    }
    s.push_str("\npub fn all() -> Vec<ToolHandle> {\n    vec![\n");
    for m in mods {
        s.push_str(&format!("        {m}::{m}_tool(),\n"));
    }
    s.push_str("    ]\n}\n");
    s
}

/// Lowercase snake_case from camelCase / kebab / path-ish identifiers.
fn snake_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let mut prev_lower = false;
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            if prev_lower {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
            prev_lower = false;
        } else if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        } else {
            if !out.ends_with('_') && !out.is_empty() {
                out.push('_');
            }
            prev_lower = false;
        }
    }
    out.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn petstore() -> Value {
        json!({
            "openapi": "3.0.0",
            "servers": [{ "url": "https://api.example.com/v1" }],
            "paths": {
                "/pets/{petId}": {
                    "get": {
                        "operationId": "getPetById",
                        "summary": "Fetch a pet by its ID.",
                        "parameters": [
                            { "name": "petId", "in": "path", "required": true,
                              "schema": { "type": "integer" }, "description": "The pet id." }
                        ]
                    }
                },
                "/pets": {
                    "get": {
                        "operationId": "listPets",
                        "summary": "List pets.",
                        "parameters": [
                            { "name": "limit", "in": "query", "required": false,
                              "schema": { "type": "integer" }, "description": "Max results." }
                        ]
                    },
                    "post": {
                        "operationId": "createPet",
                        "summary": "Create a pet.",
                        "requestBody": { "content": { "application/json": {} } }
                    }
                }
            }
        })
    }

    #[test]
    fn generates_one_tool_per_operation() {
        let tools = generate_tools(&petstore());
        let names: Vec<_> = tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["create_pet", "get_pet_by_id", "list_pets"]);
    }

    #[test]
    fn path_param_is_required_and_substituted() {
        let tools = generate_tools(&petstore());
        let g = tools.iter().find(|t| t.name == "get_pet_by_id").unwrap();
        assert!(g.source.contains("pet_id: i64"));
        assert!(g.source.contains(".replace(\"{petId}\", &pet_id.to_string())"));
        assert!(g.source.contains("#[tool]"));
        assert!(g.source.contains("http::get(&url)"));
    }

    #[test]
    fn query_param_is_optional() {
        let tools = generate_tools(&petstore());
        let g = tools.iter().find(|t| t.name == "list_pets").unwrap();
        assert!(g.source.contains("limit: Option<i64>"));
    }

    #[test]
    fn post_with_body_passes_body() {
        let tools = generate_tools(&petstore());
        let g = tools.iter().find(|t| t.name == "create_pet").unwrap();
        assert!(g.source.contains("body: Json"));
        assert!(g.source.contains("http::post(&url, &body)"));
    }

    #[test]
    fn mod_registry_lists_all() {
        let m = render_mod(&["get_pet_by_id".into(), "list_pets".into()]);
        assert!(m.contains("pub mod get_pet_by_id;"));
        assert!(m.contains("get_pet_by_id::get_pet_by_id_tool(),"));
    }
}
