#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
OUT_DIR="$ROOT/out"
REF_DIR="$ROOT/reference"
METRICS_FILE="$OUT_DIR/semantic_metrics.txt"

# Default thresholds intentionally allow small rendering jitter while still
# detecting meaningful image drift. They can be overridden from CI or local env.
SEM_MAX_MEAN_ERROR="${SEM_MAX_MEAN_ERROR:-0.75}"
SEM_MAX_RMS_ERROR="${SEM_MAX_RMS_ERROR:-2.5}"
SEM_MAX_CHANGED_RATIO="${SEM_MAX_CHANGED_RATIO:-0.005}"
SEM_CHANGED_THRESHOLD="${SEM_CHANGED_THRESHOLD:-8}"

mkdir -p "$OUT_DIR"
rm -f "$METRICS_FILE"

echo "Running semantic checks with thresholds:"
echo "  SEM_MAX_MEAN_ERROR=$SEM_MAX_MEAN_ERROR"
echo "  SEM_MAX_RMS_ERROR=$SEM_MAX_RMS_ERROR"
echo "  SEM_MAX_CHANGED_RATIO=$SEM_MAX_CHANGED_RATIO"
echo "  SEM_CHANGED_THRESHOLD=$SEM_CHANGED_THRESHOLD"

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

  local semantic_output
  semantic_output="$(cargo run --manifest-path "$ROOT/../../Cargo.toml" --bin overview-export-semantic-check -- \
    --label "$name" \
    --actual "$out_file" \
    --reference "$REF_DIR/${name}.png" \
    --max-mean-error "$SEM_MAX_MEAN_ERROR" \
    --max-rms-error "$SEM_MAX_RMS_ERROR" \
    --max-changed-ratio "$SEM_MAX_CHANGED_RATIO" \
    --changed-threshold "$SEM_CHANGED_THRESHOLD")"

  echo "$semantic_output"
  echo "$semantic_output" | grep '^SEMANTIC:' >> "$METRICS_FILE"

  echo "PASS: ${name} semantic check within thresholds"
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

echo "Wrote semantic metrics summary to: $METRICS_FILE"
