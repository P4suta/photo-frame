//! Thin wrapper around [`image::load_from_memory`] that also pulls EXIF out of
//! the raw byte stream so the orientation step can run before any pixel
//! transformation.

use image::DynamicImage;
use tracing::{debug, instrument};

use crate::error::FrameError;

#[instrument(level = "debug", skip(bytes), fields(input_bytes = bytes.len()))]
pub(crate) fn decode(bytes: &[u8]) -> Result<(DynamicImage, Option<::exif::Exif>), FrameError> {
    let img = image::load_from_memory(bytes).map_err(FrameError::Decode)?;
    let exif = crate::exif::read(bytes);
    debug!(
        width = img.width(),
        height = img.height(),
        color = ?img.color(),
        exif_present = exif.is_some(),
        "decoded",
    );
    Ok((img, exif))
}
