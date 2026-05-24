//! Hits each public `FrameError` variant via the real `frame_image`
//! entry point, so the contract (variant + category + cause message) is
//! locked down end-to-end rather than only at the unit level.

use std::error::Error;

use image::{codecs::jpeg::JpegEncoder, ExtendedColorType, ImageEncoder, Rgb, RgbImage};
use photo_frame_core::{frame_image, ErrorCategory, FrameError, FrameOptions};

/// Smallest valid JPEG we can hand the pipeline so tests that need a
/// *successful* decode can focus on what comes next.
fn one_pixel_jpeg() -> Vec<u8> {
    let img = RgbImage::from_pixel(1, 1, Rgb([0, 0, 0]));
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 90)
        .write_image(img.as_raw(), 1, 1, ExtendedColorType::Rgb8)
        .expect("encoding a 1x1 black JPEG cannot fail");
    out
}

#[test]
fn empty_input_is_rejected_with_named_variant() {
    let err = frame_image(&[], &FrameOptions::default()).expect_err("must reject empty input");
    assert!(matches!(err, FrameError::EmptyInput));
    assert_eq!(err.category(), ErrorCategory::Input);
    assert_eq!(err.to_string(), "input is empty (0 bytes)");
}

#[test]
fn quality_out_of_range_is_caught_by_the_encoder() {
    let opts = FrameOptions {
        jpeg_quality: 0,
        ..FrameOptions::default()
    };
    let err = frame_image(&one_pixel_jpeg(), &opts).expect_err("quality 0 must be rejected");
    match err {
        FrameError::QualityOutOfRange { got, ref valid } => {
            assert_eq!(got, 0);
            assert_eq!(*valid.start(), 1);
            assert_eq!(*valid.end(), 100);
            assert_eq!(err.category(), ErrorCategory::Input);
        },
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn quality_above_one_hundred_is_also_caught() {
    let opts = FrameOptions {
        jpeg_quality: 101,
        ..FrameOptions::default()
    };
    let err = frame_image(&one_pixel_jpeg(), &opts).expect_err("quality 101 must be rejected");
    assert!(matches!(
        err,
        FrameError::QualityOutOfRange { got: 101, .. }
    ));
}

#[test]
fn unrecognised_bytes_surface_as_decode_error_with_source() {
    let garbage = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE];
    let err = frame_image(&garbage, &FrameOptions::default()).expect_err("decode must fail");
    assert!(matches!(err, FrameError::Decode(_)), "got {err:?}");
    assert_eq!(err.category(), ErrorCategory::Decode);
    // Cause chain must be intact.
    assert!(
        err.source().is_some(),
        "decode error must preserve image::ImageError source"
    );
}

#[test]
fn category_round_trip_covers_every_known_variant() {
    let cases = [
        (FrameError::EmptyInput, ErrorCategory::Input),
        (
            FrameError::QualityOutOfRange {
                got: 0,
                valid: 1..=100,
            },
            ErrorCategory::Input,
        ),
        (
            FrameError::ZeroDimension {
                width: 0,
                height: 0,
            },
            ErrorCategory::Layout,
        ),
    ];
    for (err, expected) in cases {
        assert_eq!(err.category(), expected, "{err:?}");
    }
}
