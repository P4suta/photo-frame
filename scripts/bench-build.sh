#!/usr/bin/env bash
# bench-build.sh — Data-driven measurement of build / test / lint times.
#
# Usage:
#   scripts/bench-build.sh                # all targets (host + container variants)
#   scripts/bench-build.sh host           # host-side cargo / wasm / web only
#   scripts/bench-build.sh container      # Docker-side just ci variants only
#   scripts/bench-build.sh quick          # subset: cargo build + cargo test only
#
# Output:
#   artifacts/bench/<UTC-timestamp>/{summary.md, *.json}
#   Each command's hyperfine JSON sits next to a summary markdown file that
#   pastes the median wall-clock numbers into a compact table.
#
# Conventions:
#   - hyperfine `--warmup 1 --runs 3` for compile-heavy commands (cache-cold
#     warmup, then 3 hot runs whose median we report)
#   - "cold" docker means after `docker buildx prune -a` (caller's
#     responsibility — this script does not destroy caches)
#   - all paths relative to the workspace root; cd here first.

set -Eeuo pipefail

# ── locate workspace root ────────────────────────────────────────────────
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd -P)"
cd "$ROOT"

# ── pick variant ─────────────────────────────────────────────────────────
VARIANT="${1:-all}"
case "$VARIANT" in
  host|container|quick|all) ;;
  *) echo "Usage: $0 [host|container|quick|all]" >&2 ; exit 2 ;;
esac

# ── output dir ───────────────────────────────────────────────────────────
TS="$(date -u +%Y%m%dT%H%M%SZ)"
OUT="artifacts/bench/$TS"
mkdir -p "$OUT"
echo "▶ writing results to $OUT/"

SUMMARY="$OUT/summary.md"
cat >"$SUMMARY" <<EOF
# Bench run $TS

Workspace HEAD: \`$(git rev-parse --short=12 HEAD 2>/dev/null || echo "(not a git repo)")\`
Branch: \`$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "-")\`
Host: \`$(uname -srm)\`
Cargo: \`$(cargo --version 2>/dev/null || echo "-")\`
Rustc: \`$(rustc --version 2>/dev/null || echo "-")\`

| Command | Median (s) | Min (s) | Max (s) | Notes |
| ------- | ---------: | ------: | ------: | ----- |
EOF

# ── hyperfine probe ──────────────────────────────────────────────────────
HYPERFINE="${HYPERFINE:-hyperfine}"
if ! command -v "$HYPERFINE" >/dev/null 2>&1 ; then
  echo "✖ hyperfine not on PATH (try: cargo install --locked hyperfine)" >&2
  exit 1
fi

# Each measurement appends one row to SUMMARY. `bench` is the workhorse.
#   bench <label> <warmup> <runs> -- <cmd...>
bench() {
  local label="$1" warmup="$2" runs="$3" ; shift 3
  # consume the literal `--`
  [[ "${1:-}" == "--" ]] && shift
  local json="$OUT/${label// /_}.json"

  echo "▶ ${label}"
  if ! "$HYPERFINE" \
      --warmup "$warmup" --runs "$runs" \
      --export-json "$json" \
      --shell=bash \
      -- "$*" ; then
    echo "  ⚠ failed; recording as N/A" >&2
    printf '| `%s` | N/A | N/A | N/A | command failed |\n' \
      "$(printf '%s' "$*" | head -c 120)" >>"$SUMMARY"
    return 0
  fi

  # extract median / min / max from the json result
  local median min max
  median="$(jq -r '.results[0].median' "$json")"
  min="$(jq -r '.results[0].min' "$json")"
  max="$(jq -r '.results[0].max' "$json")"
  printf '| `%s` | %.2f | %.2f | %.2f | warmup=%s runs=%s |\n' \
    "$(printf '%s' "$*" | head -c 120)" "$median" "$min" "$max" "$warmup" "$runs" \
    >>"$SUMMARY"
}

# ── host-side measurements ───────────────────────────────────────────────
host_measurements() {
  echo "═══ HOST measurements ═══"

  # cold debug build (touch a source file so incremental misses)
  bench "host_cargo_build_cold"     0 1 -- 'touch crates/photo-frame-decode/src/lib.rs && cargo build --workspace 2>&1 | tail -5'
  # warm debug build (no changes)
  bench "host_cargo_build_warm"     0 3 -- 'cargo build --workspace 2>&1 | tail -5'
  # test (no rebuild required if Cargo.toml unchanged)
  bench "host_cargo_test_workspace" 1 3 -- 'cargo test --workspace --all-targets --no-fail-fast 2>&1 | tail -5'
  # clippy
  bench "host_cargo_clippy"         1 3 -- 'cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3'
  # cargo check (cheaper than build; useful for "type-only loop" speed)
  bench "host_cargo_check"          1 3 -- 'cargo check --workspace --all-targets 2>&1 | tail -3'
  # wasm-pack build
  bench "host_wasm_pack_build"      0 2 -- 'cd crates/photo-frame-wasm && wasm-pack build --target web --release --out-dir www/pkg 2>&1 | tail -3'
}

# ── container-side measurements ──────────────────────────────────────────
container_measurements() {
  echo "═══ CONTAINER measurements ═══"

  local image="photo-frame-dev:latest"
  if ! docker image inspect "$image" >/dev/null 2>&1 ; then
    echo "✖ $image not built. Run: docker compose build dev" >&2
    return 0
  fi

  # Use `docker compose run --rm` so the bench shares the same named
  # volumes (cargo-registry, cargo-git, target-cache, node-cache) as the
  # interactive dev session — measurements reflect what a developer's
  # actual `docker compose run dev <cmd>` would experience.
  #
  # Phase 1.4 removed the older `--mount type=bind` to `.cargo-cache-docker`
  # / `target-docker` workaround dirs; compose's named volumes are now the
  # only cache surface inside the container.
  local DC="docker compose run --rm --no-deps dev"

  bench "docker_just_ci"      1 2 -- "$DC bash -c 'just ci 2>&1 | tail -3'"
  bench "docker_cargo_build"  1 2 -- "$DC bash -c 'cargo build --workspace 2>&1 | tail -3'"
  bench "docker_cargo_test"   1 2 -- "$DC bash -c 'cargo nextest run --workspace --all-targets 2>&1 | tail -3'"
  bench "docker_cargo_clippy" 1 2 -- "$DC bash -c 'cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3'"
}

# ── quick subset ─────────────────────────────────────────────────────────
quick_measurements() {
  echo "═══ QUICK measurements ═══"
  bench "host_cargo_build_warm" 0 3 -- 'cargo build --workspace 2>&1 | tail -3'
  bench "host_cargo_test"       1 3 -- 'cargo test --workspace --all-targets --no-fail-fast 2>&1 | tail -3'
}

case "$VARIANT" in
  host)      host_measurements ;;
  container) container_measurements ;;
  quick)     quick_measurements ;;
  all)       host_measurements ; container_measurements ;;
esac

echo "✔ done. Summary at: $SUMMARY"
echo
cat "$SUMMARY"
