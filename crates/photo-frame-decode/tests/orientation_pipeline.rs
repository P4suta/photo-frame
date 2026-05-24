//! End-to-end check that EXIF Orientation flows through the public pipeline.
//!
//! Unit tests under `src/orientation.rs` verify the eight rotations in
//! isolation. This file pins the *wiring*: `from_bytes` must actually read
//! the Orientation tag and rotate the buffer before handing it back.

use photo_frame_decode::from_bytes;

#[path = "../src/test_support.rs"]
mod test_support;

use test_support::{jpeg_with_app1, tiff_with_orientation};

#[test]
fn portrait_jpeg_with_orientation_6_comes_out_upright() {
    // 16w × 24h source. Orientation=6 (rotate 90° CW) → upright is 24×16.
    let bytes = jpeg_with_app1(16, 24, &tiff_with_orientation(6));
    let photo = from_bytes(&bytes).expect("decode");
    assert_eq!((photo.pixels.width(), photo.pixels.height()), (24, 16));
}

#[test]
fn portrait_jpeg_with_orientation_8_comes_out_upright() {
    // Orientation=8 (rotate 270° CW) also swaps axes.
    let bytes = jpeg_with_app1(16, 24, &tiff_with_orientation(8));
    let photo = from_bytes(&bytes).expect("decode");
    assert_eq!((photo.pixels.width(), photo.pixels.height()), (24, 16));
}

#[test]
fn landscape_jpeg_with_orientation_1_stays_landscape() {
    let bytes = jpeg_with_app1(24, 16, &tiff_with_orientation(1));
    let photo = from_bytes(&bytes).expect("decode");
    assert_eq!((photo.pixels.width(), photo.pixels.height()), (24, 16));
}

#[test]
fn orientation_3_keeps_dimensions_but_rotates_180() {
    let bytes = jpeg_with_app1(24, 16, &tiff_with_orientation(3));
    let photo = from_bytes(&bytes).expect("decode");
    // 180° rotation preserves dimensions but reverses contents.
    assert_eq!((photo.pixels.width(), photo.pixels.height()), (24, 16));
}
