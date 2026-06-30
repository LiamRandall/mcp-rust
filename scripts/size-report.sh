#!/usr/bin/env bash
# Report component size and the delta vs the previous build. Never fails the
# build — size is tracked and optimized, not gated (DESIGN §2.2 / DECISIONS D2).
set -euo pipefail

WASM="${1:?usage: size-report.sh <path-to.wasm>}"
STATE_DIR="$(dirname "$WASM")/.size"
STATE_FILE="$STATE_DIR/$(basename "$WASM").last"
mkdir -p "$STATE_DIR"

size=$(wc -c < "$WASM" | tr -d ' ')
human=$(awk -v b="$size" 'BEGIN{printf "%.1f KB", b/1024}')

if [[ -f "$STATE_FILE" ]]; then
  prev=$(cat "$STATE_FILE")
  delta=$((size - prev))
  sign=$([[ $delta -ge 0 ]] && echo "+" || echo "")
  dhuman=$(awk -v d="$delta" 'BEGIN{printf "%.1f KB", d/1024}')
  echo "size: $human ($size bytes)  delta: ${sign}${delta} bytes (${sign}${dhuman})"
else
  echo "size: $human ($size bytes)  delta: (no previous build)"
fi

echo "$size" > "$STATE_FILE"
