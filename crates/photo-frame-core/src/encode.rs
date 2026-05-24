//! JPEG encoding boundary. Lives in its own module so a future feature-gated
//! encoder (mozjpeg on the CLI) can drop in without touching the pipeline.

use std::ops::RangeInclusive;

use image::{codecs::jpeg::JpegEncoder, ExtendedColorType, ImageEncoder, RgbImage};
use tracing::{instrument, trace};

use crate::error::FrameError;

const VALID_QUALITY: RangeInclusive<u8> = 1..=100;

#[instrument(level = "debug", skip(img), fields(width = img.width(), height = img.height()))]
pub(crate) fn jpeg(img: &RgbImage, quality: u8) -> Result<Vec<u8>, FrameError> {
    if !VALID_QUALITY.contains(&quality) {
        return Err(FrameError::QualityOutOfRange {
            got: quality,
            valid: VALID_QUALITY,
        });
    }
    let mut out = Vec::with_capacity(img.as_raw().len() / 4);
    JpegEncoder::new_with_quality(&mut out, quality)
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            ExtendedColorType::Rgb8,
        )
        .map_err(FrameError::Encode)?;
    trace!(bytes = out.len(), "JPEG encoded");
    Ok(out)
}
