# Developing photo-frame

Use this guide for routine development tasks.
See `BENCHMARKS.md` for measurements and `docs/EVENTS.md` for event contracts.

## One-shot setup

```bash
# host: install the project-pinned tooling
mise install

# container: build the dev image with Rust and workspace CLI tools
docker compose build dev

# REQUIRED: install the git hooks before contributing
just hooks
```

The pre-commit hook checks staged files with `cargo fmt --check`.
It also runs `biome format`, `taplo fmt --check`, `typos`, and `biome lint`.
The pre-push hook verifies commit signatures.
It runs `just lint-rust`, `just lint-deps`, `just test`, and `just wasm-build`.

Mise pins the local Biome version in `mise.toml`.
CI and the development container request the same version.
Run local checks through `mise x --` so they use the project tool versions.
Run Git commands from a Mise-activated shell so Git hooks also use those versions.
Biome preserves paragraph line boundaries in Markdown.

## Inner loop

Run `just dev` for the background Cargo runner in `clippy-all` mode.
Press `t` for nextest, `d` for doc tests, or `r` for the CLI.
Use `just test`, `just lint`, `just fmt-check`, and `just ci` for one-shot checks.

```bash
# bacon: switch jobs with t for nextest, d for doc tests, or r for the CLI
just dev

# one-shot project checks
mise x -- just test
mise x -- just lint
mise x -- just fmt-check
mise x -- just ci
```

## Performance work

Run `just bench-measure quick` for Hyperfine host measurements.
The recipe accepts `host`, `container`, `quick`, or `all` variants.
Run `just bench` for Divan measurements of decode, frame, and encode paths.
The harness prints results to standard output.
`scripts/bench-build.sh` writes build results beneath `artifacts/bench/<UTC>/`.
`BENCHMARKS.md` records selected measurements, while raw JSON remains local.

```bash
just bench-measure quick
just bench
```

## Observability

Run the CLI with `--log-format json` to print structured event data.

```bash
photo-frame input.jpg --log-format json 2>&1 | jq .
```

Each event includes a dotted `event_id`, such as `decode.exif.datetime.exhausted`.
`docs/EVENTS.md` contains the complete event catalogue.

## Error handling

The CLI renders diagnostics through `miette`.
Each error variant has stable `code` and `help` values.
The `Categorize` trait maps errors to a `Category` for exit-code handling.

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

Empty input returns code 2.
Unknown formats and invalid command-line arguments also return code 2.
Decode errors return code 3 for corrupt `JPEG` or `HEIF` image data.
Code 4 remains reserved for rendering errors.
Encode errors return code 5 for failures such as `JPEG` output errors.
Batch processing returns code 6 after default continuation with failed files.
Code 1 marks an internal error caused by a producer-side invariant breach.

## Batch processing

The `--jobs 8` option processes files in parallel and continues after failures.

```bash
photo-frame photos/*.jpg -o out/ --jobs 8
#   …progress bar…
#   batch summary
#     processed: 998 / 1000  (99.8%)
#     failures:  2
#       photos/bad_a.jpg → input  (0.0s)
#       photos/bad_b.jpg → decode (0.1s)
#     total:     23.4s  (avg 23ms / file, 14.8x speedup over single-thread, 8 jobs)
echo $?   # 6 (PartialFailure) — some outputs missing

# Stop at the first failure and return its category.
photo-frame photos/*.jpg -o out/ --strict
```

For browser use, drop image files onto the drop zone to open batch mode.
The `frame_batch` WebAssembly worker keeps image processing off the main thread.
Run `just wasm-dev`, then drop five or more `JPEG` images onto the drop zone.
Confirm that every row changes from `processing` to `done`.
Select each row's Download control to save its framed image.
Open the browser Performance panel during encoding.
Confirm that the main thread stays idle while the worker encodes images.

```bash
just wasm-dev
```

## Release image

Run `just docker-build-release` to build the `photo-frame:latest` image.
The release image uses Debian slim and contains CA certificates and the CLI.
The image measures approximately 32 MB.

```bash
just docker-build-release
docker run --rm -v $PWD:/data photo-frame /data/input.jpg -o /data/out.jpg
```

## Running tests

`just test` runs workspace nextest tests and documentation tests.
`just e2e` runs CLI subprocess tests.
`just bench` runs Divan measurements.
`just cov` writes `lcov.info` through llvm-cov.

```bash
just test
just e2e
just bench
just cov
```

## Updating dependencies

Run `just outdated` to find dependencies with newer versions.
Use `cargo update` for a one-time lockfile update.
Renovate opens pull requests for routine dependency updates.

```bash
just outdated
# `cargo update` for a one-off bump; Renovate auto-PRs the rest.
```

## Project files

Cargo aliases and linker overrides live in `.cargo/`.
Nextest profiles live in `.config/nextest.toml`.
The active workflows are `ci.yml` and `security.yml`.
Those workflow files live under `.github/workflows/`.
CI deploys the current main run's web bundle after required checks pass.
The `security.yml` workflow runs optional `heif` verification only on manual dispatch.
`.dockerignore` prevents build outputs from invalidating Docker layers.
`artifacts/` holds local timing reports, and `bacon.toml` configures jobs.
`deny.toml` controls dependency bans, licenses, advisories, and sources.
`crates/photo-frame-types` defines `Photograph`, `Pixels`, `Provenance`, and `Category`.
That crate also defines the `Categorize` trait.
`crates/photo-frame-decode` converts encoded bytes into a `Photograph`.
It contains the `tests/proptest_decode.rs` property tests.
`crates/photo-frame-frame` converts a `Photograph` into framed pixels.
`crates/photo-frame-encode` converts pixels into `JPEG` bytes.
`crates/photo-frame` provides the pipeline façade and re-exports.
`crates/photo-frame-cli` provides the `photo-frame` binary and `tests/e2e.rs`.
`crates/photo-frame-wasm` provides the browser-facing wasm-bindgen API.
`crates/photo-frame-bench` contains Divan benchmarks and shared fixtures.
`Dockerfile` defines chef, cache, builder, runtime, and development stages.
`docker-compose.yml` builds the development target by default.
`mise.toml` pins host tools, and `scripts/` holds benchmark commands.
Taplo, Biome, rustfmt, and typos use repository configuration files.
