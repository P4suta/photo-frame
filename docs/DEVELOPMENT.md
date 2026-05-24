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

| Category        | Exit code | Examples                                                          |
| --------------- | --------- | ----------------------------------------------------------------- |
| Input           | 2         | empty input, unknown format, bad CLI arg                          |
| Decode          | 3         | corrupt JPEG, HEIF decode failure                                 |
| Render          | 4         | (reserved)                                                        |
| Encode          | 5         | JPEG encoder failure                                              |
| PartialFailure  | 6         | batch run finished with ≥1 failure under default continue-on-fail |
| Internal        | 1         | producer-side invariant breach                                    |

## Batch processing

```bash
# Process a folder in parallel; failures don't stop the run.
photo-frame photos/*.jpg -o out/ --jobs 8
#   …progress bar…
#   batch summary
#     processed: 998 / 1000  (99.8%)
#     failures:  2
#       photos/bad_a.jpg → input  (0.0s)
#       photos/bad_b.jpg → decode (0.1s)
#     total:     23.4s  (avg 23ms / file, 14.8x speedup over single-thread, 8 jobs)
echo $?   # 6 (PartialFailure) — some outputs missing

# Stop at first failure; exit code reflects the failing input's category.
photo-frame photos/*.jpg -o out/ --strict
```

For the browser, drop multiple files on the drop zone; the UI
switches to a batch view and runs the Worker-hosted `frame_batch`
off the main thread. Manual verification:

```bash
just wasm-dev
# In the browser: drop 5+ JPEGs on the drop zone.
# Expected: each row reports `processing` → `done`; per-row "Download"
# saves the framed JPEG.
# DevTools → Performance: the Main thread stays mostly idle while
# the WASM Worker drives encoding.
```

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
