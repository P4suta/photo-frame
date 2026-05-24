//! Encode a [`Pixels`] grid into a chosen output format.
//!
//! Currently only JPEG. The crate exists in its own crate so a future
//! alternative encoder (lossy WebP once image-rs ships one, or a
//! CLI-only mozjpeg path that opts out of the Pure-Rust contract) can
//! drop in without touching the renderer or the pipeline orchestrator.
//!
//! [`Pixels`]: photo_frame_types::Pixels

use std::ops::RangeInclusive;

use image::{codecs::jpeg::JpegEncoder, ExtendedColorType, ImageEncoder};
use miette::Diagnostic;
use photo_frame_types::{Categorize, Category, Pixels};
use thiserror::Error;

const VALID_QUALITY: RangeInclusive<u8> = 1..=100;

/// Knobs for [`jpeg`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct JpegOptions {
    /// JPEG quality, 1..=100. Default 92 — visually transparent at the
    /// downsample-and-zoom sizes most viewers use.
    pub quality: u8,
}

impl JpegOptions {
    /// Default quality (92), made `const` so callers can derive presets
    /// at compile time.
    pub const DEFAULT: Self = Self { quality: 92 };
}

impl Default for JpegOptions {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Why encoding failed.
#[derive(Debug, Error, Diagnostic)]
pub enum EncodeError {
    #[error("JPEG quality {got} out of range; expected {}..={}", VALID_QUALITY.start(), VALID_QUALITY.end())]
    #[diagnostic(
        code(photo_frame::encode::invalid_quality),
        help("JPEG quality must be 1..=100. Try a value like 78 (SNS), 92 (standard), 98 (max).")
    )]
    InvalidQuality { got: u8 },

    #[error("JPEG encoder failed")]
    #[diagnostic(
        code(photo_frame::encode::encoder_error),
        help(
            "The image crate's JPEG encoder reported an error. Typical \
             causes: out-of-memory on very large canvases, or a malformed \
             pixel buffer. The wrapped error has the format-specific reason."
        )
    )]
    Encode(#[source] image::ImageError),
}

impl Categorize for EncodeError {
    fn category(&self) -> Category {
        match self {
            Self::InvalidQuality { .. } => Category::Input,
            Self::Encode(_) => Category::Encode,
        }
    }
}

/// Encode `pixels` to JPEG bytes.
///
/// JPEG has no alpha channel, so the alpha samples in [`Pixels`] are
/// dropped before encoding. Callers that care about alpha must compose
/// against a background colour upstream (the frame crate already does
/// this when laying out the canvas).
///
/// # Errors
/// - [`EncodeError::InvalidQuality`] when `opts.quality` is outside 1..=100.
/// - [`EncodeError::Encode`] when the underlying JPEG encoder fails.
#[tracing::instrument(
    level = "debug",
    name = "encode_jpeg",
    skip(pixels),
    fields(
        width = pixels.width(),
        height = pixels.height(),
        quality = opts.quality,
        output_bytes = tracing::field::Empty,
    ),
)]
pub fn jpeg(pixels: &Pixels, opts: &JpegOptions) -> Result<Vec<u8>, EncodeError> {
    if !VALID_QUALITY.contains(&opts.quality) {
        return Err(EncodeError::InvalidQuality { got: opts.quality });
    }
    let rgb = drop_alpha(pixels);
    let mut out = Vec::with_capacity(rgb.len() / 4);
    JpegEncoder::new_with_quality(&mut out, opts.quality)
        .write_image(
            &rgb,
            pixels.width(),
            pixels.height(),
            ExtendedColorType::Rgb8,
        )
        .map_err(EncodeError::Encode)?;
    tracing::Span::current().record("output_bytes", out.len());
    Ok(out)
}

/// Pack an RGBA8 buffer into RGB8 by dropping the alpha sample.
fn drop_alpha(pixels: &Pixels) -> Vec<u8> {
    let rgba = pixels.as_rgba8();
    let pixel_count = (pixels.width() as usize) * (pixels.height() as usize);
    let mut out = Vec::with_capacity(pixel_count * 3);
    for chunk in rgba.chunks_exact(4) {
        out.extend_from_slice(&chunk[..3]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{jpeg, EncodeError, JpegOptions, VALID_QUALITY};
    use image::ImageReader;
    use photo_frame_types::Pixels;
    use std::io::Cursor;

    fn solid_pixels(w: u32, h: u32) -> Pixels {
        let mut buf = Vec::with_capacity((w as usize) * (h as usize) * 4);
        for _ in 0..(w * h) {
            buf.extend_from_slice(&[200, 60, 60, 255]);
        }
        Pixels::from_rgba8(w, h, buf).expect("pixels")
    }

    #[test]
    fn default_quality_is_92() {
        assert_eq!(JpegOptions::default().quality, 92);
        assert_eq!(JpegOptions::DEFAULT.quality, 92);
    }

    #[test]
    fn jpeg_round_trips_to_expected_dimensions() {
        let p = solid_pixels(16, 8);
        let bytes = jpeg(&p, &JpegOptions::default()).expect("encode");
        let decoded = ImageReader::new(Cursor::new(&bytes))
            .with_guessed_format()
            .expect("guess")
            .decode()
            .expect("decode");
        assert_eq!(decoded.width(), 16);
        assert_eq!(decoded.height(), 8);
    }

    #[test]
    fn quality_zero_is_rejected() {
        let p = solid_pixels(4, 4);
        let err = jpeg(&p, &JpegOptions { quality: 0 }).expect_err("must reject");
        match err {
            EncodeError::InvalidQuality { got } => assert_eq!(got, 0),
            EncodeError::Encode(_) => panic!("expected InvalidQuality, got Encode"),
        }
    }

    #[test]
    fn quality_above_max_is_rejected() {
        let p = solid_pixels(4, 4);
        let err = jpeg(
            &p,
            &JpegOptions {
                quality: VALID_QUALITY.end() + 1,
            },
        )
        .expect_err("must reject");
        assert!(matches!(err, EncodeError::InvalidQuality { .. }));
    }

    #[test]
    fn smaller_quality_produces_smaller_output() {
        // Solid-colour input is largely entropy-free, so the size delta is
        // small, but quality=10 must still beat quality=95.
        let p = solid_pixels(128, 128);
        let q10 = jpeg(&p, &JpegOptions { quality: 10 }).expect("q10");
        let q95 = jpeg(&p, &JpegOptions { quality: 95 }).expect("q95");
        assert!(
            q10.len() <= q95.len(),
            "q10={} q95={}",
            q10.len(),
            q95.len()
        );
    }

    #[test]
    fn output_starts_with_jpeg_soi_marker() {
        let p = solid_pixels(4, 4);
        let bytes = jpeg(&p, &JpegOptions::default()).expect("encode");
        assert_eq!(&bytes[..2], &[0xFF, 0xD8], "JPEG SOI marker");
    }
}
