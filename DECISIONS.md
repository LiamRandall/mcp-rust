# Decisions log — mcp-rust

Records resolved design decisions and any deviations from the dependency policy in `DESIGN.md` §2.1. Append new entries; don't rewrite history.

---

## D1 — v1 session store: `wasi:keyvalue` default, behind a `SessionStore` trait
**Status:** Accepted · **Date:** 2026-06-29

A tools-only MCP server's per-session state is tiny: `session_id → {protocol_version, capabilities, created_at}`. wasmCloud v2 components are ephemeral (fresh instance per request, multiple replicas), so in-component memory is **not** a safe place for it.

- **Default:** `wasi:keyvalue` host plugin (in-process in v2, no capability provider), keyed by `Mcp-Session-Id`, with a TTL to reap abandoned sessions.
- **Dev/test:** an in-memory `SessionStore` impl behind the same trait for a hermetic, fast local loop (single instance only).
- **Fast-follow option:** a stateless **signed session token** (HMAC via `wasi:random`) that encodes version+capabilities into the id, eliminating storage entirely. Adopt only if v1 must be operationally stateless.
- Keep the stored record minimal so migrating to the v2 stateless transport (MCP 2026-07-28) is a deletion, not a rewrite.

**Rejected:** in-component memory as the default (incorrect under scaling/ephemerality).

---

## D2 — Size: optimize aggressively, no hard gate yet
**Status:** Accepted · **Date:** 2026-06-29

Prioritize ergonomics and small output, but do **not** enforce an absolute size cap in CI yet. CI measures `server.wasm` and flags regressions vs the previous build; revisit a hard cap once a real baseline exists. `serde_json` stays; consider a minimal first-party JSON path only if size later demands it. Release profile + `wasm-opt -Oz` remain mandatory.

---

## D3 — Middleware: off by default
**Status:** Accepted · **Date:** 2026-06-29

Auth/logging/rate-limit middleware components are not spliced into the graph by default. A single documented `splicer` command enables them. MCP OAuth bearer validation may alternatively live in the transport; the shipped default activates neither.

---

## D4 — Dual, separate transport crates
**Status:** Accepted · **Date:** 2026-06-29

`transport-v1` (MCP 2025-11-25, stateful) and `transport-v2` (MCP 2026-07-28, stateless) are independent crates, not one crate behind a feature flag. Keeps each conformance target and its tests clean and independently buildable. The tools component, `mcp-core`, and `mcp-derive` are shared and version-agnostic.

---

## D5 — Bindings: raw `wasi:http`, not `wstd`
**Status:** Accepted · **Date:** 2026-06-29

`mcp-core` uses hand-rolled `wasi:http` bindings for minimum size and zero extra deps. `wstd` is a gated escape hatch for the transport's incoming handler only (never the tools component) and requires a new entry here if adopted.

---

## D6 — v1 ships as a single self-contained component (not the two-component wac-plug split, yet)
**Status:** Accepted · **Date:** 2026-06-30

DESIGN §3 specifies two components (`mcp-transport` ⊕ `mcp-tools`) composed with `wac plug`. The shipped v1 (`crates/mcp-server-v1`) is instead a **single** WASI 0.2 component that implements the full protocol and carries the example/fixture tools in-process.

**Why:** reaching *full* conformance green requires server→client requests (`sampling/createMessage`, `elicitation/create`) issued mid-tool-call. The tool handler must drive the **live HTTP SSE response stream** and then block on a **cross-request** correlation (the client delivers its reply on a separate POST). Doing that across a `wac`-plugged WIT edge needs a bidirectional host-callback interface plus shared state spanning two components — far more surface than a single component, for no conformance gain. The `#[tool]` ergonomic surface and the `mcp-core`/`mcp-derive` schema-gen machinery are preserved and unit-tested; the two-component split + host-callback WIT edge is a documented follow-up. `wac`/`splicer` remain available for that work and for middleware.

**Rejected:** forcing the two-component split now (high complexity, zero conformance benefit, risk to the green gate).

---

## D7 — Conformance scope: the full active server suite for 2025-11-25, empty baseline
**Status:** Accepted · **Date:** 2026-06-30

The official `@modelcontextprotocol/conformance@0.1.16` **active** server suite has no capability gating — every scenario runs and fails if unimplemented. For `--spec-version 2025-11-25` that is **30 scenarios** spanning initialize, ping, logging, completion, the 12 named test tools (incl. sampling/elicitation callbacks), resources (list/read/templates/subscribe/unsubscribe), prompts (list/get variants), the SEP-1034/1330 elicitation checks, multi-stream POST, and DNS-rebinding protection.

The v1 transport therefore ships the reference **resource/prompt/completion fixtures** the suite expects, in addition to tools. For a real OpenAPI server these fixtures are replaced/dropped; they are template content, not protocol. Result: **39/39 checks pass, empty baseline** (`conformance-baseline.yml`).

---

## D8 — `wasm-opt` is skipped on the component; size pass is the release profile
**Status:** Accepted · **Date:** 2026-06-30

DESIGN §2.2/§5.3 put `wasm-opt -Oz` in the pipeline. Binaryen does not yet support the **component model** ([binaryen#6728](https://github.com/WebAssembly/binaryen/issues/6728)) — `wasm-opt` rejects the component the `wasm32-wasip2` target emits. The size pass is therefore the mandatory release profile (`opt-level="z"`, `lto`, `codegen-units=1`, `panic="abort"`, `strip`); CI measures and reports size (non-gating, D2). Component baseline today: ~211 KB. Re-introducing a core-module `wasm-opt` step (opt the core module before componentization) is a documented follow-up.

---

## D9 — v2 (MCP 2026-07-28) deferred
**Status:** Accepted · **Date:** 2026-06-30

No released conformance tooling knows spec version `2026-07-28` (the suite's max date version is `2025-11-25`; valid versions are `2025-03-26`, `2025-06-18`, `2025-11-25`, `draft`, `extension`). A v2 transport cannot be conformance-validated today, so v2 is **deferred**: `templates/v2-2026-07-28/` is a placeholder. Revisit when a conformance release ships `2026-07-28`. This supersedes the v2 scope in DESIGN §1/§11 for now.

---

## D10 — v1 transport is effectively stateless under the SessionStore contract
**Status:** Accepted · **Date:** 2026-06-30 · refines [D1]

Per D1 the per-session record is minimal. In practice the conformance flow needs only that the server **emits** an `Mcp-Session-Id` on `initialize` (so stateful clients, e.g. `server-sse-multiple-streams`, observe one) — no request depends on server-side session lookup. The v1 transport therefore runs effectively stateless: it issues a session id but does not require a store for sessions. The shared `wasi:keyvalue` store is used instead for **server→client request correlation** (sampling/elicitation): the SSE handler polls for the client's reply, which arrives on a separate POST instance. Under `wasmtime serve -S keyvalue` and the wasmCloud keyvalue plugin alike, that store is process-shared, which is what makes the correlation work.

---

## Dependency-policy deviations
_None. Runtime deps are exactly `wit-bindgen`, `serde`/`serde_json`, and first-party crates (DESIGN §2.1). `wstd` was **not** needed — the transport uses raw `wasi:http`/`wasi:io` bindings (D5)._
