//! Per-stage and end-to-end benches for the photo-frame pipeline.
//!
//! Five groups, mirroring the pipeline boundary plus a couple of
//! sub-steps that Phase B will need to attribute cost to:
//!
//! - `decode`: raw JPEG bytes → `Photograph` (decode + EXIF +
//!   orientation, the whole `from_bytes` surface).
//! - `resize_lanczos3_to_sns`: an in-memory `RgbaImage` resized to
//!   the SNS-preset long edge via `image::DynamicImage::resize(Lanczos3)`,
//!   isolated from the rest of the renderer so the filter's contribution
//!   to `frame` can be read off.
//! - `frame`: `Photograph` → framed `Pixels` (the full
//!   `photo_frame_frame::render`, including caption draw and canvas
//!   compose).
//! - `encode`: framed `Pixels` → JPEG bytes at quality 92
//!   (the user-default).
//! - `pipeline`: end-to-end bytes-in / bytes-out, which is the
//!   number the user cares about — Phase B's report anchors against
//!   this row.
//!
//! Each bench reports MP/s via `divan::counter::ItemsCount`. Run via
//! `just bench` (`cargo bench -p photo-frame-bench`). Output goes to
//! stdout; `artifacts/bench/runtime/<UTC>/` capture is wired up in
//! Phase A3.

use divan::counter::ItemsCount;
use divan::{black_box, Bencher};

use image::{DynamicImage, ImageBuffer, Rgba, RgbaImage};
use photo_frame::{
    decode::from_bytes,
    encode::{jpeg, JpegOptions},
    frame::{render, FrameOptions},
    types::Pixels,
};
use photo_frame_bench::fixtures::{self, Fixture};

fn main() {
    divan::main();
}

// ─── decode: bytes → Photograph ─────────────────────────────────────────

#[divan::bench(args = fixtures::all())]
fn decode(bencher: Bencher<'_, '_>, fixture: &Fixture) {
    bencher
        .counter(ItemsCount::new(fixture.pixel_count()))
        .bench(|| from_bytes(black_box(&fixture.bytes)).expect("decode"));
}

// ─── resize: isolated Lanczos3 cost on a synthesised RgbaImage ──────────
//
// Reproduces `render.rs:79` exactly (DynamicImage::resize with
// Lanczos3 to the SNS-preset long edge of 2048) but driven directly,
// so Phase B can read the filter's contribution to `frame` without
// having to subtract caption / compose costs.

const SNS_LONG_EDGE: u32 = 2048;

fn synth_rgba_image(fixture: &Fixture) -> RgbaImage {
    let pixel_count = (fixture.width as usize) * (fixture.height as usize);
    let mut buf = Vec::with_capacity(pixel_count * 4);
    for _ in 0..pixel_count {
        buf.extend_from_slice(&[200, 60, 60, 255]);
    }
    ImageBuffer::<Rgba<u8>, _>::from_raw(fixture.width, fixture.height, buf)
        .expect("synth buffer length matches w*h*4 by construction")
}

#[divan::bench(args = fixtures::all())]
fn resize_lanczos3_to_sns(bencher: Bencher<'_, '_>, fixture: &Fixture) {
    // The cost we want to measure is the filter itself, not the
    // upstream buffer copy. Build the input once outside the timed
    // closure and reuse it across samples.
    let img = synth_rgba_image(fixture);
    let long = fixture.width.max(fixture.height);
    // Integer-only target-dimension math — same ratio as `render.rs:79`
    // but without the float→u32 cast clippy makes us own. The numerator
    // fits in u64 trivially (max is 6016 × 2048 ≈ 1.2e7), and dividing
    // by a `long` that is strictly greater than `SNS_LONG_EDGE` keeps
    // the result smaller than the input dimension, so it always fits
    // back into u32.
    let (new_w, new_h) = if long <= SNS_LONG_EDGE {
        (fixture.width, fixture.height)
    } else {
        let edge = u64::from(SNS_LONG_EDGE);
        let long_u64 = u64::from(long);
        let w64 = u64::from(fixture.width) * edge / long_u64;
        let h64 = u64::from(fixture.height) * edge / long_u64;
        (
            u32::try_from(w64.max(1)).expect("fits"),
            u32::try_from(h64.max(1)).expect("fits"),
        )
    };
    bencher
        .counter(ItemsCount::new(fixture.pixel_count()))
        .bench_local(|| {
            DynamicImage::ImageRgba8(img.clone())
                .resize(
                    black_box(new_w),
                    black_box(new_h),
                    image::imageops::FilterType::Lanczos3,
                )
                .to_rgba8()
        });
}

// ─── frame: full render (compose + caption + downscale) ─────────────────
//
// `render` now consumes `Photograph` (Phase C1 zero-copy handoff), so
// each bench iteration needs a fresh copy. `with_inputs` clones the
// pre-decoded photo outside the timed window — clone is a single
// `Vec::clone` on the RGBA buffer (~30 ms at 24 MP) vs re-decoding
// the JPEG (~270 ms) so the bench stays fast and the per-iteration
// allocation cost stays out of the measured budget.

#[divan::bench(args = fixtures::all())]
fn frame(bencher: Bencher<'_, '_>, fixture: &Fixture) {
    let photo = from_bytes(&fixture.bytes).expect("decode");
    let opts = FrameOptions::default();
    bencher
        .counter(ItemsCount::new(fixture.pixel_count()))
        .with_inputs(|| photo.clone())
        .bench_local_values(|photo| render(black_box(photo), black_box(&opts)));
}

// ─── encode: framed Pixels → JPEG ───────────────────────────────────────
//
// We bench encode at the user-default quality (92) on the post-frame
// canvas, not the raw decoded pixels — that is the byte path the CLI
// actually walks. The post-frame canvas is larger than the source by
// the golden-ratio border, so `bencher.counter` uses the canvas pixel
// count, not the source's. The pre-render `Photograph` is consumed
// once, outside the timed window, to obtain the framed `Pixels`.

#[divan::bench(args = fixtures::all())]
fn encode(bencher: Bencher<'_, '_>, fixture: &Fixture) {
    let photo = from_bytes(&fixture.bytes).expect("decode");
    let pixels: Pixels = render(photo, &FrameOptions::default());
    let opts = JpegOptions::default();
    let canvas_px = u64::from(pixels.width()) * u64::from(pixels.height());
    bencher
        .counter(ItemsCount::new(canvas_px))
        .bench_local(|| jpeg(black_box(&pixels), black_box(&opts)).expect("encode"));
}

// ─── pipeline: end-to-end bytes → bytes ─────────────────────────────────

#[divan::bench(args = fixtures::all())]
fn pipeline(bencher: Bencher<'_, '_>, fixture: &Fixture) {
    let opts = photo_frame::PipelineOptions::default();
    bencher
        .counter(ItemsCount::new(fixture.pixel_count()))
        .bench(|| {
            photo_frame::pipeline(black_box(&fixture.bytes), black_box(&opts)).expect("pipeline")
        });
}

// ─── pipeline_sns: end-to-end SNS preset (max_long_edge=2048) ─────────
//
// The default `pipeline` bench above runs with no downscale, which is
// the path where the renderer's `maybe_downscale` never fires. The
// SNS preset is the only built-in that triggers it (and the one most
// users actually pick for social-media sharing), so a dedicated row
// is the only place Phase D2 (fast_image_resize swap) actually shows
// its win.

#[divan::bench(args = fixtures::all())]
fn pipeline_sns(bencher: Bencher<'_, '_>, fixture: &Fixture) {
    let opts = photo_frame::PipelineOptions::from_preset(photo_frame::QualityPreset::Sns);
    bencher
        .counter(ItemsCount::new(fixture.pixel_count()))
        .bench(|| {
            photo_frame::pipeline(black_box(&fixture.bytes), black_box(&opts)).expect("pipeline")
        });
}

// ─── resize: direct fast_image_resize comparison ───────────────────────
//
// `resize_lanczos3_to_sns` above drives the *image crate's* resize
// directly so the baseline column survives the Phase D2 swap. This
// sibling bench drives `fast_image_resize` directly on the same
// inputs so the two rows are an apples-to-apples comparison —
// looking at both side by side answers "what fraction of the SNS
// pipeline speedup is the resize itself?".

#[divan::bench(args = fixtures::all())]
fn resize_fir_lanczos3_to_sns(bencher: Bencher<'_, '_>, fixture: &Fixture) {
    use fast_image_resize as fir;
    use image::{DynamicImage, ImageBuffer as IB};
    let img = synth_rgba_image(fixture);
    let long = fixture.width.max(fixture.height);
    let (new_w, new_h) = if long <= SNS_LONG_EDGE {
        (fixture.width, fixture.height)
    } else {
        let edge = u64::from(SNS_LONG_EDGE);
        let long_u64 = u64::from(long);
        let w64 = u64::from(fixture.width) * edge / long_u64;
        let h64 = u64::from(fixture.height) * edge / long_u64;
        (
            u32::try_from(w64.max(1)).expect("fits"),
            u32::try_from(h64.max(1)).expect("fits"),
        )
    };
    let options = fir::ResizeOptions::new()
        .resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::Lanczos3));
    bencher
        .counter(ItemsCount::new(fixture.pixel_count()))
        .bench_local(|| {
            let src = DynamicImage::ImageRgba8(img.clone());
            let mut dst = DynamicImage::ImageRgba8(IB::new(black_box(new_w), black_box(new_h)));
            fir::Resizer::new()
                .resize(&src, &mut dst, &options)
                .expect("fir: same pixel type, valid dims");
            dst.into_rgba8()
        });
}
