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
    if matches!(fmt, image::ImageFormat::Jpeg) {
        return decode_jpeg_zune(bytes);
    }
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

/// JPEG-only fast path: drive `zune-jpeg` for the IDCT + colour
/// conversion, then route the resulting RGBA buffer through the
/// existing EXIF / orientation pass. zune is ~30–50 % faster than
/// image-crate's `jpeg-decoder` on `x86_64` because its YCbCr→RGB
/// conversion uses SSE/AVX2 when available; it falls back to scalar
/// on wasm32 so the WASM build stays portable.
fn decode_jpeg_zune(bytes: &[u8]) -> Result<Photograph, DecodeError> {
    use image::{ImageBuffer, Rgba};
    use zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions};
    use zune_jpeg::JpegDecoder;

    // `JpegDecoder<T>` is generic over `ZByteReaderTrait`; `ZCursor<&[u8]>`
    // is the no-std cursor that satisfies the trait. `&[u8]` directly
    // doesn't impl it.
    let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), opts);
    let rgba = decoder.decode().map_err(DecodeError::JpegDecode)?;
    let info = decoder
        .info()
        .expect("zune-jpeg: info populated after successful decode");
    let width = u32::from(info.width);
    let height = u32::from(info.height);

    // Re-use the existing orientation pass. Wrapping the raw bytes in
    // an `ImageBuffer` is zero-copy (it just takes ownership of the
    // Vec); `DynamicImage::ImageRgba8` is a tagged wrapper.
    let buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, rgba)
        .expect("zune-jpeg returned width * height * 4 bytes by construction");
    let mut img = image::DynamicImage::ImageRgba8(buffer);

    let exif_outcome = exif::read(bytes);
    let exif_present = exif_outcome.was_present();
    let orientation_raw = exif::orientation_value(exif_outcome.as_parsed());
    orientation::apply(&mut img, orientation_raw);

    let rgba = img.into_rgba8();
    let (final_w, final_h) = rgba.dimensions();
    let pixels = Pixels::from_rgba8(final_w, final_h, rgba.into_raw())?;

    let span = tracing::Span::current();
    span.record("width", final_w);
    span.record("height", final_h);
    span.record("exif_present", exif_present);

    let provenance = exif_outcome
        .as_parsed()
        .map_or_else(Provenance::default, provenance::extract);
    Ok(Photograph::new(pixels, provenance))
}
