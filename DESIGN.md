# mcp-rust — Design & Build Spec for Claude Code

> A Rust scaffold that turns an OpenAPI spec into a **strictly conformant MCP server**, compiled to a **single tiny WebAssembly component** for **CNCF wasmCloud v2** (`wash-runtime`, WASI P2, no capability providers) — built and verified entirely with **Wasm-native tooling** (`wasm32-wasip2`, `wit-bindgen`, `wasm-tools`, `wac`, `splicer`, `wasmtime`, `wash`).
>
> This document is the build spec. Hand it to Claude Code and run the phased plan in §11. It optimizes for four things, in priority order: **(1) strict conformance, (2) tiny output size, (3) minimal LLM tokens/effort to author a server, (4) zero non-essential dependencies.**

---

## 1. Goals & non-goals

**Goals**
- One OpenAPI spec in → one conformant MCP server `.wasm` out, with the LLM editing the smallest possible surface.
- Two template variants, sharing one tool-authoring model:
  - **`v1` → MCP `2025-11-25`** (stateful lifecycle, Streamable HTTP, `initialize` handshake).
  - **`v2` → MCP `2026-07-28`** (stateless core, per-request `_meta`, no session).
- A CI + local loop that runs the **official conformance suite** and lets an agent iterate until every test passes.
- Output that is sandboxed, OCI-distributable, and deployable with `wash`.

**Non-goals**
- No reliance on a third-party MCP framework (no `rmcp`, no `wasmcp` runtime components, no Tokio). We define our own minimal WIT world and compose with Wasm-native tools.
- No SDK lock-in: the only "framework" we ship is first-party code in this repo.
- Not a general agent runtime — this builds API-bridging tool servers.

---

## 2. Hard constraints

### 2.1 Dependency policy (strict)
The LLM-authored and framework code may depend **only** on:

| Allowed | Why it's unavoidable |
|---|---|
| `wit-bindgen` (guest macro crate) | Canonical Bytecode Alliance guest bindings; not a framework, just the ABI glue. |
| `serde` + `serde_json` | JSON-RPC and JSON Schema are JSON; hand-rolling a serializer costs more size and far more LLM tokens than it saves. This is the **only** runtime data dependency permitted. |
| First-party crates in this repo (`mcp-derive`, `mcp-core`) | Our own code; the macro that removes LLM boilerplate. |

**Everything else is forbidden** without an explicit note in `DECISIONS.md`. No HTTP client crate (use `wasi:http/outgoing-handler` directly), no async runtime, no router, no schema crate (`schemars` is **out** — the `#[tool]` macro generates JSON Schema itself).

> **`wstd` exception (optional):** the upstream wasmCloud templates use `wstd` (a thin std-like async wrapper over `wasi:http`/`wasi:io`). It is wasmCloud-idiomatic but adds size. Default to raw `wasi:http` bindings in `mcp-core` to hit the size budget; allow `wstd` only if the transport's incoming handler becomes unwieldy, and record it in `DECISIONS.md`. It is the **only** additional dependency that may be admitted, and never in the tools component.

### 2.2 Size budget (optimize, don't gate — yet)
- **Optimize aggressively for small**, but **no hard size limit for now.** CI measures `server.wasm` size on every build and flags regressions vs the previous build; it does **not** fail on an absolute threshold. Revisit a hard cap once a real baseline exists.
- Release profile is mandatory (see §5.2). `wasm-opt -Oz` is part of the build. Treat size as a first-class, tracked metric and prefer the smaller option whenever ergonomics are equal.

### 2.3 Conformance
- `make conformance` must exit 0 against the official suite for the template's target spec version, with an **empty** `conformance-baseline.yml` (no permitted known-failures in a shipped template).

### 2.4 wasmCloud v2 platform constraints (target is v2.0 only)
This template targets **wasmCloud v2.0 exclusively** (`wash` 2.0+, `wash-runtime`). Do not design for v1 concepts.

- **No capability providers.** They were removed in v2.0. Capabilities are satisfied by **in-process host plugins** that ship with `wash-runtime` (`wasi:keyvalue`, `wasi:config`, `wasi:logging`, `wasi:blobstore`, `wasmcloud:messaging`) and, where persistent state/sockets are needed, by **services**. The server imports only standard **WASI P2 (0.2)** interfaces — no `wasmcloud:*` host-specific imports beyond those built-ins, no wRPC, no NATS in the call path.
- **Pure, portable WASI P2 component.** The output is a standards-compliant WASI 0.2 component that runs unchanged on `wash-runtime`, `wasmtime serve`, or any conformant P2 runtime. `wasm_target = "wasm32-wasip2"`. (P3 lands later; stay on P2 until `wash-runtime` ships P3.)
- **Explicit networking.** All capability calls are in-process (nanoseconds). Nothing is implicitly routed over the network.
- **Deployment = Kubernetes operator + CRDs.** Orchestration is the `runtime-operator` with Custom Resource Definitions, managed via `kubectl`/Helm/ArgoCD. **No `wadm`/OAM manifests** (that was the v1 path). Runtime config and secrets come from Kubernetes `ConfigMap`/`Secret`, surfaced to the component as env vars / `wasi:config`.
- **Structure mirrors the upstream wasmCloud templates** (e.g. `templates/http-hello-world`): a `wasmcloud.toml` per template, `wash new --subfolder`, and a `wash dev` inner loop. See [wasmCloud/wasmCloud/templates](https://github.com/wasmCloud/wasmCloud/tree/main/templates).

---

## 3. Architecture

Two components, composed at build time. The split is the whole ergonomic bet: **the LLM only ever touches the tools component**; the transport component is fixed per spec version and never edited by the model.

```
        wasi:http/incoming-handler (wasmtime serve / wasmCloud)
                         │
        ┌────────────────▼─────────────────┐
        │   mcp-transport  (FIXED)          │   JSON-RPC framing, MCP lifecycle,
        │   - implements MCP protocol       │   tools/list aggregation, error
        │   - imports `mcp:tools/tools`     │   envelopes, capabilities, version.
        └────────────────┬─────────────────┘   v1: stateful + wasi:keyvalue
                         │ (WIT edge)            v2: stateless, per-request _meta
        ┌────────────────▼─────────────────┐
        │   mcp-tools  (LLM-AUTHORED)       │   one Rust fn per tool; schema
        │   - exports `mcp:tools/tools`     │   derived by #[tool]; calls upstream
        │   - imports wasi:http/outgoing    │   API via wasi:http.
        └───────────────────────────────────┘
```

**Composition:** `wac plug` satisfies the transport's import of `mcp:tools/tools` with the tools component's export → `server.wasm`. Pure Wasm-native, no glue code.

**Middleware (optional):** cross-cutting concerns — bearer-token auth, request logging, rate limiting — are **injected with `splicer`** on the `wasi:http` edge, not written into either component. Each middleware is its own tiny component spliced into the graph; the LLM never edits the transport to add auth. This keeps the protocol core and the policy layer independently versionable.

Why this shape wins on all four priorities: conformance logic lives in one audited component reused across every generated server; the tools component carries no protocol code so it stays tiny and version-agnostic; the LLM's surface is two functions; and there's no framework dependency because the "framework" is just the fixed transport `.wasm` we publish.

**wasmCloud v2 fit.** The composed `server.wasm` is a plain WASI P2 component. On `wash-runtime` its `wasi:http/incoming-handler` export is served by the host; its imports (`wasi:http/outgoing-handler`, `wasi:config`, and in v1 `wasi:keyvalue`) are satisfied by **built-in host plugins**, not capability providers — all in-process. Upstream API base URLs and tokens are read from `wasi:config` (backed by Kubernetes `ConfigMap`/`Secret`), never hardcoded. Nothing in the design imports a `wasmcloud:*` provider interface or touches wRPC.

---

## 4. The WIT contract

The entire boundary the LLM implements. Keep it this small.

`wit/tools.wit`
```wit
package mcp:tools@0.1.0;

interface tools {
  /// A tool advertised in tools/list. `input-schema` is a JSON Schema object (string).
  record tool-def {
    name: string,
    description: string,
    input-schema: string,
  }

  /// Result of tools/call. `content` is MCP content JSON; `is-error` maps to isError.
  record tool-result {
    content: string,   // JSON array of MCP content blocks
    is-error: bool,
  }

  list-tools: func() -> list<tool-def>;
  call-tool: func(name: string, arguments-json: string) -> tool-result;
}

world tools-component {
  import wasi:http/outgoing-handler@0.2.0;
  import wasi:http/types@0.2.0;
  import wasi:config/store@0.2.0;        // base URLs + tokens from host plugin (ConfigMap/Secret)
  export tools;
}
```

`wit/transport.wit` (fixed; differs per version only in lifecycle imports)
```wit
package mcp:server@0.1.0;

world transport {
  import mcp:tools/tools@0.1.0;
  // v1 only: session persistence for the stateful lifecycle.
  // Satisfied by the built-in wasi:keyvalue HOST PLUGIN in wasmCloud v2 — not a capability provider.
  import wasi:keyvalue/store@0.2.0;        // (omit entirely in mcp:server v2 / MCP 2026-07-28)
  export wasi:http/incoming-handler@0.2.0;
}
```

The transport translates MCP JSON-RPC (`initialize`, `tools/list`, `tools/call`, ping, etc.) to/from these two functions. **All spec compliance lives here**, so the model that writes tools cannot get protocol framing wrong.

**Session state (v1 only).** A tools-only server holds almost nothing per session — just `session_id → {protocol_version, capabilities, created_at}`. Because v2 components are ephemeral (fresh instance per request, multiple replicas), this must **not** live in component memory. The transport accesses it through a small first-party `SessionStore` trait with two implementations: a `wasi:keyvalue` backend (default; correct under scaling, TTL-reaped) and an in-memory backend (dev/test only, single instance). v2 (MCP 2026-07-28) is stateless and drops the trait entirely. See §13.1 for the full rationale; an optional stateless **signed session token** (no store at all) is a documented fast-follow.

---

## 5. Tooling & build

### 5.1 Required tools (all Wasm-native / Bytecode Alliance)
Pin versions in `.tool-versions` and check them in `make doctor`.

| Tool | Role |
|---|---|
| `rustup` + `wasm32-wasip2` target | Native component output from `rustc`. |
| `wit-bindgen` (crate) | Guest bindings from WIT. |
| `wasm-tools` | `component new`/inspect; size + validity checks. |
| `wac` ([bytecodealliance/wac](https://github.com/bytecodealliance/wac)) | Compose tools⊕transport via `wac plug`. |
| `splicer` ([ejrgilbert/splicer](https://github.com/ejrgilbert/splicer)) | Inject middleware components on WIT edges (optional). |
| `binaryen` (`wasm-opt`) | `-Oz` size pass. |
| `wasmtime` | `wasmtime serve` for local conformance runs (portability check). |
| `wash` (**2.0+**) | wasmCloud v2 `wash-runtime`: `wash new --subfolder`, `wash dev` inner loop, `wash build`, host plugins (`wasi:config`/`wasi:keyvalue`) for local runs. |
| Node (npx) | Runs `@modelcontextprotocol/conformance` only — not a build dep. |

> Note: `cargo-component` is being superseded by the native `wasm32-wasip2` target. Prefer the native target + `wit-bindgen` + `wasm-tools`. Use `cargo component` only if a custom-WIT edge case forces it, and record it in `DECISIONS.md`.

### 5.2 Cargo release profile (mandatory, in every component crate)
```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

### 5.3 Build pipeline (`Makefile` — no scripting framework)
```make
build: build-tools build-transport compose opt validate size
build-tools:      ; cargo build -p mcp-tools     --release --target wasm32-wasip2
build-transport:  ; cargo build -p mcp-transport --release --target wasm32-wasip2
compose:          ; wac plug target/wasm32-wasip2/release/mcp_transport.wasm \
                      --plug target/wasm32-wasip2/release/mcp_tools.wasm \
                      -o dist/server.wasm
opt:              ; wasm-opt -Oz dist/server.wasm -o dist/server.wasm
validate:         ; wasm-tools validate dist/server.wasm
size:             ; scripts/size-report.sh dist/server.wasm   # prints size + delta vs last build; never fails
serve:            ; wasmtime serve -Scli -Shttp -Skeyvalue dist/server.wasm   # mcp:server v2: drop -Skeyvalue
dev:              ; wash dev                                                  # wasmCloud-native inner loop (host plugins built in)
conformance:      ; npx @modelcontextprotocol/conformance server \
                      --url http://localhost:8080/mcp \
                      --spec-version $(SPEC_VERSION) \
                      --expected-failures conformance-baseline.yml
deploy:           ; kubectl apply -f deploy/            # runtime-operator CRDs (see §8.3)
```
`SPEC_VERSION` = `2025-11-25` (v1) or `2026-07-28` (v2). `make serve` (wasmtime) is the CI portability path; `make dev` (`wash dev`) is the wasmCloud-native loop — both serve the same component, and conformance runs identically against either.

`wasmcloud.toml` per template (mirrors upstream templates):
```toml
name = "mcp-server"
language = "rust"
type = "component"
version = "0.1.0"

[component]
wit_world = "transport"
wasm_target = "wasm32-wasip2"
```

---

## 6. Tool-authoring ergonomics (the token-minimizing surface)

The LLM writes **one file per tool**, ~15–25 lines, no protocol code. The first-party `#[tool]` proc-macro (crate `mcp-derive`) generates the JSON Schema from the argument struct and registers the tool — so the model writes Rust types, never JSON Schema, and the two can't drift.

`src/tools/get_pet.rs`
```rust
use mcp_core::{tool, http, ToolError, Json};

/// Fetch a pet by its ID.            // ← becomes the MCP tool description verbatim
#[tool]
pub fn get_pet(
    /// The pet's unique identifier.  // ← becomes the property description
    pet_id: u64,
) -> Result<Json, ToolError> {
    // Base URL + auth come from wasi:config (ConfigMap/Secret) — never hardcoded.
    let base = http::config("API_BASE_URL")?;          // mcp_core wraps wasi:config
    // wasi:http outbound — no HTTP crate. Bearer token auto-attached from config if present.
    let resp = http::get(&format!("{base}/pets/{pet_id}"))?;
    Ok(resp.json()?)                  // returned JSON → MCP text content automatically
}
```

What the macro does (so the LLM never has to): emits the `tool-def` (name = fn name, description = doc comment, `input-schema` = JSON Schema derived from args + doc comments), and routes `call-tool(name, args)` to a typed call by deserializing `arguments-json` into the arg struct. `mcp-core` provides `http` (a ~one-screen wrapper over `wasi:http/outgoing-handler`), `ToolError` (maps to `isError: true` + message), and `Json`.

**Rule for the model:** to add a tool, create one file in `src/tools/` and add one `mod` line. Never touch the transport, the WIT, or the macro.

---

## 7. Repository layout

```
mcp-rust/
├─ DESIGN.md                 # this file
├─ DECISIONS.md              # any deviation from §2 dependency policy, with reason
├─ AGENTS.md                 # instructions for the end-user LLM (see §9)
├─ wit/                      # tools.wit (shared) + transport.wit (per version)
├─ crates/
│  ├─ mcp-core/              # first-party: http wrapper, ToolError, Json, runtime glue
│  ├─ mcp-derive/            # first-party: #[tool] proc-macro (schema gen + dispatch)
│  ├─ mcp-transport-v1/      # FIXED: 2025-11-25 stateful transport
│  └─ mcp-transport-v2/      # FIXED: 2026-07-28 stateless transport
├─ templates/
│  ├─ v1-2025-11-25/         # `wash new --subfolder` template → user's server (uses transport-v1)
│  │  ├─ src/tools/_template.rs
│  │  ├─ src/tools/mod.rs
│  │  ├─ wasmcloud.toml       # mirrors upstream wasmCloud templates
│  │  ├─ deploy/              # runtime-operator CRDs: Component workload + ConfigMap/Secret
│  │  ├─ Makefile  Cargo.toml  conformance-baseline.yml
│  └─ v2-2026-07-28/         # same, targets transport-v2 (stateless; no wasi:keyvalue)
├─ generator/                # OpenAPI → src/tools/*.rs (one file per operation)
├─ tests/conformance/        # fixtures, baseline, harness wiring
└─ .github/workflows/conformance.yml
```

Only `transport-v1` vs `transport-v2` differ between versions. The tools component, `mcp-core`, `mcp-derive`, generator, and per-tool files are **identical** across both — that's the design paying off.

---

## 8. CI & the conformance loop

### 8.1 GitHub Action (`.github/workflows/conformance.yml`)
```yaml
name: conformance
on: [push, pull_request]
jobs:
  matrix:
    strategy: { matrix: { version: [v1-2025-11-25, v2-2026-07-28] } }
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: make -C templates/${{ matrix.version }} build
      - run: scripts/size-report.sh templates/${{ matrix.version }}/dist/server.wasm  # report only, non-failing
      - run: |
          wasmtime serve -Scli -Shttp templates/${{ matrix.version }}/dist/server.wasm &
          timeout 15 bash -c 'until curl -s localhost:8080/mcp; do sleep 0.5; done'
      - uses: modelcontextprotocol/conformance@v0.1.16
        with:
          mode: server
          url: http://localhost:8080/mcp
          # spec-version inferred from the server; baseline must be empty to merge
          expected-failures: templates/${{ matrix.version }}/conformance-baseline.yml
```

### 8.2 The agent loop (how Claude Code iterates to green)
A deterministic loop the model runs locally until conformance passes:

1. `make build && make serve &` then `make conformance SPEC_VERSION=<v>`.
2. Read `results/server-*/checks.json`. Each entry has a scenario, a check, pass/fail, and a message.
3. Map failure → owner: protocol/framing/lifecycle → `mcp-transport-v*`; tool schema/result shape → `mcp-derive` or the offending `src/tools/*.rs`.
4. Make the **smallest** fix. Never silence a failure by adding it to `conformance-baseline.yml` — the baseline ships empty.
5. Re-run. Repeat until `checks.json` is all-pass. Size is reported each build (and minimized), but is not a gate.
6. Run `--suite draft` against v2 as an early-warning signal for the 2026 line.

Stop condition: every scenario passes for the target version with an empty baseline. (Size is tracked and optimized, not gated.)

### 8.3 wasmCloud v2 deployment (`deploy/`)
No `wadm`/OAM. Deployment is the `runtime-operator` via CRDs and stock Kubernetes objects:

- **Component workload CR** — references the published `server.wasm` (OCI), declares the HTTP trigger, and binds host plugins it imports (`wasi:http`, `wasi:config`, and `wasi:keyvalue` for v1). All plugins are in-process; nothing is a capability provider.
- **`ConfigMap`** — non-secret config (`API_BASE_URL`, timeouts) surfaced to the component via `wasi:config`.
- **`Secret`** — upstream API token, surfaced via `wasi:config`; never baked into the `.wasm`.
- Apply with `kubectl apply -f deploy/` (or Helm/ArgoCD). State lives in Kubernetes `etcd`; scaling/observability use standard k8s tooling.

If persistent state or outbound sockets are ever needed beyond request scope, attach a **service** (the workload's `localhost`) rather than reintroducing a provider. Publish the component with `wash build` + push to OCI before applying the CR.

---

## 9. `AGENTS.md` — instructions for the end-user LLM (ships in the template)

This is the content Claude Code should generate into each template so that *a future LLM building a real server from an OpenAPI spec* needs minimal context. Keep it terse and imperative:

```md
# Building an MCP server from an OpenAPI spec

You edit ONE thing: files in `src/tools/`. Never edit `wit/`, the transport crate, or `mcp-derive`.

## Add a tool
1. Copy `src/tools/_template.rs` to `src/tools/<operation_id>.rs`.
2. Set the doc comment (becomes the tool description), the args (become the input schema),
   and the body (call the upstream API via `http::get/post/...`).
3. Add `pub mod <operation_id>;` to `src/tools/mod.rs`.

## Rules
- One operation = one file. No shared state between tools.
- Return `Ok(Json)` for success, `Err(ToolError::msg(..))` for tool errors. Never panic.
- Do not add dependencies. Use `mcp_core::http` for all network calls.
- Read base URLs/tokens via `http::config("KEY")` (wasi:config) — never hardcode them.
- Names must be snake_case and unique; keep them < 64 chars.

## Verify (loop until green)
    make build && make serve &
    make conformance
Read results/server-*/checks.json; fix failing tools; repeat. Do not edit the baseline.

## Generate from OpenAPI (bulk start)
    cargo run -p generator -- path/to/openapi.yaml --out src/tools/
Then refine descriptions/auth per tool and run the conformance loop.
```

---

## 10. OpenAPI generator (`generator/`)

A small Rust binary (runs on the host, not in Wasm) that parses an OpenAPI 3.x document and emits one `src/tools/<operationId>.rs` per operation using the §6 pattern:
- `description` ← `summary`/`description`; args ← path/query/body params with doc comments from the schema; body ← `http::<method>` against `servers[0].url` + path, mapping the typed response back to `Json`.
- Flags mirror the proven Cosmonic generator surface: `--include-methods`, `--include-tools <regex>`, `--skip-long-tool-names`, `--oauth2*`.
- The generator only produces the **declarative** per-tool files — it never writes protocol code, because there is none to write.

This is the "least tokens" endgame: the generator does the bulk pass, and the LLM only refines. Reuse logic/lessons from `cosmonic-labs/openapi2mcp` but emit the Rust-component shape above instead of the TS template.

---

## 11. Build plan for Claude Code (phased, test-driven)

Run these phases in order. Each ends with a green gate before proceeding.

**Phase 0 — Scaffolding & doctor.** Create the repo layout (§7), `.tool-versions`, `make doctor` that checks every tool in §5.1. Gate: `make doctor` passes.

**Phase 1 — WIT + core.** Write `wit/tools.wit` and `wit/transport.wit` (§4). Build `mcp-core` (`http` over `wasi:http/outgoing-handler`, `ToolError`, `Json`) and `mcp-derive` (`#[tool]` → schema + dispatch). Unit-test schema generation against hand-written expected JSON Schemas. Gate: macro tests pass; both crates build for `wasm32-wasip2`.

**Phase 2 — Transport v1 (2025-11-25).** Implement the stateful transport: `initialize` handshake + capabilities + protocol version, `tools/list` (aggregating `list-tools`), `tools/call` (→ `call-tool`), ping, error envelopes, Streamable HTTP framing, session via `wasi:keyvalue`. Provide a 1–2 tool example. Gate: `make build` produces `dist/server.wasm`, `wasm-tools validate` passes, size < 512 KB.

**Phase 3 — Conformance to green (v1).** Wire `make conformance` + the §8.2 loop. Iterate until **all** `2025-11-25` server scenarios pass with an empty baseline. Gate: conformance exit 0; size gate holds.

**Phase 4 — Transport v2 (2026-07-28).** Fork transport into the stateless model: per-request `_meta`, no `initialize` session, no `wasi:keyvalue`. Reuse the *same* tools component unchanged. Gate: v2 `make conformance --spec-version 2026-07-28` exit 0; size gate holds.

**Phase 5 — Generator + middleware.** Build `generator/` (§10). Add an optional auth middleware component and a `splicer` recipe to inject it on the `wasi:http` edge; document toggling it. Gate: generating from a sample OpenAPI (e.g. petstore) + conformance loop reaches green with no hand edits to transport.

**Phase 6 — wasmCloud v2 packaging.** Add `wasmcloud.toml` (§5.3) and a `deploy/` CRD set (§8.3) to each template. Verify the `wash dev` loop serves the component and that `make conformance` passes against `wash dev` exactly as against `wasmtime serve`. Confirm `wasi:config`/`wasi:keyvalue` resolve via built-in host plugins (no providers). Gate: `wash dev` + conformance green; `kubectl apply --dry-run` validates the CRDs.

**Phase 7 — CI + docs.** Land `.github/workflows/conformance.yml` (matrix over v1/v2), write `AGENTS.md` (§9) into both templates, and `DECISIONS.md`. Gate: CI green on both versions; `README` quick-start (`wash new --subfolder` → `wash dev` → conformance) verified end to end.

---

## 12. Acceptance criteria

- `templates/v1-2025-11-25` and `templates/v2-2026-07-28` each build to a single `server.wasm` that passes the **entire** official conformance server suite for its version with an empty baseline.
- Composed output is **aggressively size-optimized** (size measured and tracked in CI; no hard cap yet), produced only with `wasm32-wasip2` + `wit-bindgen` + `wasm-tools` + `wac` (+ optional `splicer`) + `wasm-opt`.
- Runtime dependencies limited to `wit-bindgen`, `serde`/`serde_json`, and first-party crates; any exception recorded in `DECISIONS.md`.
- Adding a tool = create one file + one `mod` line; no protocol code touched.
- `cargo run -p generator -- <openapi> --out src/tools/` produces a buildable tool set that reaches conformance-green after refinement only.
- The same tools component compiles unchanged against both transport versions.
- Runs on wasmCloud v2 (`wash-runtime`) as a pure WASI P2 component: imports satisfied only by built-in host plugins (`wasi:http`, `wasi:config`, v1 `wasi:keyvalue`), **zero capability providers**, no `wasmcloud:*` or wRPC in the call path.
- Deploys via `runtime-operator` CRDs in `deploy/` (`kubectl apply` clean); config/secrets injected through `wasi:config`, never embedded. No `wadm`/OAM artifacts anywhere.
- `make conformance` passes identically against `wash dev` and `wasmtime serve`.

---

## 13. Decisions (RESOLVED — see `DECISIONS.md`)

### 13.1 v1 session store — **`wasi:keyvalue` default, behind a `SessionStore` trait**
A tools-only server's per-session state is tiny (`session_id → {protocol_version, capabilities, created_at}`). It must not live in component memory: v2 components are ephemeral (per-request instances, multiple replicas), so in-memory state is lost across requests and replicas.

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| **A. `wasi:keyvalue` host plugin** | Correct under per-request instantiation + scaling; in-process in v2 (no provider/wRPC); survives restarts; ships built-in | Per-request (de)serialize hop; needs TTL + key hygiene to bound growth | **Default** |
| **B. In-component memory** | Zero deps, fastest, simplest | Only correct for a single never-recycled instance; breaks under scaling/ephemerality | **Dev/test impl only**, behind the trait |
| **C. Stateless signed session token** | No store at all; scales trivially; survives any replica | Adds HMAC code (`wasi:random` key); longer session id; no server-side revoke | **Fast-follow option** if v1 must be operationally stateless |

Decision: ship **A** as the default with a small first-party `SessionStore` trait; provide **B** for the hermetic local loop; keep the stored record minimal so the v2 stateless transport is a deletion, not a rewrite; document **C** as an opt-in.

### 13.2 Size — **optimize hard, no gate yet** (per §2.2)
Keep ergonomics first and size small; CI measures + flags regressions but enforces no absolute cap until a real baseline exists. `serde_json` stays; revisit a minimal first-party JSON path only if size later demands it.

### 13.3 Middleware — **off by default**
Auth/logging/rate-limit middleware components are **not** spliced in by default. One documented `splicer` command enables them. (Per-spec MCP OAuth bearer validation may alternatively live in the transport; default ships with neither active.)

### 13.4 Transport crates — **dual, separate crates**
Keep `transport-v1` and `transport-v2` as independent crates (not one crate behind a feature flag), so each conformance target and its test surface stay clean and independently buildable.

### 13.5 `wstd` vs raw `wasi:http` — **raw bindings (recommended)**
Default to hand-rolled `wasi:http` bindings in `mcp-core` for minimum size and zero extra deps. `wstd` stays a documented, `DECISIONS.md`-gated escape hatch for the transport's incoming handler only, never in the tools component.
