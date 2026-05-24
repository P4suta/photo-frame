#!/usr/bin/env bash
# timings.sh — collect cargo build --timings reports per crate / per profile.
#
# Usage:
#   scripts/timings.sh                # workspace, dev profile
#   scripts/timings.sh release        # workspace, release profile
#   scripts/timings.sh dev wasm32     # wasm32-unknown-unknown target
#
# Output:
#   artifacts/timings/<UTC>/cargo-timing-*.html
#   Each invocation also forces incremental compilation off so the timings
#   reflect a representative full rebuild rather than the (mostly empty)
#   incremental compile delta from the last interactive build.

set -Eeuo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd -P)"
cd "$ROOT"

PROFILE="${1:-dev}"
TARGET="${2:-}"

TS="$(date -u +%Y%m%dT%H%M%SZ)"
OUT="artifacts/timings/$TS"
mkdir -p "$OUT"

echo "▶ profile=$PROFILE target=${TARGET:-host}  →  $OUT/"

# Build command — touch a dep to force compile across the whole graph.
touch crates/photo-frame-decode/src/lib.rs
build_cmd=(cargo build --workspace --timings -p photo-frame --p photo-frame-cli --p photo-frame-wasm)

case "$PROFILE" in
  dev)     ;;
  release) build_cmd+=(--release) ;;
  *) echo "Unknown profile: $PROFILE (use dev|release)" >&2 ; exit 2 ;;
esac

if [[ -n "$TARGET" ]]; then
  build_cmd+=(--target "$TARGET")
fi

CARGO_PROFILE_DEV_INCREMENTAL=false "${build_cmd[@]}" 2>&1 | tail -20

# cargo writes timings to target/cargo-timings/cargo-timing-<TS>.html and a
# stable cargo-timing.html symlink. Copy out the unique-name HTML so we keep
# a history per run.
find target -maxdepth 4 -name 'cargo-timing-*.html' -newer "$OUT" -print0 \
  | xargs -0 -I{} cp {} "$OUT/"

echo "✔ timings collected in $OUT/"
ls -la "$OUT/" || true
