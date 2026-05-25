set shell := ["bash", "-eu", "-o", "pipefail", "-c"]
set positional-arguments

# Default: list recipes.
_default:
    @just --list --unsorted

# ── Composite ────────────────────────────────────────────────────────────────

# Run the full CI suite locally (mirrors .github/workflows/ci.yml).
ci: fmt-check lint test wasm-build web-build

# Install lefthook git hooks (run once after clone).
#
# Some contributors have a global `core.hooksPath` pointing to a
# personal hooks directory (e.g. `~/.config/git/hooks` for GPG-
# signing enforcement across all repos). lefthook respects that
# global setting and installs into it, which (a) silently shadows
# the contributor's other repos' lefthook configs and (b) needs
# `--reset-hooks-path` to undo — destructive on the global state.
#
# Setting per-repo `core.hooksPath = .git/hooks` first scopes the
# lefthook install to this repo only. The contributor's global
# hooks keep firing for every OTHER repo unchanged; THIS repo
# gets the strict `lefthook.yml` pre-commit / pre-push pipeline.
hooks:
    git config core.hooksPath .git/hooks
    # `--force`: lefthook refuses install when both local + global
    # `core.hooksPath` are set (it can't tell which the contributor
    # wants). We've just set the local one one line above, so the
    # force is safe and unambiguous: install into `.git/hooks/`,
    # ignore the global setting.
    lefthook install --force

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

# Enforce the Pure-Rust dep contract + license / advisory / source gates.
# The four subchecks mirror what the `security.yml` workflow runs as
# separate matrix jobs, but bundling them here means `just lint` (and the
# lefthook pre-push hook that calls it) catches them locally instead of
# letting CI fail. `bans` enforces the C-toolchain banlist (cc, bindgen,
# pkg-config, etc.); `licenses` enforces the allow-list in
# `deny.toml [licenses]`; `advisories` flags known RUSTSEC findings;
# `sources` restricts deps to crates.io + an explicit allow-list.
lint-deps:
    cargo deny --workspace check bans licenses advisories sources

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

# `env -u RUSTUP_TOOLCHAIN` strips the host environment's stable pin
# (mise/devcontainer profile sets RUSTUP_TOOLCHAIN=1.95.0 for the
# rest of the workspace) so rust-toolchain.toml inside the wasm
# crate can elect its nightly pin instead. The nightly is needed
# because `wasm-bindgen-rayon` requires `target-feature=+atomics`,
# which only works with `-Z build-std` (nightly only). All other
# `just` recipes inherit the host pin unchanged.
#
# `CARGO_UNSTABLE_BUILD_STD` belt-and-braces: the same flag lives
# in `crates/photo-frame-wasm/.cargo/config.toml` as `[unstable]
# build-std = [...]`, which works locally but is silently ignored
# on CI runners (suspected interaction with how `rust-toolchain.toml`
# + `rustup default` propagate the nightly selection through
# `wasm-pack`'s cargo invocation). Setting the env var bypasses the
# config-file resolution entirely; nightly cargo honours it
# unconditionally, std gets rebuilt with the per-crate
# `target-feature=+atomics,+bulk-memory,+mutable-globals,+simd128`
# rustflags, and `wasm-bindgen-rayon`'s cfg-guard sees what it
# expects.
wasm-build:
    cd crates/photo-frame-wasm && env -u RUSTFLAGS RUSTUP_TOOLCHAIN=nightly-2026-04-01 CARGO_UNSTABLE_BUILD_STD=panic_abort,std wasm-pack build --target web --release --out-dir www/pkg

# Mirror the Geist font files from the frame crate into the web bundle's
# public/ directory so Vite serves them at /fonts/Geist/. Canonical source
# of truth stays in `photo-frame-frame/assets/fonts/Geist/`; this is just
# a build-output copy (ignored by git, regenerated on every build).
copy-web-fonts:
    mkdir -p crates/photo-frame-wasm/www/public/fonts/Geist
    cp -p crates/photo-frame-frame/assets/fonts/Geist/. crates/photo-frame-wasm/www/public/fonts/Geist/ -r

# Mirror the coi-serviceworker JS into the web bundle's public/
# directory. The service worker masquerades as the COOP/COEP HTTP
# headers SharedArrayBuffer needs on hosts that can't set headers
# directly (GitHub Pages). Loaded as the very first <script> in
# index.html so it registers before any module that touches WASM.
copy-coi-sw:
    mkdir -p crates/photo-frame-wasm/www/public
    cp -p crates/photo-frame-wasm/www/node_modules/coi-serviceworker/coi-serviceworker.js \
          crates/photo-frame-wasm/www/public/coi-serviceworker.js

# Build the Vite/SolidJS web bundle on top of the WASM artefact. Mirrors what
# `.github/workflows/pages.yml` runs on push to main, so `just ci` catches
# TypeScript or Vite regressions locally instead of letting Pages discover
# them after the merge.
# Regenerate Panda CSS's `styled-system/` package (typed tokens,
# recipes, css() / patterns helpers) from `panda.config.ts` + the
# config-side modules under `panda/`. Idempotent — also chained
# from `bun run dev` / `bun run build` / `postinstall`, so the
# manual recipe exists mainly for when you've just edited
# panda/*.ts and want fresh types in your editor before saving.
panda-codegen:
    cd crates/photo-frame-wasm/www && bun run panda codegen

web-build: wasm-build copy-web-fonts
    cd crates/photo-frame-wasm/www && bun install --frozen-lockfile
    just copy-coi-sw
    cd crates/photo-frame-wasm/www && bun run build

# Run the vitest suite (pure-function + component tests).
# Routed through the host's bun (matches `web-build` /
# `wasm-dev`) so it doesn't depend on the container's
# named-volume node_modules being populated.
web-test:
    cd crates/photo-frame-wasm/www && bun install --frozen-lockfile
    cd crates/photo-frame-wasm/www && bun run test

wasm-dev: wasm-build copy-web-fonts
    cd crates/photo-frame-wasm/www && bun install
    just copy-coi-sw
    cd crates/photo-frame-wasm/www && bun run dev -- --host 0.0.0.0

wasm-preview: wasm-build copy-web-fonts
    cd crates/photo-frame-wasm/www && bun install
    just copy-coi-sw
    cd crates/photo-frame-wasm/www && bun run build && bun run preview -- --host 0.0.0.0

# ── CLI ──────────────────────────────────────────────────────────────────────

# Run the CLI with arguments, e.g. `just run samples/scratch/sample.jpg -o /tmp/out.jpg`
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
# Compiles the CLI with the opt-in `trace` feature so the existing
# pipeline / decode / frame / encode tracing spans flush to a
# `.folded` file via tracing-flame, then turns that into an SVG via
# `inferno-flamegraph` (mise installs from cargo:inferno).
profile-trace fixture:
    cargo build -p photo-frame-cli --release --features trace
    mkdir -p target/profiling
    ./target/release/photo-frame \
        --profile-trace=target/profiling/trace.folded \
        {{ fixture }} -o target/profiling/out.jpg
    inferno-flamegraph < target/profiling/trace.folded > target/profiling/flamegraph.svg
    @echo ""
    @echo "▶ folded:    target/profiling/trace.folded"
    @echo "▶ svg:       target/profiling/flamegraph.svg (open in any browser)"

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
