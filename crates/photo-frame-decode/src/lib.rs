//! Decode arbitrary input bytes into a canonical [`Photograph`].
//!
//! This crate owns the "left half" of the pipeline: it absorbs the variety
//! of input formats (JPEG / PNG / TIFF / BMP / WebP by default, HEIC under
//! the opt-in `heif` feature) and yields a single, normalized intermediate
//! — orientation already applied, EXIF metadata broken out into primitive
//! [`Provenance`] fields. Downstream crates (`photo-frame-frame`,
//! `photo-frame-encode`) never have to think about input formats.
//!
//! [`Provenance`]: photo_frame_types::Provenance

mod error;
mod exif;
mod format;
mod heif;
mod orientation;
mod provenance;
#[cfg(test)]
pub(crate) mod test_support;

pub use crate::error::DecodeError;
pub use photo_frame_types::Photograph;

use crate::format::DetectedFormat;
use photo_frame_types::{Pixels, Provenance};

/// Decode an image into a [`Photograph`].
///
/// The returned pixels are RGBA8, row-major, with EXIF Orientation already
/// applied so the top-left of the buffer matches the top-left a viewer
/// would expect. EXIF metadata is parsed into a [`Provenance`]; missing
/// or malformed EXIF degrades gracefully to `Provenance::default()` rather
/// than failing the decode.
///
/// # Errors
/// Returns a [`DecodeError`] when the bytes are empty, the format cannot
/// be determined, decoding fails, HEIC bytes arrive without the `heif`
/// feature enabled, or the decoded pixel buffer fails the [`Pixels`]
/// invariant checks.
///
/// [`Provenance`]: photo_frame_types::Provenance
/// [`Pixels`]: photo_frame_types::Pixels
#[tracing::instrument(
    level = "info",
    name = "decode",
    skip(bytes),
    fields(
        input_bytes = bytes.len(),
        format = tracing::field::Empty,
        width = tracing::field::Empty,
        height = tracing::field::Empty,
        exif_present = tracing::field::Empty,
    ),
)]
pub fn from_bytes(bytes: &[u8]) -> Result<Photograph, DecodeError> {
    if bytes.is_empty() {
        return Err(DecodeError::EmptyInput);
    }
    let detected = format::detect(bytes);
    let span = tracing::Span::current();
    span.record("format", format::name(detected));
    match detected {
        DetectedFormat::Heic => decode_heic_path(bytes),
        DetectedFormat::Image(fmt) => decode_image(bytes, fmt),
        DetectedFormat::Unknown => Err(DecodeError::UnknownFormat),
    }
}

#[cfg(feature = "heif")]
fn decode_heic_path(bytes: &[u8]) -> Result<Photograph, DecodeError> {
    let (mut img, exif_outcome) = heif::decode_heic(bytes)?;
    let exif_present = exif_outcome.was_present();
    let orientation_raw = exif::orientation_value(exif_outcome.as_parsed());
    orientation::apply(&mut img, orientation_raw);
    let rgba = img.into_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels = Pixels::from_rgba8(width, height, rgba.into_raw())?;
    let span = tracing::Span::current();
    span.record("width", width);
    span.record("height", height);
    span.record("exif_present", exif_present);
    let provenance = exif_outcome
        .as_parsed()
        .map_or_else(Provenance::default, provenance::extract);
    Ok(Photograph::new(pixels, provenance))
}

#[cfg(not(feature = "heif"))]
const fn decode_heic_path(_bytes: &[u8]) -> Result<Photograph, DecodeError> {
    Err(DecodeError::HeifFeatureDisabled)
}

fn decode_image(bytes: &[u8], fmt: image::ImageFormat) -> Result<Photograph, DecodeError> {
    let mut img = image::load_from_memory_with_format(bytes, fmt).map_err(DecodeError::Decode)?;
    let exif_outcome = exif::read(bytes);
    let exif_present = exif_outcome.was_present();
    let orientation_raw = exif::orientation_value(exif_outcome.as_parsed());
    orientation::apply(&mut img, orientation_raw);
    let rgba = img.into_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels = Pixels::from_rgba8(width, height, rgba.into_raw())?;
    let span = tracing::Span::current();
    span.record("width", width);
    span.record("height", height);
    span.record("exif_present", exif_present);
    let provenance = exif_outcome
        .as_parsed()
        .map_or_else(Provenance::default, provenance::extract);
    Ok(Photograph::new(pixels, provenance))
}
