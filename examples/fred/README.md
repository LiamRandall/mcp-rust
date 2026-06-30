# fred — walkthrough

An MCP server that bridges the **Federal Reserve Economic Data (FRED)** API
(`https://api.stlouisfed.org`). This example shows a real public API that needs
**authentication** (an API key) and **query parameters** — the pattern most
production API bridges follow.

## What it does

| Tool | FRED endpoint | Notes |
|---|---|---|
| `search_series(query)` | `/fred/series/search` | Find series by text, e.g. "unemployment" |
| `get_series(series_id)` | `/fred/series` | Series metadata, e.g. `UNRATE` |
| `get_series_observations(series_id, limit?)` | `/fred/series/observations` | The actual data points, newest first |

## Prerequisites

- Rust + `wasm32-wasip2`, `wasmtime` 39+, `curl` (see `make doctor`).
- A **free FRED API key**: request one at
  <https://fred.stlouisfed.org/docs/api/api_key.html> (instant, no cost).
- Outbound network access to `api.stlouisfed.org`.

## 1. Build

```sh
cd examples/fred
cargo build --release --target wasm32-wasip2
```

## 2. Run (with your API key)

The key is read from the `FRED_API_KEY` config value. Locally that is an env var
passed to `wasmtime serve`:

```sh
wasmtime serve -Scli -Shttp \
  --env FRED_API_KEY=YOUR_FRED_API_KEY \
  target/wasm32-wasip2/release/fred.wasm --addr 127.0.0.1:8092
```

The key is **never** baked into the `.wasm`; it is injected at runtime (env var
locally; a Kubernetes `Secret` / `wasi:config` in production).

## 3. Call it

```sh
# Search for unemployment-related series
curl -s -X POST http://127.0.0.1:8092/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"search_series","arguments":{"query":"unemployment rate"}}}'

# Metadata for the US unemployment rate series
curl -s -X POST http://127.0.0.1:8092/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_series","arguments":{"series_id":"UNRATE"}}}'

# The 5 most recent observations of UNRATE
curl -s -X POST http://127.0.0.1:8092/ -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_series_observations","arguments":{"series_id":"UNRATE","limit":5}}}'
```

Each returns the FRED JSON as MCP text content. If `FRED_API_KEY` is missing the
tool returns a clear error telling you to set it (`isError: true`).

## How it's built

`src/lib.rs` holds two helpers and the registration:

- `fred_url(path, params)` reads `FRED_API_KEY` from config and appends
  `api_key` + `file_type=json` + the query params (percent-encoded).
- each tool in `src/tools/` builds its URL and calls `http::get(&url)?.ok()?`.

```rust
/// Fetch metadata for a FRED economic data series by its ID.
#[tool]
pub fn get_series(
    /// The FRED series ID, e.g. "UNRATE".
    series_id: String,
) -> Result<Json, ToolError> {
    let url = fred_url("/fred/series", &[("series_id", &series_id)])?;
    let resp = http::get(&url)?.ok()?;
    Ok(resp.json()?)
}
```

`get_series_observations` shows an **optional** argument: `limit: Option<u32>`
becomes a non-required property in the JSON Schema and defaults to 10.

## Use it from an MCP client

Point your client at `http://127.0.0.1:8092/`. The three FRED tools appear and
an assistant can answer questions like *"what's the latest US unemployment
rate?"* by calling `get_series_observations("UNRATE")`.

## Deploy

See [`docs/deploying.md`](../../docs/deploying.md). On wasmCloud:

- register the key as a secret: `cosmonic_set_secret FRED_API_KEY <value>` (or a
  Kubernetes `Secret`) and reference it from the workload's `secretFrom`;
- add `api.stlouisfed.org` to the workload's outbound `allowedHosts`.
