# Benchmarks

This document records baseline and per-phase build, test, and lint timings.
It also covers WebAssembly and web timings.
Each entry reports the **median wall-clock** of three hyperfine runs.
One warmup precedes the three runs unless noted.
All times are in seconds.

Measurement script: [`scripts/bench-build.sh`](scripts/bench-build.sh).
Cargo `--timings` HTML reports: [`scripts/timings.sh`](scripts/timings.sh).

## Method

```bash
cargo install --locked hyperfine          # one-time
scripts/bench-build.sh host               # ~3-5 min on a warm cache
scripts/bench-build.sh container          # uses photo-frame-dev:latest image
```

The scripts write results to `artifacts/bench/<UTC>/{summary.md, *.json}`.
This file records selected values from those summaries at each phase boundary.
Readers can review the history without opening the JSON files.

## Conventions

**Cold** runs begin with `touch crates/photo-frame-decode/src/lib.rs`.
That command forces incremental compilation across the graph.
**Warm** runs leave source unchanged and measure a cache hit.
**Container** runs use `photo-frame-dev:latest` and a bind-mounted workspace.
The `.cargo-cache-docker` and `target-docker` directories provide workarounds.
Phase 1.6 replaces them with named volumes that use corrected permissions.

## Baseline before phase 0

Commit `8974cc6` marks the baseline from Phase 0 setup in `v2.0-rewrite`.
The host ran Linux 6.8.0-117-generic on x86_64.
It used Cargo 1.95.0 and rustc 1.95.0.

The numbers below are hyperfine medians from three runs unless noted.
Raw JSON results are in `artifacts/bench/20260524T125844Z/`.

### Host with a warm incremental cache

Cold `cargo build --workspace` took **0.62 s** in one run.
The run followed `touch crates/photo-frame-decode/src/lib.rs`.
Warm `cargo build --workspace` took **0.08 s** across three unchanged-source runs.
`cargo check --workspace --all-targets` took **0.08 s** across three runs.
`cargo test --workspace --all-targets --no-fail-fast` took **14.73 s**.
That result is the dominant host cost across three runs.
`cargo clippy --workspace --all-targets -- -D warnings` took **0.10 s**.
The clippy result covers three runs.
`wasm-pack build --target web --release` took **3.04 s** across two runs.
The cold and warm split produced high standard deviation.

With a hot incremental cache, `cargo build`, `clippy`, and `check` do little work.
Host tests remain the largest cost at ~15 s.
Phase 1.3 adds nextest to reduce this cost.
Phase 1.2 sets test `opt-level=1` to reduce it.

### Container measurements with `photo-frame-dev:latest`

The measurements used `docker run --rm` with the bind-mounted workspace.
They also used `.cargo-cache-docker` and `target-docker` workaround directories.
Phase 1.4 replaced those directories.
Raw JSON results are in `artifacts/bench/20260524T130030Z/`.

Warm-cache `just ci` took **18.45 s**.
It ran fmt, lint, test, wasm-build, and web-build.
`cargo build --workspace` took **0.38 s** for a no-op rebuild.
That time includes `docker run` startup and Cargo overhead.
Default `cargo test --workspace --all-targets` took **15.45 s**.
Tests were the dominant cost.
Warm-cache `cargo clippy --workspace --all-targets -D warnings` took **0.40 s**.

### Docker rebuild

Warm `docker compose build dev` took **1.00 s** with no changes.
That result reflects buildx overhead only.
The cold build lacks a timing because it invalidates the cache.
The original cold-build estimate was ≈ 5–8 min.
The sccache and cargo-chef target was ≤ 2 min.

`docker run` startup adds ~0.3–0.5 s to each step.
That overhead is small next to the ~18 s `just ci` run.
Phases 1.4 and 1.5 reduce rebuild time.
Phase 1.3 reduces the container test step from ~15 s.

## Per-phase deltas

Record each phase's medians and delta after it lands.
A negative delta means faster execution.
A positive delta means a regression.

The `8974cc6` baseline measured a 0.08 s warm host build.
Host tests took 14.73 s, and clippy took 0.10 s.
Container `just ci` took 18.45 s.
The warm Docker rebuild took 1.00 s.
The cold rebuild lacks a timing because it would invalidate the cache.
The post-1.2 snapshot measured a 0.08 s warm host build.
Host tests took **1.15 s**, and clippy took 0.10 s.
Container `just ci` and the cold Docker rebuild need new measurements.
The test time improved **92 %** with `package."*"` at -O3 in dev and test profiles.
The post-1.3 snapshot measured a 0.08 s warm host build.
Host tests took 1.33 s, and clippy took 0.10 s.
Container `just ci` and the cold Docker rebuild need new measurements.
Nextest added 180 ms over `cargo test`.
It enabled CI JUnit output, retries, and per-test timing.
The post-1.4 snapshot measured a 0.08 s warm host build.
Host tests took 1.33 s, and clippy took 0.10 s.
Container `just ci` took **5.00 s**.
The warm Docker rebuild took 1.00 s.
Container `just ci` improved by 73 % as Phase 1.2 reached the container.
The image used BuildKit cache mounts, cargo-binstall, and a user ID fix.

## Results without measured improvement

This record keeps experiments without measured improvement.
It helps contributors avoid repeating them.

### Cargo-chef runtime image in phase 1.5

The initial Phase 1.5 commit deferred this image.
It called the bind-mount dev image `not applicable`.
The project later adopted cargo-chef for a slim command-line runtime image.
The next commit shipped the image.

The Dockerfile uses these stages:

```text
chef-base   rust:slim + cargo-binstall + cargo-chef
planner     `cargo chef prepare` → recipe.json (workspace dep manifest)
cacher      `cargo chef cook --release` → dependencies compiled
builder     COPY workspace source → `cargo build --release -p cli`
runtime     debian:trixie-slim + ca-certificates + the CLI binary
dev         (unchanged) full tooling for `docker compose run`
```

`docker-compose.yml` builds `target: dev` by default.
Run `just docker-build-release` to build the release image.
The equivalent command is `docker build --target runtime -t photo-frame:latest .`.

The first cold runtime image build took ~480 s with dependencies compiled at -O3.
The warm build took **1.3 s** with unchanged source and a full cache hit.
Cached dependencies should yield about 30–60 s for warm builds.
The estimate applies after app-source changes.
The final image is **32 megabytes**.
It contains a debian-slim base, a 14-megabyte command-line binary, and CA certificates.

The `.dockerignore` file preserves the warm path.
Without it, `COPY . .` includes `target/`, `artifacts/`, and other build outputs.
Those files invalidate the cargo-chef cache on each build.

### Mold linker in planned phase 1.1, skipped after 0 % improvement

The host comparison used `gcc -fuse-ld=mold` and `GNU ld`.
Mold 2.41.0 came from mise.
The test touched `crates/photo-frame-cli/src/main.rs`.
It then ran `cargo build -p photo-frame-cli`.

`GNU ld` relinked dev in 404.1 ms ± 3.4 across five runs.
It relinked release in 8.390 s ± 0.059 across three runs.
Mold 2.41 relinked dev in 404.7 ms ± 4.9 across five runs.
It relinked release in 8.392 s ± 0.028 across three runs.

The dev binary is 78 megabytes.
Both linkers finish in ≲100 ms.
Cargo and rustc startup dominate elapsed time.
Link-time optimization and code generation dominate release builds.
This result doesn't support a linker entry in `.cargo/config.toml` at this scale.
Measure again if the workspace grows beyond ~10 crates with much larger binaries.

Raw JSON results are in `artifacts/bench/mold_cli_relink.json` and remain local.

## Performance targets

The target is 50 % less warm host `cargo build` time with mold and split debug info.
The target is 30 % less host `cargo test` time with cargo-nextest and test `opt-level=1`.
The target is 40 % less warm container `just ci` time.
Nextest and better cache mounts support this target.
The target is 60 % less cold Docker rebuild time with cargo-chef.
The target is 50 % less GitHub Actions time with Phase 2 parallel jobs.

These targets show direction, not guaranteed results.
Record measured results here as each phase completes.

## Runtime measurements

This track measures the image pipeline from decode through frame and encode.
It covers different timings from the build, test, and lint results earlier here.
The `divan` harness lives in `crates/photo-frame-bench/`.
Run it with `just bench`.
Each entry is a sample-run median.
Raw `divan` output is under `artifacts/bench/runtime/<UTC>/`.

The fixture matrix stays fixed so measurements remain comparable across phases.
The workspace `.gitignore` excludes image files, so real-world fixtures stay local.
CI and clean clones log one warning line and run only synthetic fixtures.
Synthetic fixtures detect regressions.
The real-image rows record measurements from one development machine.
Future runs can reproduce them by placing the same files in `samples/bench/`.

Fixture `real_z5_landscape_a_24mp` uses `samples/bench/IMG_3936.JPG`.
It's a native landscape image from a Nikon Z 5.
Its dimensions are 6016×4016 pixels.
Its orientation metadata value is 1.
Fixture `real_z5_landscape_b_24mp` uses `samples/bench/IMG_3939.JPG`.
It's a second sample from the same camera.
Its dimensions are 6016×4016 pixels.
Its orientation metadata value is 1.
Fixture `real_z5_portrait_rot8_24mp` uses `samples/bench/IMG_3940.JPG`.
Its dimensions are 6016×4016 pixels.
Its orientation metadata value is 8.
It exercises 90-degree counterclockwise rotation.
Fixture `synth_noise_4mp_2400x1600` uses xorshift red, green, and blue pixels.
It encodes the pixels as `JPEG` at `q85`.
Its dimensions are 2400×1600 pixels.
Its orientation metadata value is 1.
It represents a phone-class image.
Fixture `synth_noise_12mp_4240x2832` uses xorshift red, green, and blue pixels.
It encodes the pixels as `JPEG` at `q85`.
Its dimensions are 4240×2832 pixels.
Its orientation metadata value is 1.
It represents a mid-range mirrorless image.
Fixture `synth_noise_24mp_6016x4016` uses xorshift red, green, and blue pixels.
It encodes the pixels as `JPEG` at `q85`.
Its dimensions are 6016×4016 pixels.
Its orientation metadata value is 1.
Its dimensions match the Nikon Z 5 sensor's megapixel count.
Fixture `synth_noise_panorama_10000x100` uses xorshift red, green, and blue pixels.
It encodes the pixels as `JPEG` at `q85`.
Its dimensions are 10000×100 pixels.
Its orientation metadata value is 1.
It covers extreme aspect ratios.

### Phase A baseline from commit `1dcbfdf`

The raw output is in `artifacts/bench/runtime/20260525T001511Z/`.
The host ran Linux 6.8.0-117-generic on x86_64.
It used rustc 1.95.0, Cargo 1.95.0, and divan 0.1.21.
The sample count was 10.

The measurements report median stage times in milliseconds.
They report throughput in megapixels per second.
Throughput uses the **source** pixel count, not the framed canvas.

Use this sequence for the measurements below.
The first fixtures are `r_z5_la_a`, `r_z5_la_b`, and `r_z5_pt8`.
The next fixtures are `synth_4mp`, `synth_12mp`, and `synth_24mp`.
The final fixture is `panorama`.
In that order, decode times for the first four were 268, 225, 357, and 86 ms.
The final three decode times were 270, 546, and 20.0 ms.
Decode rates for the first four were 90.0, 107, 67.7, and 44.5 megapixels/s.
The final three decode rates were 44.6, 44.2, and 49.9 megapixels/s.
The first four `resize_lanczos3_to_sns` times were 440, 436, 439, and 167 ms.
The final three resize times were 272, 439, and 9.9 ms.
Frame times without resizing were 131, 131, 130, and 9.1 ms for the first four.
The final three frame times were 63.4, 126, and 2.2 ms.
Encoding at `q92` took 819, 776, 755, and 227 ms for the first four fixtures.
The final three encoding times were 732, 1423, and 58.1 ms.
The full `pipeline` took 1245, 1143, 1266, and 333 ms for the first four.
The final three pipeline times were 1097, 2111, and 79.4 ms.
Pipeline rates for the first four were 19.4, 21.1, 19.1, and 11.5 MP/s.
The final three pipeline rates were 10.9, 11.4, and 12.6 MP/s.

The baseline supports three observations.
The plan must use these data rather than a preconception.

### Encoding dominates

The representative 24-megapixel fixture encoded at `q92` in 819 ms.
Encoding used **66 %** of the 1.25 s pipeline time.
Decode accounts for 22 %.
Frame composition without resizing accounts for 10 %.
Focus on encoding first, then decoding.

### Orientation 8 adds ~30 % to decode time

The two landscape fixtures average 247 ms for decoding.
The rotated portrait takes 357 ms.
Rotation adds a measurable cost.
Phase B should isolate the rotation cost.

### Synthetic fixtures differ from real images

Synthetic noise fixtures decode at ~44 megapixels/s.
Real-image fixtures decode at ~90–107 megapixels/s.
Xorshift creates high-entropy data that increases decode work.
Use synthetic fixtures to test a pessimistic upper bound.
Use real-image results to track the project's performance goals.

### Runtime deltas by phase

Record each phase's medians in the same fixture-by-stage matrix.
A negative delta means faster execution.

The `1dcbfdf` baseline pipeline took 1.25 s on a 24 MP real image.
Decode took 268 ms, encoding took 819 ms, and frame composition took 131 ms.
Encoding used 66 % of pipeline time.
Rotation added 30 % to decode time.

The post-C1 pipeline took **1.17 s**, down 6 %.
Decode took 270 ms, within measurement noise.
Encoding took 806 ms, down 1.5 %.
Frame composition took **92 ms**, down 30 %.
dhat reported total allocation falling from 535 MB to 439 MB.
The 97 MB reduction is about one 92 MB four-channel pixel buffer.
Peak allocation fell from 336 MB to 243 MB.

The post-D1 pipeline took **1.11 s**, down 11 % from baseline.
Decode took **217 ms**, down 19 %.
Encoding took 788 ms, down 2 % within measurement noise.
Frame composition took 94 ms.
dhat reported total allocation falling from 439 MB to 353 MB.
The `zune` decoder used less memory than `image-crate` on these images.
They use the Joint Photographic Experts Group (JPEG) format.
Peak allocation remained at 243 MB.
The orientation-8 case improved 9 %.
The inverse discrete cosine transform wasn't on the critical path.
Landscape decoding improved 23 %.
The vectorized `YCbCr→RGB` path in `zune` drove most of the acceleration.

The post-D2 default pipeline took 1.10 s, unchanged because it doesn't resize.
Decode took 217 ms, encoding took 788 ms, and frame composition took 91 ms.
On 24 MP images, resizing fell from 438 ms to 113 ms, down 74 % or about fourfold.
The `sns` pipeline preset took **0.47 s end to end**.
The run used `--preset sns` on a real 24 MP Nikon Z 5 image.
An estimate put image-crate resizing at about 0.80 s on the same path.
The `resize_fir_lanczos3_to_sns` benchmark reached 213 MP/s.
The image-crate result was 55 MP/s.
The change used a rayon worker pool and vectorized Lanczos3 processing per row.
The WebAssembly build passed.
The `fast_image_resize` library falls back to one thread on `wasm32`.

The post-C4 pipeline took **1.02 s**, down 18 % from baseline and 7 % from D2.
Decode took 216 ms, encoding took 787 ms, and frame composition took **18 ms**.
Frame time fell 86 % from baseline and 80 % from D2.
The single-pass, row-parallel `build_canvas_with_photo` replaced two passes.
Those passes used `from_pixel` and `imageops::replace`.
Rayon assigns each canvas row to a worker.
The worker fills the background, copies photo bytes, then fills the remaining background.
Compose throughput rose from 184 MP/s to **1.33 GP/s**, about sevenfold.
Panorama frame time fell from 1.79 ms to 0.30 ms, about sixfold.
The change didn't increase memory use.
`vec![0; n]` initializes memory less expensively than `from_pixel` did.

The post-D3 pipeline took **662 ms**, down 47 % from baseline and 35 % from C4.
Decode took 216 ms, encoding took **424 ms**, and frame composition took 18 ms.
Encoding fell 48 % from baseline and 46 % from C4.
The change replaced `image::codecs::jpeg::JpegEncoder` with `jpeg_encoder::Encoder`.
The `jpeg-encoder` crate uses vectorized discrete cosine transform and Huffman coding.
It uses AVX2 on x86_64.
At `quality=92`, the swap halves encoding time while retaining 4:4:4 chroma.
Automatic 4:2:0 subsampling applies only below `q<90`.
The measured gain came from vectorized encoding, not a format change.
For the `sns` preset at `q78`, `pipeline_sns` fell from 374 ms to **314 ms**.
The run used automatic 4:2:0 subsampling and improved by 16 %.
The gain is smaller because the `sns` preset already uses chroma subsampling.
The encode crate dropped its `image` dependency.
It now uses `jpeg-encoder` and the existing `photo-frame-types::Pixels` type.
This removes one large transitive dependency from the runtime tree.

### Baseline through D3 on a 24-megapixel real Z 5 landscape

The decode stage fell from 268 ms to 216 ms, down 19 %.
Frame time fell from 131 ms to **18 ms**, down 86 %.
Encoding at `q92` fell from 819 ms to **424 ms**, down 48 %.
The default pipeline fell from **1245 ms** to **662 ms**, down 47 %.
That's about twice the wall-clock speed.
The `sns` pipeline fell from **~1500–1800 ms** to **314 ms**, about 80 %.
That's about five times the wall-clock speed.

For the same input, dhat reported 535 MB total allocation at baseline.
Post-D3 total allocation was **≤ 353 MB**, down 34 %.
Peak allocation remained unchanged from D1 onward.

Encoding takes 424 ms of the 662 ms default pipeline, or **64 %**.
It remains the largest stage, while decoding takes 33 %.
Further encoding gains could lower Standard quality to `q88`.
That enables automatic 4:2:0 subsampling.
That option preserves the `deny.toml` C-purity contract.
Progressive Joint Photographic Experts Group encoding could shift loading latency.
It wouldn't change total encoding time.

### Baseline through C4 on a 24-megapixel real Z 5 landscape

The decode stage fell from 268 ms to 216 ms, down 19 %.
Frame time fell from 131 ms to 18 ms, down 86 %.
Encoding at `q92` fell from 819 ms to 787 ms, down 4 % within noise.
The default pipeline fell from **1245 ms** to **1019 ms**, down 18 %.
The estimated baseline `sns` pipeline CLI time was ~1.5–1.8 s.
Its measured current time is **~0.47 s**, down about 75 %.

Encoding at `q92` takes ~77 % of the default pipeline's remaining time.
JPEG entropy coding is sequential because Huffman state chains across blocks.
Further gains require an encoder with restart markers or parallel chunks.
Lowering quality is another option because it reduces coding cost.
Phase D3 is the next data-supported target.
Image-crate's `JpegEncoder` may support further tuning.
