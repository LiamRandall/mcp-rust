# Deploying

The output of every build is a single, portable WASI 0.2 component. It runs
unchanged on `wasmtime serve`, the wasmCloud v2 host (`wash`), or any conformant
P2 runtime. Capabilities (outbound HTTP, config, keyvalue) are satisfied by
**built-in host plugins** — there are zero capability providers and no
`wasmcloud:*` / wRPC in the call path (DESIGN §2.4).

## Run locally — wasmtime

```sh
wasmtime serve -Scli -Shttp <component>.wasm --addr 127.0.0.1:8080
# pass config/secrets as env vars:
wasmtime serve -Scli -Shttp --env API_BASE_URL=… --env API_TOKEN=… <component>.wasm
```

The conformance reference server also needs keyvalue: add `-Skeyvalue`.

## Run locally — wasmCloud (`wash dev`)

```sh
cd templates/v1-2025-11-25   # or an example with a wasmcloud.toml
wash dev
```

`wash dev` provides the host plugins (`wasi:http`, `wasi:config`, `wasi:keyvalue`)
in-process and gives a hot-reload inner loop. Conformance/calls work identically
against `wash dev` and `wasmtime serve`.

## Connect an MCP client

Every server speaks **Streamable HTTP**. Point your client (Claude Desktop, an
IDE MCP extension, the MCP Inspector) at the server URL — e.g.
`http://127.0.0.1:8080/`. The tools appear automatically. No SSE or session
configuration is needed for the tool servers (they answer over
`application/json`).

## Production — wasmCloud v2 (runtime-operator CRDs)

Orchestration is the wasmCloud `runtime-operator` via CRDs — **no `wadm`/OAM**.
A ready-to-edit manifest ships at
[`templates/v1-2025-11-25/deploy/workload.yaml`](../templates/v1-2025-11-25/deploy/workload.yaml).

```sh
# 1. Build + publish the component
wash build
wash push ghcr.io/<org>/<server>:0.1.0 build/<server>.wasm

# 2. Register secrets (never inline a value)
cosmonic_set_secret API_TOKEN <value>     # or a Kubernetes Secret

# 3. Apply
kubectl apply -f deploy/                   # or cosmonic_apply_workload
kubectl apply --dry-run=server -f deploy/  # validate against the operator
```

Key points in the `Workload` CR:

- **Expose the HTTP entrypoint** so the ingress routes to it:
  ```yaml
  hostInterfaces:
    - namespace: wasi
      package: http
      interfaces: ["incoming-handler"]
      config: { host: my-server.localhost }
  ```
- **Config** (non-secret) goes in `localResources.environment.config` — surfaced
  to tools via `wasi:config` (the `API_BASE_URL`, timeouts, …).
- **Secrets** go in `localResources.environment.secretFrom` as registered refs —
  never inline values.
- **Outbound is deny-all by default.** List every host your tools dial in
  `localResources.allowedHosts` (e.g. `api.stlouisfed.org` for the FRED example).

## Size

CI reports `server.wasm` size on every build and flags regressions; there is no
hard cap yet (DESIGN §2.2). The size pass is the release profile
(`opt-level="z"`, `lto`, `strip`); component-level `wasm-opt` is a follow-up
([DECISIONS.md](../DECISIONS.md) D8).
