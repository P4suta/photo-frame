# Developing photo-frame

A quick reference for the inner-loop workflows. The full per-phase
history of speed / observability decisions lives in `BENCHMARKS.md`
(measurements) and `docs/EVENTS.md` (observability contract).

## One-shot setup

```bash
# host: install the project-pinned tooling
mise install

# container: build the dev image (includes Rust toolchain + every
# CLI tool the workspace needs)
docker compose build dev

# Optional: install the git hooks (fmt + clippy on commit)
just hooks
```

## Inner loop

```bash
# bacon: background cargo runner with single-key job switch.
# Default job is `clippy-all`. Press `t` for nextest, `d` for doc
# tests, `r` for run.
just dev

# Or one-shot per-task:
just test        # cargo nextest run --workspace + doc tests
just lint        # clippy + cargo-deny + biome + typos
just fmt-check   # cargo fmt + biome format + taplo fmt (all dry-run)
just ci          # full pipeline mirroring CI
```

## Performance work

```bash
# Hyperfine-based wall-clock measurements. Variant: host, container,
# quick, all (default).
just bench-measure quick

# Criterion benchmarks for decode / frame / encode hot paths. HTML
# report at target/criterion/<group>/report/index.html.
just bench
```

Results land in `artifacts/bench/<UTC>/`. `BENCHMARKS.md` carries the
curated history; raw JSON is local-only.

## Observability

The CLI surfaces tracing events as structured JSON via:

```bash
photo-frame input.jpg --log-format json 2>&1 | jq .
```

Every event has a `event_id` field with a dotted name (e.g.
`decode.exif.datetime.exhausted`). The complete catalogue lives in
`docs/EVENTS.md`.

## Error handling

The CLI uses miette for diagnostic rendering. Every error variant
declares a stable code + help string and maps to a `Category` via the
`Categorize` trait:

```bash
photo-frame /dev/null --quiet
# photo_frame::decode::empty_input
#   × processing /dev/null
#   ├─▶ framing
#   ├─▶ decode failed
#   ╰─▶ input is empty (0 bytes)
#   help: Pass a real image file.
echo $?   # 2  (Category::Input)
```

Exit code map (stable contract):

| Category | Exit code | Examples                                |
| -------- | --------- | --------------------------------------- |
| Input    | 2         | empty input, unknown format, bad CLI arg |
| Decode   | 3         | corrupt JPEG, HEIF decode failure        |
| Render   | 4         | (reserved)                              |
| Encode   | 5         | JPEG encoder failure                    |
| Internal | 1         | producer-side invariant breach          |

## Release image

```bash
just docker-build-release
# produces photo-frame:latest (~32 MB) — debian-slim + ca-certs + the CLI
docker run --rm -v $PWD:/data photo-frame /data/input.jpg -o /data/out.jpg
```

## Running tests

```bash
just test        # full workspace (nextest + doc tests)
just e2e         # CLI end-to-end (binary-level)
just bench       # criterion (warm baseline + HTML report)
just cov         # llvm-cov → lcov.info
```

## Updating dependencies

```bash
just outdated   # what's behind?
# `cargo update` for a one-off bump; Renovate auto-PRs the rest.
```

## File / directory map

```
.cargo/              cargo config (alias, linker overrides if any)
.config/nextest.toml nextest profiles (default + ci)
.github/workflows/   ci.yml, pages.yml, bench.yml, security.yml
.dockerignore        keeps cargo-chef cache cold-resistant
artifacts/           local-only; bench / timing reports
BENCHMARKS.md        curated speed history per phase
bacon.toml           bacon job + keybinding config
crates/              workspace members
  photo-frame-types/    Photograph / Pixels / Provenance / Category / Categorize
  photo-frame-decode/   bytes → Photograph; tests/proptest_decode.rs
  photo-frame-frame/    Photograph → framed Pixels
  photo-frame-encode/   Pixels → JPEG bytes
  photo-frame/          facade: pipeline() + re-exports
  photo-frame-cli/      `photo-frame` binary; tests/e2e.rs
  photo-frame-wasm/     wasm-bindgen surface for the browser
  photo-frame-bench/    criterion benchmark suite
deny.toml             cargo-deny config (bans / licences / advisories / sources)
docs/                 EVENTS.md (event catalogue), DEVELOPMENT.md (this file)
Dockerfile            multi-stage: chef-base / planner / cacher / builder / runtime / dev
docker-compose.yml    builds `target: dev` for `docker compose run`
justfile              command surface
mise.toml             host-side tool version pin
scripts/              bench-build.sh, timings.sh
taplo.toml / biome.json / rustfmt.toml / typos.toml   per-formatter config
```
