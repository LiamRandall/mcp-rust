# Generate tools from an OpenAPI spec

If your upstream API publishes an OpenAPI 3.x document, the `generator` emits one
`#[tool]` file per operation so you don't write them by hand. It does the bulk
pass; you refine descriptions and auth afterwards.

## Usage

```sh
cargo run -p generator -- path/to/openapi.json --out examples/myapi/src/tools/
```

- Input is an OpenAPI **3.x JSON** document. Convert YAML specs to JSON first
  (e.g. `yq -o=json eval openapi.yaml > openapi.json`).
- `--out <dir>` is where the `*.rs` files and a `mod.rs` registry are written
  (default `src/tools`).

## What it generates

For each `paths.<path>.<method>` operation:

- a file `src/tools/<operationId>.rs` containing a `#[tool]` function named after
  the `operationId` (snake_cased),
- the doc comment from the operation `summary`/`description`,
- one typed argument per `path`/`query` parameter (path params are required;
  query params become `Option<…>` unless marked required),
- a `body: Json` argument when the operation has a request body,
- a body that substitutes path parameters, builds the URL from
  `servers[0].url` (overridable via `API_BASE_URL`), and calls the matching
  `http::<method>`,
- a `mod.rs` that registers every generated tool in `all()`.

For example, `GET /pets/{petId}` with `operationId: getPetById` →

```rust
/// Find a pet by its ID.
#[tool]
pub fn get_pet_by_id(
    /// The pet id.
    pet_id: i64,
) -> Result<Json, ToolError> {
    let base = http::config("API_BASE_URL")?;
    let path = "/pets/{petId}".replace("{petId}", &pet_id.to_string());
    let url = format!("{base}{path}");
    let resp = http::get(&url)?;
    Ok(resp.json()?)
}
```

## After generating

1. Review/adjust each tool's description and arguments.
2. Wire auth: read tokens via `http::config("…")`; `API_TOKEN` is auto-attached
   as a bearer header if set.
3. Build and run the [conformance loop](conformance.md) / call the tools.

## Scope and limits

The generator covers the common case (path/query params, request bodies, GET/
POST/PUT/DELETE/PATCH, `operationId` → tool name). Advanced features from the
DESIGN surface — `--include-methods`, `--include-tools <regex>`,
`--skip-long-tool-names`, OAuth2 flows — are follow-ups; for now, refine the
generated files by hand. The generator only writes the declarative per-tool
files; there is no protocol code to generate.
