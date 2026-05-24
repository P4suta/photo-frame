//! End-to-end check that EXIF Orientation flows through the public pipeline.
//!
//! The unit tests under `src/orientation.rs` verify the eight rotations in
//! isolation, and the unit tests under `src/exif.rs` cover tag formatting.
//! What neither covers is the *wiring*: that `frame_image()` actually reads
//! the Orientation tag and applies the corresponding rotation before laying
//! out the canvas. This file pins that wiring down so a refactor can't
//! silently disconnect it.
//!
//! We construct the JPEG fixture in-process rather than committing a binary
//! blob — keeps the test self-explanatory and the repo free of opaque bytes.

use image::{
    codecs::jpeg::JpegEncoder, ExtendedColorType, ImageEncoder, ImageReader, Rgb, RgbImage,
};
use photo_frame_core::{frame_image, FrameOptions};

#[test]
fn portrait_jpeg_with_orientation_6_comes_out_upright() {
    // Source pixels: 16 wide × 24 tall. With Orientation=6 (rotate 90° CW)
    // the visually-upright image is 24 wide × 16 tall.
    let input = jpeg_with_orientation(16, 24, 6);

    let framed = frame_image(&input, &FrameOptions::default()).expect("frame succeeds");

    // Reload the output to recover dimensions.
    let out = ImageReader::new(std::io::Cursor::new(&framed))
        .with_guessed_format()
        .expect("guess output format")
        .decode()
        .expect("decode framed output");

    // After normalisation the photo is 24×16. There is no captionable EXIF
    // (only Orientation), so the metadata strip collapses and the bottom
    // border matches the side. min_dim=16, so side falls back to MIN_SIDE_PX=8.
    let expected_side = 8;
    let expected_w = 24 + 2 * expected_side;
    let expected_h = 16 + 2 * expected_side;
    assert_eq!(
        (out.width(), out.height()),
        (expected_w, expected_h),
        "framed dimensions show the photo was rotated before layout",
    );
}

#[test]
fn portrait_jpeg_with_orientation_8_comes_out_upright() {
    // Orientation=8 also swaps axes (rotate 270° CW).
    let input = jpeg_with_orientation(16, 24, 8);
    let framed = frame_image(&input, &FrameOptions::default()).expect("frame succeeds");
    let out = ImageReader::new(std::io::Cursor::new(&framed))
        .with_guessed_format()
        .expect("guess output format")
        .decode()
        .expect("decode framed output");
    assert_eq!((out.width(), out.height()), (40, 32));
}

#[test]
fn normal_orientation_does_not_swap_axes() {
    // Sanity baseline: Orientation=1 must leave a landscape source landscape.
    let input = jpeg_with_orientation(24, 16, 1);
    let framed = frame_image(&input, &FrameOptions::default()).expect("frame succeeds");
    let out = ImageReader::new(std::io::Cursor::new(&framed))
        .with_guessed_format()
        .expect("guess output format")
        .decode()
        .expect("decode framed output");
    assert_eq!((out.width(), out.height()), (40, 32));
}

/// Build a minimal valid JPEG of size `w × h` carrying an APP1 (EXIF) segment
/// whose only tag is Orientation = `orientation`.
fn jpeg_with_orientation(w: u32, h: u32, orientation: u16) -> Vec<u8> {
    let solid: RgbImage = RgbImage::from_pixel(w, h, Rgb([200, 60, 60]));
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 90)
        .write_image(&solid, w, h, ExtendedColorType::Rgb8)
        .expect("jpeg encode");

    let exif_segment = build_exif_app1(orientation);

    // Insert APP1 immediately after SOI (the first two bytes).
    let mut out = Vec::with_capacity(jpeg.len() + exif_segment.len() + 4);
    out.extend_from_slice(&jpeg[..2]); // SOI
    out.push(0xFF);
    out.push(0xE1); // APP1
    let segment_len = u16::try_from(exif_segment.len() + 2).expect("APP1 length fits u16");
    out.extend_from_slice(&segment_len.to_be_bytes());
    out.extend_from_slice(&exif_segment);
    out.extend_from_slice(&jpeg[2..]);
    out
}

/// Build the body of an APP1 EXIF segment (everything after the
/// `FF E1 <length>` header) carrying exactly one tag: Orientation.
fn build_exif_app1(orientation: u16) -> Vec<u8> {
    let mut buf = Vec::with_capacity(32);
    buf.extend_from_slice(b"Exif\x00\x00"); // EXIF identifier
    buf.extend_from_slice(b"MM"); // big-endian TIFF
    buf.extend_from_slice(&0x002A_u16.to_be_bytes()); // TIFF magic
    buf.extend_from_slice(&8_u32.to_be_bytes()); // offset to IFD0 from TIFF header start

    // IFD0: one entry, no next IFD.
    buf.extend_from_slice(&1_u16.to_be_bytes()); // entry count
    buf.extend_from_slice(&0x0112_u16.to_be_bytes()); // tag = Orientation
    buf.extend_from_slice(&3_u16.to_be_bytes()); // type = SHORT
    buf.extend_from_slice(&1_u32.to_be_bytes()); // count
                                                 // SHORT value is 2 bytes left-aligned in a 4-byte field (big-endian).
    buf.extend_from_slice(&orientation.to_be_bytes());
    buf.extend_from_slice(&[0_u8, 0]); // padding
    buf.extend_from_slice(&0_u32.to_be_bytes()); // no further IFD
    buf
}
