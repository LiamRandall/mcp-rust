# Conformance

This repo ships a **conformance reference server** — `crates/mcp-server-v1` — a
single WASI 0.2 component implementing the full MCP `2025-11-25` server surface.
It passes the **entire official conformance suite** with an **empty baseline**.

> The lightweight API-bridging examples (hello-world / petstore / fred) implement
> just the tool surface (lifecycle + `tools/*`). The full conformance gate is the
> reference server, which additionally implements resources, prompts, completion,
> logging, progress, sampling, elicitation, SSE, and DNS-rebinding protection.

## Run the suite

```sh
make build                 # build crates/mcp-server-v1 -> dist/server.wasm
make serve &               # wasmtime serve -Scli -Shttp -Skeyvalue
make conformance           # official suite @ 2025-11-25, empty baseline -> exit 0
```

`make conformance` runs:

```sh
npx -y @modelcontextprotocol/conformance@0.1.16 server \
  --url http://127.0.0.1:8080/mcp \
  --suite active --spec-version 2025-11-25 \
  --expected-failures conformance-baseline.yml
```

The active suite for `2025-11-25` is **30 scenarios / 39 checks**. A passing run
ends with `Total: 39 passed, 0 failed` and exit code 0.

## The baseline ships empty

[`conformance-baseline.yml`](../conformance-baseline.yml) is the list of
permitted known-failures. It is **empty** and must stay empty — never silence a
failure by adding it. Fix the server instead.

## The green loop

The reference server is structured so failures map to one place:

| Failure | Owner |
|---|---|
| protocol framing / lifecycle / transport | `crates/mcp-server-v1/src/{lib,proto,sse}.rs` |
| a tool/resource/prompt fixture shape | `crates/mcp-server-v1/src/fixtures.rs` |

Loop: `make build && make serve &` → `make conformance` → read the failing
check's message → make the smallest fix → re-run, until all-pass.

## CI

`.github/workflows/conformance.yml` runs the suite on every push/PR (it builds
the component, serves it under `wasmtime serve -Skeyvalue`, waits for readiness,
and gates on the empty baseline), alongside host unit tests and the examples
build/smoke-test.

## A note on spec versions

The conformance tool (`@0.1.16`) knows `2025-03-26`, `2025-06-18`, `2025-11-25`,
`draft`, `extension`. The DESIGN's v2 target `2026-07-28` does **not** exist in
any release yet, so v2 is deferred until a conformance release ships it
([DECISIONS.md](../DECISIONS.md) D9).
