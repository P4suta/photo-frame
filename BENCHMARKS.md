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

### cargo-chef multi-stage Dockerfile (Phase 1.5) — adopted

(Initial Phase 1.5 commit deferred this as "not applicable for the
bind-mount dev image". User reversed: ship cargo-chef anyway so we
also gain a slim CLI runtime image for distribution. Subsequent commit
ships it.)

**Multi-stage layout** (Dockerfile):

  chef-base   rust:slim + cargo-binstall + cargo-chef
  planner     `cargo chef prepare` → recipe.json (workspace dep manifest)
  cacher      `cargo chef cook --release` → dependencies compiled
  builder     COPY workspace source → `cargo build --release -p cli`
  runtime     debian:trixie-slim + ca-certificates + the CLI binary
  dev         (unchanged) full tooling for `docker compose run`

`docker-compose.yml` builds `target: dev` by default. The release image
is built via `just docker-build-release` (`docker build --target runtime
-t photo-frame:latest .`).

| Scenario | Runtime image build (s) |
| -------- | ----------------------: |
| Cold (first ever build, all deps compiled at -O3 release) | ~480 s |
| Warm (no source change, full cache hit) | **1.3 s** |
| Warm (app source touched, deps cached) | _expected ~30-60 s_ |
| Final image size | **32 MB** (debian-slim base + 14 MB CLI binary + ca-certs) |

`.dockerignore` is required for the warm path — without it `COPY . .`
in the planner / builder stages picks up writes to `target/`,
`artifacts/`, etc. and invalidates the cargo-chef cache on every
build.

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

## Runtime performance

Separate measurement track for the **image-processing pipeline itself**
(decode → frame → encode), as opposed to the build / test / lint timings
above. Driven by the divan harness in `crates/photo-frame-bench/`
(`just bench`). Every entry is the median of a sample run; the raw
divan output lives under `artifacts/bench/runtime/<UTC>/`.

The fixture matrix is fixed once for the lifetime of the project so
rows across phases stay comparable. The workspace `.gitignore` excludes
image binaries, so the **real-world fixtures are local-only** — on CI
and clean clones, the bench harness logs a one-line warning and runs
the synth-only subset. The synth fixtures alone are enough for
regression detection; the real-world rows in the table below capture
"as measured on the author's machine" snapshots that future runs need
to reproduce by dropping the same files into `examples/`.

| Fixture | Source | Dimensions | EXIF orient. | Notes |
| --- | --- | ---: | :---: | --- |
| `real_z5_landscape_a_24mp` | `examples/IMG_3936.JPG` | 6016×4016 | 1 | Nikon Z 5 native landscape |
| `real_z5_landscape_b_24mp` | `examples/IMG_3939.JPG` | 6016×4016 | 1 | second sample, same camera |
| `real_z5_portrait_rot8_24mp` | `examples/IMG_3940.JPG` | 6016×4016 | 8 | exercises 90° CCW rotation |
| `synth_noise_4mp_2400x1600` | xorshift RGB → JPEG q85 | 2400×1600 | 1 | smartphone-class |
| `synth_noise_12mp_4240x2832` | xorshift RGB → JPEG q85 | 4240×2832 | 1 | mid-range mirrorless |
| `synth_noise_24mp_6016x4016` | xorshift RGB → JPEG q85 | 6016×4016 | 1 | matches Z 5 sensor MP count |
| `synth_noise_panorama_10000x100` | xorshift RGB → JPEG q85 | 10000×100 | 1 | extreme aspect, edge cases |

### Baseline (Phase A, commit `1dcbfdf`)

Raw output: [`artifacts/bench/runtime/20260525T001511Z/`](artifacts/bench/runtime/20260525T001511Z/).
Host: Linux 6.8.0-117-generic x86_64, rustc 1.95.0, cargo 1.95.0,
divan 0.1.21, sample-count=10.

Median wall-clock per stage (ms) and throughput (MP/s, computed against
the **source** pixel count, not the framed canvas):

| Stage \ Fixture | `r_z5_la_a` | `r_z5_la_b` | `r_z5_pt8` | `synth_4mp` | `synth_12mp` | `synth_24mp` | `panorama` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `decode` (ms)            | 268  | 225  | 357  | 86   | 270  | 546  | 20.0 |
| `decode` (MP/s)          | 90.0 | 107  | 67.7 | 44.5 | 44.6 | 44.2 | 49.9 |
| `resize_lanczos3_to_sns` (ms) | 440  | 436  | 439  | 167  | 272  | 439  | 9.9  |
| `frame` (ms, no resize)  | 131  | 131  | 130  | 9.1  | 63.4 | 126  | 2.2  |
| `encode` q92 (ms)        | 819  | 776  | 755  | 227  | 732  | 1423 | 58.1 |
| `pipeline` (ms)          | 1245 | 1143 | 1266 | 333  | 1097 | 2111 | 79.4 |
| `pipeline` (MP/s)        | 19.4 | 21.1 | 19.1 | 11.5 | 10.9 | 11.4 | 12.6 |

Three observations from the baseline that **the plan must take as data,
not as preconception**:

1. **Encode dominates.** On the representative 24 MP real-world fixture
   the JPEG encode at q92 takes 819 ms — **66 %** of the 1.25 s
   pipeline wall-clock. Decode is 22 %, frame (no resize) is 10 %.
   Optimisation priority points first at encode, then decode.

2. **EXIF orientation=8 costs ~30 % at decode.** Comparing the two
   landscape fixtures (decode median 247 ms avg) against the rotated
   portrait (357 ms), orientation rotation is a real cost line, not a
   rounding error. Phase B should split it out for isolated measurement.

3. **Synthetic ≠ real.** The synth noise fixtures decode at
   ~44 MP/s vs the real-world ones at ~90-107 MP/s. Xorshift produces
   maximum-entropy JPEGs that the decoder works harder on. Useful as a
   pessimistic upper bound, but real-world numbers are the ones we
   actually want to move.

### Per-phase deltas (runtime)

After each phase lands a row appends below with new medians on the
same fixture × stage matrix. Negative = faster.

| Phase | Snapshot | 24 MP real `pipeline` | 24 MP real `decode` | 24 MP real `encode` | 24 MP real `frame` | Notes |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| baseline | `1dcbfdf` | 1.25 s | 268 ms | 819 ms | 131 ms | encode = 66 % of pipeline; rotation costs +30 % at decode |

