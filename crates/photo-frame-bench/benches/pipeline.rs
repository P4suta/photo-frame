//! Hot-path benchmarks for the photo-frame pipeline.
//!
//! Three groups, mirroring the pipeline stages:
//!   - `decode`: 1024×768 JPEG (synthesised) → Photograph
//!   - `frame`:  3000×2000 RGBA8 Photograph → framed Pixels
//!   - `encode`: 3000×2000 RGBA8 Pixels → JPEG bytes at q78 / q92 / q98
//!
//! Run via `just bench` or `cargo bench -p photo-frame-bench`.
//! Output is HTML at `target/criterion/<bench>/report/index.html`.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use image::{codecs::jpeg::JpegEncoder, ExtendedColorType, ImageEncoder, RgbImage};
use photo_frame::{
    decode::from_bytes,
    encode::{jpeg, JpegOptions},
    frame::{render, FrameOptions},
    types::{Photograph, Pixels, Provenance},
};

fn synth_jpeg(w: u32, h: u32) -> Vec<u8> {
    let img = RgbImage::from_pixel(w, h, image::Rgb([200, 60, 60]));
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 90)
        .write_image(&img, w, h, ExtendedColorType::Rgb8)
        .expect("synthetic jpeg encode");
    out
}

fn synth_photograph(w: u32, h: u32) -> Photograph {
    let buf = vec![200_u8; (w as usize) * (h as usize) * 4];
    let pixels = Pixels::from_rgba8(w, h, buf).expect("pixels");
    Photograph::new(pixels, Provenance::default())
}

fn bench_decode(c: &mut Criterion) {
    let jpeg_bytes = synth_jpeg(1024, 768);
    let mut g = c.benchmark_group("decode");
    g.throughput(Throughput::Bytes(jpeg_bytes.len() as u64));
    g.bench_function("jpeg_1024x768", |b| {
        b.iter(|| from_bytes(&jpeg_bytes).expect("decode"));
    });
    g.finish();
}

fn bench_frame(c: &mut Criterion) {
    let photo = synth_photograph(3000, 2000);
    let opts = FrameOptions::default();
    let mut g = c.benchmark_group("frame");
    g.bench_function("render_3000x2000", |b| {
        b.iter(|| render(&photo, &opts));
    });
    g.finish();
}

fn bench_encode(c: &mut Criterion) {
    let pixels = Pixels::from_rgba8(3000, 2000, vec![200; 3000 * 2000 * 4]).expect("pixels");
    let mut g = c.benchmark_group("encode");
    for q in [78_u8, 92, 98] {
        g.bench_with_input(BenchmarkId::from_parameter(q), &q, |b, &q| {
            b.iter(|| jpeg(&pixels, &JpegOptions { quality: q }).expect("encode"));
        });
    }
    g.finish();
}

criterion_group!(benches, bench_decode, bench_frame, bench_encode);
criterion_main!(benches);
