# Performance and dependency policy

Use this guide to assess performance-sensitive code and dependencies.
`BENCHMARKS.md` records the measurements behind those decisions.

## Architectural constraints

`deny.toml` rejects standard C-toolchain build dependencies.
The blocklist includes `cc`, `cc-rs`, `bindgen`, and `pkg-config`.
It also includes `vcpkg`, `cmake`, `autotools`, and `meson`.
Any dependency that adds a blocked crate fails `cargo deny check bans`.
The root `Cargo.toml` sets `workspace.lints.rust.unsafe_code` to `forbid`.
Project code uses safe crates whose upstream unsafe blocks receive audits.
Every change must keep the baseline WebAssembly build portable.
Run `wasm-pack build --target web --release crates/photo-frame-wasm` to check it.
Feature flags enable vectorized and threaded implementations where supported.
Public errors include `DecodeError`, `EncodeError`, and `PipelineError`.
Those types use `thiserror::Error` and `miette::Diagnostic`.
Each variant carries a stable diagnostic code, help text, and a `Category`.
The `Category` controls the CLI exit-code mapping.
Choose the simpler implementation when measurements are similar.
`tasks/feedback_architecture_beauty.md` records the longer design rationale.

## The optional image decoder feature

The `photo-frame-decode` crate has one exception to the pure-Rust rule.
Its `heif` feature adds `libheif-rs`, `libheif-sys`, and build dependencies.
Those dependencies include `pkg-config` or `vcpkg`.
The feature stays off by default.
Ordinary builds and `cargo deny check bans` without `--all-features` omit it.
Users enable the feature with this command:

```bash
cargo install --path crates/photo-frame-cli --features heif
```

Builds without the feature report detected `HEIC` input as unsupported.
`DecodeError::HeifFeatureDisabled` identifies that condition.
The top comment in `deny.toml` records this dependency-policy exception.
Another C-toolchain exception requires a default-off feature.
It also requires a typed error and an update to this guide.

## Proposing a performance change

Every performance-critical change must cite a relevant baseline row.
Use `docs/PERF-REPORT-baseline.md` or its phase successor.
When no row exists, run `just bench` on the realistic Z-series fixture.
Record that measurement before proposing the change.
Pure-Rust dependencies with vectorized code can go in `[workspace.dependencies]`.
Examples include `zune-jpeg`, `fast_image_resize`, and `jpeg-encoder`.
WebAssembly uses scalar or single-thread paths when crates lack acceleration.
Dependencies that build C code through a foreign-function interface need a feature.
Keep that Cargo feature off by default and follow the `heif` pattern.
Map each new failure into its relevant `*Error` enum and `Category`.
Also define its stable `code` and `help` methods.
Avoid `Box<dyn Error>` and `String` errors at public boundaries.
Run `just bench` before and after the change.
Add the measured delta and commit to `BENCHMARKS.md`.
Quote the same results in the pull request.

## Performance tools

`mise.toml` pins host tools.
The `dev` stage in `Dockerfile` installs tools for the development container.
GitHub Actions jobs install selected tools through `taiki-e/install-action`.
Some jobs use `cargo binstall` instead.
`divan` measures wall-clock time by stage and fixture through `just bench`.
`iai-callgrind`, `iai-callgrind-runner`, and `valgrind` measure instructions.
Run those instruction-count measurements with `just bench-icount`.
`samply` records CPU profiles through `just profile-pipeline FIXTURE`.
Open its profiles in Firefox Profiler.
`inferno-flamegraph` renders folded samples as SVG through `just profile-trace FIXTURE`.
The CLI `dhat` feature records heap allocations.
Enable it with `cargo run -p photo-frame-cli --features dhat -- …`.
The `tracing-flame` feature on `photo-frame` captures per-span timings.
Use its `--profile-trace=PATH` flag or the `just profile-trace` recipe.
`hyperfine` measures build time and workflow steps through `scripts/bench-build.sh`.

## Performance choices

The workspace forbids `unsafe` code, including unsafe micro-optimizations.
Use maintained vectorized crates instead of handwritten intrinsics.
The project considered foreign-function interfaces to `mozjpeg` and `libjpeg-turbo`.
It also considered `libvips` before declining those dependencies.
Vectorized `jpeg-encoder` closes the former speed gap.
GPU approaches such as `wgpu` and `vulkano` would add C-toolchain dependencies.
Those dependencies would also weaken WebAssembly portability.
The framing workload is too bursty to amortize GPU upload and download costs.
The Standard preset remains at quality 92.
Below quality 90, the crate uses automatic 4:2:0 chroma subsampling.
The `sns` preset uses quality 78 and takes that subsampling path.
The supported Rust version floor remains 1.78.
Fixture code uses `OnceLock` instead of `std::sync::LazyLock` to preserve it.
Features that need a newer Rust release need a broader policy decision.

## Build and test measurements

The pre-Phase-0 baseline used commit `8974cc6` from `v2.0-rewrite`.
The host was Linux 6.8.0-117-generic on x86_64.
Cargo and rustc were both version 1.95.0.
Hyperfine baselines use three-run medians unless stated otherwise.
The raw JSON remains under its matching `artifacts/bench/` directory.
One cold `cargo build --workspace` run took 0.62 seconds.
That run followed `touch crates/photo-frame-decode/src/lib.rs`.
Warm `cargo build --workspace` took 0.08 seconds over three runs.
`cargo check --workspace --all-targets` also took 0.08 seconds over three runs.
`cargo test --workspace --all-targets --no-fail-fast` took 14.73 seconds.
That test result came from three runs.
Host clippy used `cargo clippy --workspace --all-targets -- -D warnings`.
That command took 0.10 seconds.
`wasm-pack build --target web --release` took 3.04 seconds over two runs.
Those WebAssembly build runs showed high cold-versus-warm variance.
Warm-cache host tests dominated the measured workflow at 15 seconds.
Phase 1.2 targeted test-profile optimization.
Phase 1.3 targeted nextest.

The container baseline used `photo-frame-dev:latest` with bind mounts.
It also used `.cargo-cache-docker` and `target-docker` workaround directories.
Warm container `just ci` took 18.45 seconds.
Container `cargo build --workspace` took 0.38 seconds.
Container `cargo test --workspace --all-targets` took 15.45 seconds.
Container clippy took 0.40 seconds with a warm cache.
Each container startup added about 0.3 to 0.5 seconds.
Warm `docker compose build dev` took 1.00 second.
The cold development-image build wasn't measured.
The original cold estimate was 5 to 8 minutes.
The cargo-chef and sccache target was at most 2 minutes.
The later phases targeted the 18-second warm pipeline and its test cost.

Phase 1.2 reduced host test time to 1.15 seconds.
That result was 92 percent below the 14.73-second baseline.
The phase applied `-O3` to the `package."*"` development and test profiles.
Phase 1.3 measured nextest at 1.33 seconds.
That result added 180 milliseconds over the prior Cargo test result.
Nextest added JUnit output, retries, and per-test timing.
Phase 1.4 reduced container `just ci` to 5.00 seconds.
That result was 73 percent below the 18.45-second baseline.
The phase combined cache mounts, `cargo-binstall`, and a container user-ID fix.

Phase 1.5 adopted a cargo-chef multi-stage Dockerfile.
An earlier commit had deferred cargo-chef for the bind-mounted dev image.
The project later added a slim CLI runtime image alongside the dev image.
The `chef-base` stage installs Rust, `cargo-binstall`, and `cargo-chef`.
It uses the `rust:slim` image.
The `planner` stage runs `cargo chef prepare` and writes `recipe.json`.
The `cacher` stage runs `cargo chef cook --release` on that recipe.
The `builder` stage copies source and runs `cargo build --release -p cli`.
The `runtime` stage contains Debian trixie slim, CA certificates, and the CLI.
The `dev` stage retains the full development toolchain.
`docker-compose.yml` selects `dev` by default.
`just docker-build-release` builds the `runtime` target.
The first cold runtime-image build took about 480 seconds.
That run compiled every release dependency at `-O3`.
An unchanged warm build took 1.3 seconds.
The estimate for an app-only rebuild was 30 to 60 seconds.
The final runtime image measured 32 megabytes.
It contains a 14-megabyte CLI binary and CA certificates.
`.dockerignore` keeps `target/` and `artifacts/` out of Docker build contexts.
Without that file, planner and builder copy steps invalidate cached layers.

The mold experiment compared `GNU ld` with mold 2.41.0.
It followed `touch crates/photo-frame-cli/src/main.rs`.
`GNU ld` development relinking took 404.1 milliseconds with 3.4-millisecond deviation.
That result came from five runs.
`GNU ld` release relinking took 8.390 seconds with 0.059-second deviation.
That result came from three runs.
mold development relinking took 404.7 milliseconds with a 4.9-millisecond deviation.
That result came from five runs.
mold release relinking took 8.392 seconds with a 0.028-second deviation.
That result came from three runs.
The development binary measured 78 megabytes.
Either linker completed the development link in less than 100 milliseconds.
Cargo and rustc startup dominated development wall-clock time.
Link-time optimization and code generation dominated release linking.
The experiment found no mold speedup.
The project added no `.cargo/config.toml` linker setting.
Re-measure after the workspace grows beyond 10 larger crates.

The original overhaul listed targets rather than achieved results.
It targeted 50 percent faster warm host builds through mold and split debuginfo.
It targeted 30 percent faster tests through nextest and test-profile optimization.
It targeted 40 percent faster warm container `just ci` through nextest and sccache.
Improved cache mounts formed part of that container target.
It targeted 60 percent faster cold Docker builds through cargo-chef.
It targeted 50 percent faster GitHub Actions runs through parallel jobs.
`BENCHMARKS.md` records measured results apart from those targets.

## Runtime benchmark method

The Divan suite measures decode, resize, frame, encode, and pipeline stages.
Its harness lives in `crates/photo-frame-bench/`.
Each recorded value is a median for its sample run.
Raw output lives under `artifacts/bench/runtime/<UTC>/`.
The fixture matrix stays fixed so phases remain comparable.
The workspace `.gitignore` excludes image binaries.
Real-world fixture images remain on local machines.
Clean clones and automated jobs warn when those images are absent.
They run the synthetic fixture subset instead.
The synthetic fixtures provide a demanding decoder workload.
The real-world measurements track the desired camera inputs.

The landscape fixtures are `real_z5_landscape_a_24mp` and `real_z5_landscape_b_24mp`.
They use `IMG_3936.JPG` and `IMG_3939.JPG`, respectively.
Both images measure 6016 by 4016 pixels.
Both have Exchangeable Image File Format orientation 1.
The second landscape is another native photo from the Nikon Z 5.
The portrait fixture is `real_z5_portrait_rot8_24mp` from `IMG_3940.JPG`.
It also measures 6016 by 4016 pixels.
Its `EXIF` orientation is 8, which exercises a 90-degree counter-clockwise rotation.
The `synth_noise_4mp_2400x1600` fixture has 2400 by 1600 pixels.
It encodes xorshift `RGB` data as `JPEG` at quality 85.
That fixture uses orientation 1 and approximates a mobile-phone image.
The `synth_noise_12mp_4240x2832` fixture has 4240 by 2832 pixels.
It uses the same generation method and approximates a mid-range mirrorless image.
The `synth_noise_24mp_6016x4016` fixture has 6016 by 4016 pixels.
Its dimensions match the Z 5 sensor's megapixel count.
The `synth_noise_panorama_10000x100` fixture has 10000 by 100 pixels.
It exercises an extreme aspect ratio.
Synthetic fixtures use xorshift `RGB`, `JPEG` quality 85, and orientation 1.

## Runtime baseline

The runtime baseline used commit `1dcbfdf` on Linux 6.8.0-117-generic.
It used rustc 1.95.0, Cargo 1.95.0, and Divan 0.1.21.
The harness used 10 samples and wrote `artifacts/bench/runtime/20260525T001511Z/`.
Decode, resize, frame, encode, and pipeline times use milliseconds.
Throughput uses source pixels rather than the framed canvas.
Resize isolates `resize_lanczos3_to_sns`.
Frame excludes resize, and pipeline measures the default end-to-end path.

Landscape A decode took 268 milliseconds at 90.0 megapixels per second.
Its resize took 440 milliseconds, and its frame took 131 milliseconds.
Its quality-92 encode took 819 milliseconds.
Its default pipeline took 1245 milliseconds at 19.4 megapixels per second.
Landscape B decode took 225 milliseconds at 107 megapixels per second.
Its resize took 436 milliseconds, and its frame took 131 milliseconds.
Its quality-92 encode took 776 milliseconds.
Its default pipeline took 1143 milliseconds at 21.1 megapixels per second.
Portrait decode took 357 milliseconds at 67.7 megapixels per second.
Its resize took 439 milliseconds, and its frame took 130 milliseconds.
Its quality-92 encode took 755 milliseconds.
Its default pipeline took 1266 milliseconds at 19.1 megapixels per second.
Synthetic 4-megapixel decode took 86 milliseconds at 44.5 megapixels per second.
Its resize took 167 milliseconds, and its frame took 9.1 milliseconds.
Its quality-92 encode took 227 milliseconds.
Its pipeline took 333 milliseconds at 11.5 megapixels per second.
Synthetic 12-megapixel decode took 270 milliseconds at 44.6 megapixels per second.
Its resize took 272 milliseconds, and its frame took 63.4 milliseconds.
Its quality-92 encode took 732 milliseconds.
Its pipeline took 1097 milliseconds at 10.9 megapixels per second.
Synthetic 24-megapixel decode took 546 milliseconds at 44.2 megapixels per second.
Its resize took 439 milliseconds, and its frame took 126 milliseconds.
Its quality-92 encode took 1423 milliseconds.
Its pipeline took 2111 milliseconds at 11.4 megapixels per second.
Panorama decode took 20.0 milliseconds at 49.9 megapixels per second.
Its resize took 9.9 milliseconds, and its frame took 2.2 milliseconds.
Its quality-92 encode took 58.1 milliseconds.
Its pipeline took 79.4 milliseconds at 12.6 megapixels per second.

Quality-92 encoding took 819 milliseconds on the 24-megapixel landscape fixture.
That stage used 66 percent of the 1.25-second pipeline.
Decode used 22 percent, and framing without resize used 10 percent.
The two landscape fixtures averaged 247 milliseconds for decode.
The orientation-8 portrait took 357 milliseconds.
Orientation 8 correlated with a 30 percent decode cost.
Phase B planned isolated measurement of the rotation cost.
Synthetic fixtures decoded at about 44 megapixels per second.
Real-world fixtures decoded at about 90 to 107 megapixels per second.
The baseline report linked the gap to high-entropy synthetic images.
Those images gave the decoder a more demanding workload.

## Runtime phase measurements

Phase C1 replaced copied `Pixels` handoffs with zero-copy handoffs.
The pipeline fell from 1.25 seconds to 1.17 seconds.
That was a 6 percent reduction from baseline.
Decode measured 270 milliseconds with no material change.
Encode measured 806 milliseconds, a 1.5 percent reduction.
Framing measured 92 milliseconds, a 30 percent reduction.
dhat total allocation fell from 535 megabytes to 439 megabytes.
That saved 97 megabytes, close to one 92-megabyte `RGBA` buffer.
Peak allocation fell from 336 to 243 megabytes.

Phase D1 adopted `zune-jpeg` for `JPEG` decoding.
The pipeline measured 1.11 seconds, an 11 percent reduction from baseline.
Decode fell to 217 milliseconds, a 19 percent reduction.
Encode measured 788 milliseconds, a 2 percent change within noise.
Framing measured 94 milliseconds.
dhat total allocation fell from 439 to 353 megabytes, a further 86 megabytes.
Peak allocation remained at 243 megabytes.
Decode gains ranged from 9 percent on the rotated portrait to 23 percent on landscapes.
The rotated portrait placed orientation outside vectorized color conversion.

Phase D2 adopted `fast_image_resize` for the resize path.
The default pipeline measured 1.10 seconds with no change.
Decode measured 217 milliseconds, encode 788 milliseconds, and framing 91 milliseconds.
The isolated 24-megapixel resize fell from 438 to 113 milliseconds.
That was a 74 percent reduction and a fourfold speedup.
The `sns` pipeline on the 24-megapixel Z 5 fixture took 0.47 seconds.
The earlier resize path had a 0.80-second estimate.
The `resize_fir_lanczos3_to_sns` benchmark reached 213 megapixels per second.
The image-crate implementation measured 55 megapixels per second.
Rayon workers and vectorized code process rows in parallel.
WebAssembly uses a single thread.

Phase C4 replaced two-pass canvas construction with one row-parallel pass.
The default pipeline measured 1.02 seconds.
That was 18 percent below baseline and 7 percent below Phase D2.
Decode measured 216 milliseconds, encode 787 milliseconds, and framing 18 milliseconds.
Framing improved 86 percent from baseline and 80 percent from Phase D2.
Each Rayon worker fills the background, copies photo bytes, then fills the rest.
The worker completes that work in one row traversal.
Canvas composition rose from 184 megapixels per second to 1.33 gigapixels per second.
That was a sevenfold change.
Panorama framing fell from 1.79 milliseconds to 0.30 milliseconds.
That was a sixfold change.
The change didn't regress memory use.
Zero-initializing the vector cost less than the former `from_pixel` path.

Phase D3 replaced `image::codecs::jpeg::JpegEncoder` with `jpeg_encoder::Encoder`.
The default pipeline measured 662 milliseconds.
That was 47 percent below baseline and 35 percent below Phase C4.
Decode measured 216 milliseconds, encode 424 milliseconds, and framing 18 milliseconds.
Encoding improved 48 percent from baseline and 46 percent from Phase C4.
The encoder uses vectorized discrete cosine transform and Huffman code paths.
It uses `AVX2` on `x86_64` hosts.
Both measurements used quality 92 and 4:4:4 chroma.
The speedup didn't come from a quality or format change.
Automatic 4:2:0 chroma subsampling still activates only below quality 90.
The `sns` pipeline at quality 78 fell from 374 to 314 milliseconds.
That was a 16 percent improvement because the preset already subsamples chroma.
The encode crate removed its `image` dependency.
It now uses `jpeg-encoder` with `photo-frame-types::Pixels`.

Across the 24-megapixel landscape snapshots, decode fell from 268 to 216 milliseconds.
That was a 19 percent reduction.
Framing fell from 131 to 18 milliseconds, an 86 percent reduction.
Quality-92 encoding fell from 819 to 424 milliseconds, a 48 percent reduction.
The default pipeline fell from 1245 to 662 milliseconds.
That was a 47 percent reduction and approximately twice the speed.
The `sns` pipeline fell from about 1500 to 1800 milliseconds to 314 milliseconds.
That was an 80 percent reduction and a fivefold speedup.
dhat total allocation fell from 535 to at most 353 megabytes.
That was a 34 percent reduction for the same input.
Peak allocation fell from 336 to 243 megabytes in Phase C1.
It stayed at 243 megabytes from Phase D1 onward.

The post-C4 snapshot predates Phase D3.
Its default pipeline took 1019 milliseconds compared with 1245 milliseconds at baseline.
Decode fell from 268 to 216 milliseconds, a 19 percent reduction.
Framing fell from 131 to 18 milliseconds, an 86 percent reduction.
Encoding fell from 819 to 787 milliseconds, a 4 percent change within noise.
The `sns` CLI pipeline took about 0.47 seconds.
The earlier estimate ranged from 1.5 to 1.8 seconds.
That was a reduction of about 75 percent.
Quality-92 encoding used about 77 percent of the post-C4 default pipeline.
`JPEG` entropy coding remains sequential because Huffman state chains across blocks.

In the Phase D3 snapshot, encoding used 424 of 662 milliseconds.
That stage accounted for 64 percent of the default pipeline.
Decode accounted for 33 percent.
Encoding remained the largest stage without the previous tenfold gap.
Potential changes include reducing Standard quality to 88.
That value would enable 4:2:0 subsampling.
Progressive `JPEG` could change loading latency, but not total encoding time.
Neither option changes the recorded default quality of 92.
