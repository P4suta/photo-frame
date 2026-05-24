# Benchmarks

Data-driven baseline + per-phase tracking of build / test / lint / wasm / web
times. Every entry is the **median wall-clock** of 3 hyperfine runs (warmup 1
unless noted). All times in seconds.

Measurement script: [`scripts/bench-build.sh`](scripts/bench-build.sh).
Cargo `--timings` HTML reports: [`scripts/timings.sh`](scripts/timings.sh).

## Method

```bash
cargo install --locked hyperfine          # one-time
scripts/bench-build.sh host               # ~3-5 min on a warm cache
scripts/bench-build.sh container          # uses photo-frame-dev:latest image
```

Results land in `artifacts/bench/<UTC>/{summary.md, *.json}`. The numbers in
this file are copied from those summaries by hand at each phase boundary so
the history is reviewable here without diving into JSON.

## Conventions

- **cold** = a `touch crates/photo-frame-decode/src/lib.rs` precedes the run
  (forces incremental compile across the graph)
- **warm** = no source changes; only the cache hit path is measured
- **container** = inside `photo-frame-dev:latest`, using bind-mount workspace
  and the `.cargo-cache-docker` / `target-docker` workaround dirs (Phase 1.6
  removes these in favour of properly-permissioned named volumes)

## Baseline (pre-Phase-0)

Captured against commit `8974cc6` (top of `v2.0-rewrite` at Phase 0 setup
time). Host: Linux 6.8.0-117-generic x86_64, cargo 1.95.0, rustc 1.95.0.

Numbers below are hyperfine medians (3 runs unless noted). Raw JSON sits
in the `artifacts/bench/20260524T125844Z/` directory locally.

### Host (warm incremental cache)

| Command | Baseline (s) | Notes |
| ------- | -----------: | ----- |
| `cargo build --workspace` (cold after touch decode/lib.rs) | **0.62** | 1 run |
| `cargo build --workspace` (warm, no changes) | **0.08** | 3 runs |
| `cargo check --workspace --all-targets` | **0.08** | 3 runs |
| `cargo test --workspace --all-targets --no-fail-fast` | **14.73** | 3 runs — **dominant cost** |
| `cargo clippy --workspace --all-targets -- -D warnings` | **0.10** | 3 runs |
| `wasm-pack build --target web --release` | **3.04** | 2 runs, σ high (cold/warm split) |

**Observation:** with the incremental cache hot, `cargo build` / `clippy`
/ `check` are essentially no-ops. The single dominant cost on host is
`cargo test`, at ~15 s. Phase 1.3 (nextest) and 1.2 (test opt-level=1)
target this directly.

### Container (`photo-frame-dev:latest`)

Measured via `docker run --rm` with bind-mounted workspace and the
`.cargo-cache-docker` / `target-docker` workaround dirs (Phase 1.4 replaces
these). Raw JSON in `artifacts/bench/20260524T130030Z/`.

| Command | Baseline (s) | Notes |
| ------- | -----------: | ----- |
| `just ci` | **18.45** | fmt + lint + test + wasm-build + web-build, all warm |
| `cargo build --workspace` | **0.38** | no-op rebuild (overhead ≈ docker run start + cargo no-op) |
| `cargo test --workspace --all-targets` | **15.45** | default cargo test, dominant cost |
| `cargo clippy --workspace --all-targets -D warnings` | **0.40** | warm cache |

### Docker rebuild

| Scenario | Baseline (s) | Notes |
| -------- | -----------: | ----- |
| `docker compose build dev` (warm, no change) | **1.00** | buildx overhead only |
| `docker compose build dev` (cold) | _not measured_ | trashes cache; expected ≈ 5-8 min (sccache/cargo-chef target ≤2 min) |

**Container observation:** docker run startup overhead is ~0.3-0.5 s, which
inflates every per-step measurement but is negligible against `just ci` at
~18 s. Phase 1.4 + 1.5 reduce the rebuild cost dramatically; Phase 1.3
(nextest) reduces the in-container test step from ~15 s.

## Per-phase deltas

After each phase lands, append a row to the table below with the new
medians and the delta (negative = faster, positive = regression).

| Phase | Snapshot | Host `cargo build` warm | Host `cargo test` | Host `clippy` | Container `just ci` | Docker cold rebuild | Notes |
| ----- | -------- | ----------------------: | ----------------: | ------------: | ------------------: | ------------------: | ----- |
| baseline | `8974cc6` | 0.08 s | 14.73 s | 0.10 s | 18.45 s | 1.00 s\* | \*warm rebuild; cold not measured (cache invalidation cost) |
| Phase 1.2 (profiles) | post-1.2 | 0.08 s | **1.15 s** | 0.10 s | _re-measure_ | — | **-92% on test** — `package."*"` at -O3 on dev+test profile |
| Phase 1.3 (nextest) | post-1.3 | 0.08 s | 1.33 s | 0.10 s | — | — | +180 ms vs cargo test (overhead) but enables CI JUnit / retries / per-test timing |
| Phase 1.4 (docker) | post-1.4 | 0.08 s | 1.33 s | 0.10 s | **5.00 s** | 1.00 s\* | container `just ci` -73% (mostly Phase 1.2 propagating into container); BuildKit cache mounts + cargo-binstall + UID fix |

## Negative results (changes investigated but not shipped)

Data-driven means we also record what *didn't* move the needle, so a
future contributor doesn't waste effort re-investigating the same
optimisation.

### cargo-chef multi-stage Dockerfile (planned Phase 1.5) — not applicable, skipped

cargo-chef precompiles the dependency graph into a cacheable Docker
stage so changes to the application source don't trigger a full
dependency recompile. It's the standard pattern for **production
runtime images** that build & ship a Rust binary.

For our `photo-frame-dev` image the source isn't COPYed in — it's
bind-mounted at container start (`docker compose run -v .:/workspace`).
The dependency cache that matters at iteration time lives in the
`target-cache` named volume, *outside* the image. cargo-chef can't
warm that volume because Docker initialises it lazily on first mount.

Skip until/unless we ship a slim production runtime image of the CLI
(`scratch` or `distroless` base + statically-linked photo-frame
binary). Phase 1.4's BuildKit cache mounts already give us the
registry/git cache without the cargo-chef ceremony.

### mold linker (planned Phase 1.1) — **0% improvement, skipped**

Measured on host with `gcc -fuse-ld=mold` (mold 2.41.0 via mise) vs.
default GNU `ld`. Trigger: `touch crates/photo-frame-cli/src/main.rs`
then `cargo build -p photo-frame-cli`.

| Linker | dev relink (5 runs) | release relink (3 runs) |
| ------ | ------------------: | ----------------------: |
| GNU `ld` (default) | 404.1 ms ± 3.4 | 8.390 s ± 0.059 |
| `mold` 2.41 | 404.7 ms ± 4.9 | 8.392 s ± 0.028 |

The dev binary is 78 MB but the actual link step completes in ≲100 ms
under either linker — cargo / rustc startup overhead dominates the
wall-clock. The release build is dominated by LTO + codegen, not
linking. Conclusion: mold's value (faster linking) does not apply at
photo-frame's current scale; no `.cargo/config.toml` linker entry
shipped. Re-measure if the workspace grows past ~10 crates with much
larger binaries.

Raw JSON: `artifacts/bench/mold_cli_relink.json` (kept locally).

## Targets (success criteria for the overhaul)

| Metric | Baseline → target | Mechanism |
| ------ | ----------------- | --------- |
| Host `cargo build` warm | -50% | mold + split-debuginfo |
| Host `cargo test` | -30% | cargo-nextest + opt-level=1 on test profile |
| Container `just ci` (warm) | -40% | nextest + sccache + better cache mounts |
| Docker rebuild (cold) | -60% | cargo-chef multi-stage |
| GH Actions wall-clock | -50% | parallel jobs (Phase 2) |

These are aspirational; real numbers will land here as each phase completes.
