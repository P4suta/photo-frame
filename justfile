set shell := ["bash", "-eu", "-o", "pipefail", "-c"]
set positional-arguments

# Default: list recipes.
_default:
    @just --list --unsorted

# ── Composite ────────────────────────────────────────────────────────────────

# Run the full CI suite locally (mirrors .github/workflows/ci.yml).
ci: fmt-check lint test wasm-build web-build

# Install lefthook git hooks (run once after clone).
hooks:
    lefthook install

# ── Formatting ───────────────────────────────────────────────────────────────

# Format Rust, TS/JS, TOML in place.
fmt:
    cargo fmt --all
    biome format --write .
    taplo fmt

# Verify formatting without writing changes.
fmt-check:
    cargo fmt --all -- --check
    biome format .
    taplo fmt --check

# ── Linting ──────────────────────────────────────────────────────────────────

# Run all linters with zero tolerance for warnings.
lint: lint-rust lint-deps lint-ts lint-typos

lint-rust:
    cargo clippy --workspace --all-targets -- -D warnings

# Enforce the Pure-Rust dep contract: deny.toml lists C-compiling build deps
# (cc / bindgen / pkg-config / vcpkg / cmake / ...) that may never enter the
# transitive dep tree of any workspace crate.
lint-deps:
    cargo deny --workspace check bans

lint-ts:
    biome lint .

lint-typos:
    typos

# ── Tests ────────────────────────────────────────────────────────────────────

test:
    cargo nextest run --workspace --all-targets
    # Doc tests aren't covered by nextest; run them separately so a doc
    # comment that compiles but doesn't run keeps tripping the gate.
    cargo test --workspace --doc --no-fail-fast

# ── WASM ─────────────────────────────────────────────────────────────────────

wasm-build:
    cd crates/photo-frame-wasm && wasm-pack build --target web --release --out-dir www/pkg

# Build the Vite/SolidJS web bundle on top of the WASM artefact. Mirrors what
# `.github/workflows/pages.yml` runs on push to main, so `just ci` catches
# TypeScript or Vite regressions locally instead of letting Pages discover
# them after the merge.
web-build: wasm-build
    cd crates/photo-frame-wasm/www && bun install --frozen-lockfile && bun run build

wasm-dev: wasm-build
    cd crates/photo-frame-wasm/www && bun install && bun run dev -- --host 0.0.0.0

wasm-preview: wasm-build
    cd crates/photo-frame-wasm/www && bun install && bun run build && bun run preview -- --host 0.0.0.0

# ── CLI ──────────────────────────────────────────────────────────────────────

# Run the CLI with arguments, e.g. `just run examples/sample.jpg -o /tmp/out.jpg`
run *args:
    cargo run -p photo-frame-cli -- "$@"

build-release:
    cargo build -p photo-frame-cli --release
