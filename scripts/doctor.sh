#!/usr/bin/env bash
# Verify the Wasm-native toolchain (DESIGN §5.1). Reports missing tools; exits
# non-zero if any required tool is absent.
set -uo pipefail

ok=0
check() { # name  command  required(1/0)
  local name="$1" cmd="$2" req="$3"
  if command -v "$cmd" >/dev/null 2>&1; then
    printf '  ✓ %-12s %s\n' "$name" "$($cmd --version 2>&1 | head -1)"
  else
    if [[ "$req" == "1" ]]; then
      printf '  ✗ %-12s MISSING (required)\n' "$name"; ok=1
    else
      printf '  - %-12s missing (optional)\n' "$name"
    fi
  fi
}

echo "mcp-rust doctor:"
check rustc      rustc       1
check cargo      cargo       1
check wasm-tools wasm-tools  1
check wasmtime   wasmtime    1
check node       node        1
check npx        npx         1
check wac        wac         0
check wasm-opt   wasm-opt    0
check wash       wash        0
check wkg        wkg         0

if rustup target list --installed 2>/dev/null | grep -q wasm32-wasip2; then
  printf '  ✓ %-12s installed\n' "wasm32-wasip2"
else
  printf '  ✗ %-12s MISSING (rustup target add wasm32-wasip2)\n' "wasm32-wasip2"; ok=1
fi

exit $ok
