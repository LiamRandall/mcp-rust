# mcp-rust — build & conformance loop. No scripting framework (DESIGN §5.3).
#
# The v1 transport is a single WASI 0.2 component built natively with the
# wasm32-wasip2 target (cargo + wit-bindgen + wasm-tools). `wac`/`splicer`/
# `wasm-opt` are available for the composition/middleware/size follow-ups
# described in DECISIONS.md (D6, D8) but are not on the default v1 path.

SPEC_VERSION   ?= 2025-11-25
CONFORMANCE    ?= @modelcontextprotocol/conformance@0.1.16
COMPONENT      := target/wasm32-wasip2/release/mcp_server_v1.wasm
DIST           := dist/server.wasm
URL            ?= http://127.0.0.1:8080/mcp
ADDR           ?= 127.0.0.1:8080

.PHONY: build serve conformance dev doctor size validate test clean

build: $(DIST) validate size

$(DIST): $(shell find crates/mcp-server-v1/src crates/mcp-server-v1/wit -type f) crates/mcp-server-v1/Cargo.toml
	cargo build -p mcp-server-v1 --release --target wasm32-wasip2
	@mkdir -p dist
	cp $(COMPONENT) $(DIST)

validate: $(DIST)
	wasm-tools validate $(DIST)
	@# Guard the platform contract: zero wasmcloud:* / wRPC imports (DESIGN §2.4).
	@! wasm-tools component wit $(DIST) | grep -qiE 'import wasmcloud:' || (echo "ERROR: wasmcloud:* import present" && exit 1)

size: $(DIST)
	@scripts/size-report.sh $(DIST)

serve: $(DIST)
	wasmtime serve -Scli -Shttp -Skeyvalue $(DIST) --addr $(ADDR)

# wasmCloud-native inner loop; host plugins (keyvalue/config) are built in.
dev:
	cd templates/v1-2025-11-25 && wash dev

conformance:
	npx -y $(CONFORMANCE) server \
		--url $(URL) \
		--suite active \
		--spec-version $(SPEC_VERSION) \
		--expected-failures conformance-baseline.yml

test:
	cargo test -p mcp-core -p mcp-derive

doctor:
	@scripts/doctor.sh

clean:
	cargo clean
	rm -rf dist
