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

# Mirror the Geist font files from the frame crate into the web bundle's
# public/ directory so Vite serves them at /fonts/Geist/. Canonical source
# of truth stays in `photo-frame-frame/assets/fonts/Geist/`; this is just
# a build-output copy (ignored by git, regenerated on every build).
copy-web-fonts:
    mkdir -p crates/photo-frame-wasm/www/public/fonts/Geist
    cp -p crates/photo-frame-frame/assets/fonts/Geist/. crates/photo-frame-wasm/www/public/fonts/Geist/ -r

# Build the Vite/SolidJS web bundle on top of the WASM artefact. Mirrors what
# `.github/workflows/pages.yml` runs on push to main, so `just ci` catches
# TypeScript or Vite regressions locally instead of letting Pages discover
# them after the merge.
web-build: wasm-build copy-web-fonts
    cd crates/photo-frame-wasm/www && bun install --frozen-lockfile && bun run build

wasm-dev: wasm-build copy-web-fonts
    cd crates/photo-frame-wasm/www && bun install && bun run dev -- --host 0.0.0.0

wasm-preview: wasm-build copy-web-fonts
    cd crates/photo-frame-wasm/www && bun install && bun run build && bun run preview -- --host 0.0.0.0

# ── CLI ──────────────────────────────────────────────────────────────────────

# Run the CLI with arguments, e.g. `just run examples/sample.jpg -o /tmp/out.jpg`
run *args:
    cargo run -p photo-frame-cli -- "$@"

build-release:
    cargo build -p photo-frame-cli --release

# ── Dev inner loop ──────────────────────────────────────────────────────

# Start bacon in clippy-all mode (warnings denied, full workspace).
# Switch jobs with single-letter keys inside bacon: t = test, c =
# clippy-all, d = doc-tests, r = run.
dev:
    bacon

# Watch + run tests on every save. Equivalent to bacon's `test` job.
watch:
    bacon test

# ── Quality / measurement ───────────────────────────────────────────────

# Run hyperfine measurement of the host + container bench matrix and
# print summary. See BENCHMARKS.md for the curated history.
bench-measure variant="all":
    scripts/bench-build.sh {{ variant }}

# Run the divan benchmark suite (pipeline hot paths). Median wall-clock
# and MP/s per stage × per fixture are printed to stdout; the BENCHMARKS.md
# "Runtime performance" section is the curated history. Extra args forward
# to divan directly: `just bench decode --sample-count 30` runs only the
# decode group with 30 samples per fixture.
bench *args:
    cargo bench -p photo-frame-bench --bench pipeline -- "$@"

# Run iai-callgrind instruction-count benches. Requires `valgrind` at
# the OS level (`apt install valgrind`) plus the `iai-callgrind-runner`
# binary from mise.toml. Output goes to target/iai/ and is the basis
# for the runtime-bench CI regression gate.
bench-icount *args:
    cargo bench -p photo-frame-bench --bench icount -- "$@"

# Record a samply CPU profile of a single CLI invocation. Drop the
# resulting JSON onto https://profiler.firefox.com/ to inspect (or
# run `samply load target/profiling/trace.json` to open it locally).
# Requires `samply` (mise installs from cargo:samply) and Linux
# `kernel.perf_event_paranoid <= 2` (the typical default).
profile-pipeline fixture:
    cargo build -p photo-frame-cli --release
    mkdir -p target/profiling
    samply record -o target/profiling/trace.json -- \
        target/release/photo-frame-cli {{ fixture }} -o target/profiling/out.jpg
    @echo ""
    @echo "▶ trace saved to target/profiling/trace.json"
    @echo "▶ open: samply load target/profiling/trace.json"
    @echo "▶ or upload to https://profiler.firefox.com/"

# Render the per-stage tracing span timeline as a flamegraph SVG.
# Compiles the CLI with the opt-in `trace` feature on photo-frame so
# pipeline / decode / frame / encode span enter-exit events flush to
# target/profiling/trace.folded, then turns that into an SVG via
# `inferno-flamegraph` (mise installs from cargo:inferno). The CLI's
# `--profile-trace=PATH` flag wiring lands in Phase E; until then,
# this recipe writes a `.folded` file the user opens manually.
profile-trace fixture:
    @echo "Phase E will wire the --profile-trace=PATH CLI flag."
    @echo "For now: invoke photo_frame::trace::flame_guard from your test driver."
    @false

# End-to-end CLI tests (binary-level subprocess + stdout / stderr /
# exit code assertions). WASM playwright suite lives separately under
# crates/photo-frame-wasm/www/tests/e2e/ (not yet wired in).
e2e:
    cargo nextest run -p photo-frame-cli --test e2e

# Line / branch coverage via cargo-llvm-cov. Output: lcov.info.
cov:
    cargo llvm-cov --workspace --lcov --output-path lcov.info

# Doc generation for the whole workspace (no deps, public surface only).
docs:
    cargo doc --workspace --no-deps --open

# Show outdated workspace deps (root-only so we don't drown in
# transitive churn).
outdated:
    cargo outdated --workspace --root-deps-only

# Verify the rust-version pin in workspace Cargo.toml is satisfiable
# by the actual code (catches accidental edition-2024 / new-stdlib
# regressions on the MSRV path).
msrv:
    cargo msrv verify

# ── Docker images ────────────────────────────────────────────────────────

# Build the slim runtime image for distribution. Drives the multi-stage
# Dockerfile's chef-base → planner → cacher → builder → runtime chain
# via cargo-chef so an app-source change re-uses the cooked dep layer.
docker-build-release:
    DOCKER_BUILDKIT=1 docker build --target runtime -t photo-frame:latest .
    docker image inspect photo-frame:latest --format 'image: {{ "{{" }}.RepoTags{{ "}}" }} size: {{ "{{" }}.Size{{ "}}" }} bytes'
