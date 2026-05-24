//! End-to-end PNG decode: bytes in, Photograph out.

use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder, RgbaImage};
use photo_frame_decode::from_bytes;

fn tiny_png(w: u32, h: u32) -> Vec<u8> {
    let img = RgbaImage::from_pixel(w, h, image::Rgba([10, 20, 30, 255]));
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(&img, w, h, ExtendedColorType::Rgba8)
        .expect("png encode");
    out
}

#[test]
fn tiny_png_returns_default_provenance() {
    let bytes = tiny_png(4, 3);
    let photo = from_bytes(&bytes).expect("decode");
    assert_eq!(photo.pixels.width(), 4);
    assert_eq!(photo.pixels.height(), 3);
    // PNG has no EXIF — provenance must be empty so MetaPolicy::Auto
    // downstream can collapse the caption strip.
    assert!(photo.provenance.is_empty());
}

#[test]
fn png_pixels_are_rgba_packed() {
    let bytes = tiny_png(2, 1);
    let photo = from_bytes(&bytes).expect("decode");
    let buf = photo.pixels.as_rgba8();
    assert_eq!(buf.len(), 2 * 4); // 2 pixels × 4 bytes
                                  // The first pixel matches the solid fill we wrote.
    assert_eq!(&buf[..4], &[10, 20, 30, 255]);
}
