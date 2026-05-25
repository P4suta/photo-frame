//! Coverage for each `DecodeError` variant the public API can return.

use photo_frame_decode::{from_bytes, DecodeError};
use std::error::Error;

#[path = "../src/test_support.rs"]
mod test_support;

use test_support::{jpeg_solid, jpeg_with_app1};

#[test]
fn empty_input_returns_empty_input_variant() {
    let err = from_bytes(&[]).expect_err("empty input must fail");
    assert!(matches!(err, DecodeError::EmptyInput));
}

#[test]
fn random_bytes_return_unknown_format() {
    let bytes = b"this is not an image, just plain text bytes";
    let err = from_bytes(bytes).expect_err("unknown format must fail");
    assert!(matches!(err, DecodeError::UnknownFormat));
}

#[test]
fn truncated_jpeg_returns_jpeg_decode_variant_with_source_chain() {
    // Phase D1 moved the JPEG path off image-crate onto zune-jpeg,
    // so the typed variant changed accordingly. The contract that
    // matters to callers is still preserved: a truncated JPEG
    // surfaces as a `Category::Decode` error with a `source` chain
    // pointing at the underlying decoder's diagnostic.
    let mut jpeg = jpeg_solid(8, 8);
    jpeg.truncate(jpeg.len() / 2);
    let err = from_bytes(&jpeg).expect_err("truncated JPEG must fail decode");
    assert!(matches!(err, DecodeError::JpegDecode(_)));
    assert!(
        err.source().is_some(),
        "JpegDecode variant carries source chain"
    );
}

#[test]
fn corrupt_exif_does_not_fail_decode() {
    // APP1 segment whose payload after the "Exif\0\0" identifier is
    // garbage. The image bytes themselves are still a valid JPEG, so
    // decode must succeed with empty Provenance and a warn event.
    let mut body = b"Exif\x00\x00".to_vec();
    body.extend_from_slice(b"\x00\x01\x02\x03\x04");
    let bytes = jpeg_with_app1(8, 8, &body);
    let photo = from_bytes(&bytes).expect("decode succeeds even with corrupt EXIF");
    assert!(photo.provenance.is_empty());
}

#[cfg(not(feature = "heif"))]
#[test]
fn heic_bytes_without_heif_feature_return_heif_feature_disabled() {
    // Minimal ISO BMFF ftyp box advertising the HEIC major brand.
    let mut bytes = vec![0u8, 0, 0, 0x20];
    bytes.extend_from_slice(b"ftypheic");
    bytes.extend_from_slice(&[0u8; 16]);
    let err = from_bytes(&bytes).expect_err("HEIC without heif feature must fail");
    assert!(matches!(err, DecodeError::HeifFeatureDisabled));
}
