# Deploy (wasmCloud v2)

Orchestration is the wasmCloud `runtime-operator` via CRDs — **no `wadm`/OAM**
(that was the v1 path). DESIGN §8.3.

```sh
# 1. Build + publish the component
wash build
wash push ghcr.io/liamrandall/mcp-server-v1:0.1.0 build/mcp_server_v1.wasm

# 2. Register the upstream API token as a secret ref (never inline it)
cosmonic_set_secret API_TOKEN <value>          # or a Kubernetes Secret

# 3. Apply
kubectl apply -f deploy/                        # or cosmonic_apply_workload
kubectl apply --dry-run=server -f deploy/       # validate against the operator
```

Config (`API_BASE_URL`, …) and secrets (`API_TOKEN`) reach the component through
`wasi:config`, backed by the workload's `localResources.environment`
(Kubernetes `ConfigMap`/`Secret` equivalents). Nothing is baked into the
`.wasm`.

The component's other imports — `wasi:http` (outbound), `wasi:keyvalue`,
`wasi:clocks`, `wasi:random` — are satisfied by **built-in host plugins**
(in-process). There are **zero capability providers** and no `wasmcloud:*` /
wRPC in the call path.
