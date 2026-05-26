//! End-to-end test for the public batch shape.
//!
//! Sits in `tests/` rather than `#[cfg(test)] mod` so it exercises the
//! same import surface a downstream crate would see — catches re-export
//! regressions that an internal-only test would silently let through.

use image::{codecs::jpeg::JpegEncoder, ExtendedColorType, ImageEncoder, RgbImage};
use photo_frame::{batch_one, BatchOutcome, PipelineOptions, StageEvent};

fn tiny_jpeg(w: u32, h: u32) -> Vec<u8> {
    let img = RgbImage::from_pixel(w, h, image::Rgb([120, 90, 200]));
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 90)
        .write_image(&img, w, h, ExtendedColorType::Rgb8)
        .expect("jpeg encode");
    out
}

#[test]
fn batch_one_is_reachable_from_facade_root() {
    let bytes = tiny_jpeg(48, 32);
    let outcome: BatchOutcome<&str> = batch_one(
        "hero.jpg",
        0,
        1,
        &bytes,
        &PipelineOptions::default(),
        |_event: StageEvent| {},
    );
    assert_eq!(outcome.key, "hero.jpg");
    assert!(outcome.is_ok());
}

#[test]
fn batch_mixed_inputs_produce_per_item_outcomes() {
    // Simulate a small batch: 1 good + 1 garbage, 1-fail-continue
    // semantics live at the call site (here the test loops manually).
    let inputs: Vec<(&str, Vec<u8>)> = vec![
        ("ok.jpg", tiny_jpeg(40, 30)),
        ("bad.jpg", b"not an image".to_vec()),
    ];
    let opts = PipelineOptions::default();
    #[allow(
        clippy::cast_possible_truncation,
        reason = "fixture sizes are bounded by the test, never approach u32::MAX"
    )]
    let total = inputs.len() as u32;
    let outcomes: Vec<_> = inputs
        .iter()
        .enumerate()
        .map(|(index, (k, b))| batch_one(*k, index, total, b, &opts, |_event: StageEvent| {}))
        .collect();
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes[0].is_ok(), "first item must succeed");
    assert!(!outcomes[1].is_ok(), "garbage bytes must surface an error");
}
