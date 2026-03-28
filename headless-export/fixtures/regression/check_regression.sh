#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
OUT_DIR="$ROOT/out"
REF_DIR="$ROOT/reference"
mkdir -p "$OUT_DIR"

cargo run --manifest-path "$ROOT/../../Cargo.toml" --bin overview-export -- \
  --cmd "$ROOT/synthetic_4x4.com" \
  --out "$OUT_DIR/synthetic_4x4.png" \
  --width 320 \
  --height 200 >/dev/null

actual="$(shasum -a 256 "$OUT_DIR/synthetic_4x4.png" | awk '{print $1}')"
expected="$(cat "$REF_DIR/synthetic_4x4.sha256")"

if [[ "$actual" == "$expected" ]]; then
  echo "PASS: regression image hash matches reference"
  echo "hash: $actual"
else
  echo "FAIL: regression image hash mismatch"
  echo "expected: $expected"
  echo "actual:   $actual"
  exit 1
fi
