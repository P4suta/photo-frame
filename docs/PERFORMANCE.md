# Performance and dep-policy guide

This file is the living contract for *how* `photo-frame` keeps its
pipeline fast without giving up the architectural properties the
project considers non-negotiable. It exists so a new contributor
proposing a perf-critical change knows what guardrails they're
working inside, and so a future maintainer can read the *why* of an
unusual-looking constraint.

## Architectural properties that drive every perf decision

1. **Pure-Rust dep tree** — `deny.toml` denies every standard
   C-toolchain build crate (`cc`, `cc-rs`, `bindgen`, `pkg-config`,
   `vcpkg`, `cmake`, `autotools`, `meson`). A change that pulls any
   of those into the transitive graph fails CI via
   `cargo deny check bans`.
2. **No `unsafe`** — `workspace.lints.rust.unsafe_code = "forbid"`
   in the root `Cargo.toml`. We rely on `safe` crates whose own
   `unsafe` blocks are audited upstream.
3. **WASM target stays green** — every change must keep
   `wasm-pack build --target web --release crates/photo-frame-wasm`
   working. SIMD / threads are layered on top with feature flags;
   the baseline build is portable.
4. **Typed errors at every public boundary** — `DecodeError`,
   `EncodeError`, `PipelineError` derive `thiserror::Error` +
   `miette::Diagnostic` so each variant carries a stable
   diagnostic `code`, a `help` line, and a `Category` for exit-code
   mapping. New error paths follow the same shape.
5. **Architecture beauty over delta** — when two approaches are
   close on numbers, we pick the one that leaves the codebase
   simpler to read. `tasks/feedback_architecture_beauty.md` is the
   long-form version of this principle.

## Escape-hatch precedent: the `heif` feature flag

The one place the C-purity contract intentionally cracks open is
`photo-frame-decode`'s `heif` cargo feature. Turning it on pulls in
`libheif-rs → libheif-sys → pkg-config / vcpkg`, which `deny.toml`
would normally reject. The contract holds because:

- **Default-off.** `cargo build` / `cargo deny check bans` (no
  `--all-features`) never see those deps. CI gates on the default
  build only.
- **Visible at the call site.** Users opt in with
  `cargo install --path crates/photo-frame-cli --features heif`,
  and the `DecodeError::HeifFeatureDisabled` variant exists so a
  non-heif build that receives HEIC bytes surfaces a typed
  diagnostic rather than a confusing "unknown format" error.
- **One-way ratchet, discussed.** `deny.toml`'s top comment
  explicitly calls this out: adding another such feature is a
  contract change, not a workflow change.

If a future perf-critical dep needs a similar exception (e.g. a
hypothetical `mozjpeg` encoder feature), use the `heif` pattern
verbatim: feature-flagged, default-off, typed error for
"feature-disabled but format detected", documented here.

## How to propose a perf-critical change

Every perf-critical dep swap or algorithm change follows the same
four steps. The pattern is what makes `BENCHMARKS.md`'s history
readable months later.

1. **Cite the data.** Open `docs/PERF-REPORT-baseline.md` (or its
   Phase-N successor) and link the row your change targets. If no
   row exists, the change is premature — run `just bench` against
   the realistic Z-series fixture and capture the number first.
2. **Choose the dep boundary.**
   - Pure-Rust with internal SIMD (e.g. `zune-jpeg`,
     `fast_image_resize`, `jpeg-encoder`): adopt directly under
     `[workspace.dependencies]`. WASM falls back to scalar / single-
     thread automatically.
   - Anything that pulls a C-build crate (FFI / bindgen): goes
     behind a `[features]` flag, off by default, with the `heif`
     pattern above as the template.
3. **Map errors typed.** Wrapper variant on the relevant
   `*Error` enum, `Category` mapping, `code(...)` and `help(...)`
   filled. No `Box<dyn Error>` or `String`-typed messages.
4. **Measure, append, justify.** `just bench` before and after,
   add a row to `BENCHMARKS.md`'s "Runtime performance" table
   showing the delta and the commit. The PR description quotes
   the numbers. Reverting is then a single revert away.

## Tools the perf workflow assumes

All pinned in `mise.toml` (host) and installed in the dev
container by `Dockerfile`'s `dev` stage. CI workflows install
them ad hoc via `taiki-e/install-action` or `cargo binstall`.

| Tool | Purpose | Used by |
| ---- | ------- | ------- |
| `divan` | Wall-clock per-stage / per-fixture bench harness | `just bench` |
| `iai-callgrind` + `iai-callgrind-runner` + `valgrind` | Instruction-count CI regression gate | `.github/workflows/runtime-bench.yml` icount job |
| `samply` | CPU flamegraph via Firefox Profiler UI | `just profile-pipeline FIXTURE` |
| `inferno` (`inferno-flamegraph`) | `.folded` → SVG flamegraph render | `just profile-trace FIXTURE` |
| `dhat` (cargo feature on the CLI) | Heap allocation profile | `cargo run -p photo-frame-cli --features dhat -- …` |
| `tracing-flame` (cargo feature on `photo-frame`) | Per-span timing capture | `--profile-trace=PATH` CLI flag, `just profile-trace` |
| `hyperfine` | Build-time / CI step wall-clock | `scripts/bench-build.sh` |

## Things explicitly out of scope

These get raised periodically; the answers are recorded here so
they stop being argued from scratch every time.

- **`unsafe` for micro-optimisation.** Forbidden workspace-wide.
- **Hand-written SIMD intrinsics.** The user has been explicit:
  use crates whose authors maintain the intrinsics; don't write
  our own.
- **FFI to `mozjpeg` / `libjpeg-turbo` / `libvips`.** Considered
  and declined. `jpeg-encoder` (pure Rust + SIMD feature) covers
  the encode-speed gap that `image`'s encoder used to leave.
- **GPU compute (wgpu / vulkano).** Would pull C-toolchain deps
  in, breaks WASM-on-server, and the framing pipeline is too
  bursty to amortise GPU upload/download.
- **Quality drops to chase speed.** Standard preset stays at q92.
  jpeg-encoder's auto-4:2:0 only kicks in for q < 90, so the SNS
  preset (q78) is where chroma subsampling lands naturally. We
  don't move presets downward to fish for benchmark wins.
- **Lowering the workspace MSRV's ratchet for a single fancy
  feature.** When `std::sync::LazyLock` would have been nicer than
  `OnceLock` in the fixtures module, we picked `OnceLock` to keep
  MSRV at 1.78. Adding new bound-pushing nightly features is the
  bigger conversation.

## When the perf story changes shape, update this file

This document is the *contract*. If the contract changes (a new
escape-hatch feature lands, the dep policy moves, a new bench
tool replaces an old one), update here first, then write the code.
Future contributors will read this file before they read the diff.
