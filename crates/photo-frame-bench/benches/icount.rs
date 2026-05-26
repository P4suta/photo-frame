// iai-callgrind 0.16's `#[library_benchmark]` macro expands the
// `setup = …` expression into a path that the workspace's strict
// `unused_qualifications` lint flags at the original source span.
// The qualification is in the macro-generated code, not in ours,
// and there is no way to opt the expansion out from the call site.
// Scope the allow to this single file so the lint stays sharp
// everywhere else.
#![allow(
    unused_qualifications,
    reason = "iai-callgrind macros generate redundant path qualification"
)]
#![allow(
    missing_docs,
    reason = "iai-callgrind macros expand to modules and free fns the workspace `missing_docs` lint then flags; documenting macro-generated symbols is meaningless"
)]
//! Instruction-count benches for the photo-frame pipeline.
//!
//! iai-callgrind drives Valgrind's `callgrind` tool to count exact
//! instruction executions per benchmark — hardware-independent and
//! deterministic enough for a CI regression gate (cf. divan's
//! wall-clock numbers, which need statistical handling).
//!
//! Fixture set is intentionally small: the 4 MP synth and the
//! panorama from [`photo_frame_bench::fixtures::small`]. Running a
//! 24 MP fixture through encode under Valgrind takes minutes per
//! sample, which is prohibitive for PR-time gating.
//!
//! ## Running locally
//!
//! ```bash
//! apt install valgrind                            # one-time, OS package
//! mise install                                    # picks up iai-callgrind-runner from mise.toml
//! cargo bench -p photo-frame-bench --bench icount
//! ```
//!
//! ## CI gate
//!
//! `.github/workflows/runtime-bench.yml` compares the icount totals
//! against `main` and fails on any stage with > 5 % instruction-count
//! regression. (Workflow lands in a follow-up — Phase A3.2.)

use std::hint::black_box;

use iai_callgrind::{library_benchmark, library_benchmark_group, main};

use photo_frame::{
    decode::from_bytes,
    encode::{jpeg, JpegOptions},
    frame::{render, FrameOptions},
};
use photo_frame_bench::fixtures;

/// Setup that yields owned bytes for the bench function. Owning the
/// vec inside the bench function (rather than holding a `&Fixture`
/// reference) avoids a known iai-callgrind 0.16 macro-expansion
/// quirk where reference arguments confuse the auto-`black_box`
/// wrapper and break type inference.
fn fixture_bytes_synth_4mp() -> Vec<u8> {
    fixtures::synth_4mp().bytes.clone()
}
fn fixture_bytes_panorama() -> Vec<u8> {
    fixtures::synth_panorama().bytes.clone()
}

#[library_benchmark]
#[bench::synth_4mp(setup = fixture_bytes_synth_4mp)]
#[bench::panorama(setup = fixture_bytes_panorama)]
fn decode(bytes: Vec<u8>) {
    let _ = black_box(from_bytes(&bytes).expect("decode"));
}

#[library_benchmark]
#[bench::synth_4mp(setup = fixture_bytes_synth_4mp)]
#[bench::panorama(setup = fixture_bytes_panorama)]
fn frame(bytes: Vec<u8>) {
    let photo = from_bytes(&bytes).expect("decode");
    let opts = FrameOptions::default();
    let _ = black_box(render(photo, &opts));
}

#[library_benchmark]
#[bench::synth_4mp(setup = fixture_bytes_synth_4mp)]
#[bench::panorama(setup = fixture_bytes_panorama)]
fn encode_q92(bytes: Vec<u8>) {
    let photo = from_bytes(&bytes).expect("decode");
    let pixels = render(photo, &FrameOptions::default());
    let opts = JpegOptions::default();
    let _ = black_box(jpeg(&pixels, &opts).expect("encode"));
}

library_benchmark_group!(
    name = decode_group;
    benchmarks = decode
);

library_benchmark_group!(
    name = frame_group;
    benchmarks = frame
);

library_benchmark_group!(
    name = encode_group;
    benchmarks = encode_q92
);

main!(
    library_benchmark_groups = decode_group,
    frame_group,
    encode_group
);
