#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
OUT_DIR="$ROOT/out"
REF_DIR="$ROOT/reference"
mkdir -p "$OUT_DIR"

check_case() {
  local name="$1"
  local cmd_file="$ROOT/${name}.com"
  local out_file="$OUT_DIR/${name}.png"
  local ref_hash_file="$REF_DIR/${name}.sha256"

  cargo run --manifest-path "$ROOT/../../Cargo.toml" --bin overview-export -- \
    --cmd "$cmd_file" \
    --out "$out_file" \
    --width 320 \
    --height 200 >/dev/null

  local actual
  local expected
  actual="$(shasum -a 256 "$out_file" | awk '{print $1}')"
  expected="$(cat "$ref_hash_file")"

  if [[ "$actual" == "$expected" ]]; then
    echo "PASS: ${name} hash matches reference"
    echo "hash: $actual"
  else
    echo "FAIL: ${name} hash mismatch"
    echo "expected: $expected"
    echo "actual:   $actual"
    exit 1
  fi
}

check_case "synthetic_4x4"
check_case "synthetic_4x4_up_neg_y"
check_case "synthetic_4x4_surface"
check_case "synthetic_4x4_surface_up_neg_z"
check_case "synthetic_4x4_le_f64"
check_case "synthetic_4x4_le_f64_surface"
check_case "synthetic_4x4_be_f32"
check_case "synthetic_4x4_be_f32_surface"
check_case "synthetic_4x4_vpoint_oblique"
check_case "synthetic_4x4_vpoint_oblique_surface"
check_case "synthetic_4x4x2_vpoint_plusx"
check_case "synthetic_4x4x2_vpoint_plusx_surface"
check_case "synthetic_1x4x4_surface"
check_case "synthetic_4x1x4_surface"
