//! HEIC / HEIF decode via libheif.
//!
//! The libheif glue lives behind the `heif` cargo feature — libheif is a
//! system C library, and enabling it opts the build out of the workspace's
//! "Pure Rust" contract (see `deny.toml`). The *pure* helpers — packed
//! row copy and the HEIF EXIF-blob prefix strip — are always compiled so
//! they can be unit-tested without libheif installed, and so they live
//! next to the libheif call that drives them.
//!
//! HEIC bytes flow through three steps before merging with the JPEG/PNG
//! path:
//!
//! 1. **Decode pixels.** libheif hands back an interleaved RGBA plane. We
//!    instruct it to *not* apply EXIF transformations
//!    (`set_ignore_transformations(true)`) so the orientation tag is
//!    handled in exactly one place — [`crate::orientation::apply`] — just
//!    like the JPEG path.
//! 2. **Repack with stride.** libheif's plane stride can exceed
//!    `width * 4` (alignment padding). The decoder above us assumes
//!    `Vec<u8>` is packed `width * height * 4`, so we copy row-by-row.
//! 3. **Strip the EXIF prefix.** HEIF wraps the EXIF item in a 4-byte
//!    big-endian offset (ISO/IEC 23008-12 §A.2.1). kamadak-exif's
//!    `read_raw` expects a bare TIFF header, so we drop those four bytes
//!    before handing the blob off.

#[cfg(feature = "heif")]
pub(crate) use libheif_glue::decode_heic;

/// Strip the 4-byte big-endian TIFF header offset HEIF wraps EXIF in.
/// Returns `None` if the blob is too short to contain the prefix.
#[cfg_attr(
    not(feature = "heif"),
    allow(dead_code, reason = "only the heif decode path consumes this helper")
)]
fn strip_heif_exif_prefix(blob: &[u8]) -> Option<&[u8]> {
    if blob.len() < 4 {
        return None;
    }
    Some(&blob[4..])
}

/// Copy a possibly-strided RGBA8 plane into a packed `width * height * 4`
/// buffer suitable for `Pixels::from_rgba8`.
#[cfg_attr(
    not(feature = "heif"),
    allow(dead_code, reason = "only the heif decode path consumes this helper")
)]
fn copy_rgba_with_stride(data: &[u8], stride: usize, width: u32, height: u32) -> Vec<u8> {
    let row_bytes = rgba_row_len(width);
    let total = row_bytes * (height as usize);

    #[cfg(feature = "heif")]
    if stride > row_bytes {
        tracing::debug!(
            event_id = "decode.heif.stride_padding",
            stride,
            packed_row = row_bytes,
            "libheif plane has alignment padding; repacking row-by-row"
        );
    }

    let mut out = Vec::with_capacity(total);
    for y in 0..(height as usize) {
        let start = y * stride;
        let end = start + row_bytes;
        // Defensive: if the source slice is shorter than the declared
        // stride×height (corrupt libheif output), stop early — the caller
        // will get a DataSizeMismatch from `Pixels::from_rgba8`.
        if end > data.len() {
            #[cfg(feature = "heif")]
            tracing::warn!(
                event_id = "decode.heif.truncated",
                expected_rows = height,
                got_rows = y,
                "libheif plane data shorter than declared stride*height; truncating"
            );
            break;
        }
        out.extend_from_slice(&data[start..end]);
    }
    out
}

const fn rgba_row_len(width: u32) -> usize {
    (width as usize) * 4
}

#[cfg(feature = "heif")]
mod libheif_glue {
    use super::{copy_rgba_with_stride, rgba_row_len, strip_heif_exif_prefix};
    use crate::{
        exif::{self, ExifReadOutcome},
        DecodeError,
    };
    use image::{DynamicImage, ImageBuffer, Rgba};
    use libheif_rs::{
        ColorSpace, DecodingOptions, HeifContext, ImageHandle, ItemId, LibHeif, RgbChroma,
    };
    use photo_frame_types::PixelError;

    #[tracing::instrument(
        level = "debug",
        name = "heif_decode",
        skip(bytes),
        fields(
            bytes = bytes.len(),
            width = tracing::field::Empty,
            height = tracing::field::Empty,
            stride = tracing::field::Empty,
        ),
    )]
    pub(crate) fn decode_heic(
        bytes: &[u8],
    ) -> Result<(DynamicImage, ExifReadOutcome), DecodeError> {
        let lib = LibHeif::new();
        let ctx = HeifContext::read_from_bytes(bytes).map_err(DecodeError::HeifDecode)?;
        let handle = ctx
            .primary_image_handle()
            .map_err(DecodeError::HeifDecode)?;
        let (width, height, packed) = decode_pixels(&lib, &handle)?;
        let exif_outcome = read_exif(&handle);

        let expected = rgba_row_len(width) * (height as usize);
        let buf = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, packed).ok_or_else(|| {
            DecodeError::InvalidPixels(PixelError::DataSizeMismatch {
                width,
                height,
                got: 0,
                expected,
            })
        })?;
        Ok((DynamicImage::ImageRgba8(buf), exif_outcome))
    }

    fn decode_pixels(
        lib: &LibHeif,
        handle: &ImageHandle,
    ) -> Result<(u32, u32, Vec<u8>), DecodeError> {
        let mut opts = DecodingOptions::new().ok_or_else(|| {
            DecodeError::Decode(image::ImageError::Decoding(
                image::error::DecodingError::new(
                    image::error::ImageFormatHint::Unknown,
                    "libheif decoding options allocation failed",
                ),
            ))
        })?;
        opts.set_ignore_transformations(true);

        let img = lib
            .decode(handle, ColorSpace::Rgb(RgbChroma::Rgba), Some(opts))
            .map_err(DecodeError::HeifDecode)?;

        let planes = img.planes();
        let plane = planes.interleaved.ok_or_else(|| {
            DecodeError::Decode(image::ImageError::Decoding(
                image::error::DecodingError::new(
                    image::error::ImageFormatHint::Unknown,
                    "libheif returned a non-interleaved RGBA plane",
                ),
            ))
        })?;

        let span = tracing::Span::current();
        span.record("width", plane.width);
        span.record("height", plane.height);
        span.record("stride", plane.stride);

        let packed = copy_rgba_with_stride(plane.data, plane.stride, plane.width, plane.height);
        Ok((plane.width, plane.height, packed))
    }

    fn read_exif(handle: &ImageHandle) -> ExifReadOutcome {
        let count = handle.number_of_metadata_blocks(b"Exif");
        if count <= 0 {
            return ExifReadOutcome::Absent;
        }
        let Ok(count) = usize::try_from(count) else {
            return ExifReadOutcome::Absent;
        };
        let mut ids = vec![ItemId::default(); count];
        let written = handle.metadata_block_ids(&mut ids, b"Exif");
        let Some(&id) = ids.get(..written).and_then(<[ItemId]>::first) else {
            return ExifReadOutcome::Absent;
        };
        let raw = match handle.metadata(id) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(?error, "HEIF EXIF item present but unreadable");
                return ExifReadOutcome::Absent;
            },
        };
        let Some(tiff) = strip_heif_exif_prefix(&raw) else {
            return ExifReadOutcome::Absent;
        };
        exif::read_raw(tiff)
    }
}

#[cfg(test)]
mod tests {
    use super::{copy_rgba_with_stride, strip_heif_exif_prefix};

    #[test]
    fn strips_4byte_prefix() {
        let blob = [0u8, 0, 0, 8, 0x4D, 0x4D, 0, 42];
        let stripped = strip_heif_exif_prefix(&blob).expect("present");
        assert_eq!(stripped, &[0x4D, 0x4D, 0, 42]);
    }

    #[test]
    fn strip_too_short_returns_none() {
        assert!(strip_heif_exif_prefix(&[]).is_none());
        assert!(strip_heif_exif_prefix(&[0, 0, 0]).is_none());
    }

    #[test]
    fn strip_exactly_4_bytes_returns_empty_slice() {
        let stripped = strip_heif_exif_prefix(&[0, 0, 0, 0]).expect("present");
        assert!(stripped.is_empty());
    }

    #[test]
    fn copy_packed_buffer_is_identity() {
        // No padding: stride == width * 4.
        let data: Vec<u8> = (0..32).collect(); // 4×2 RGBA = 32 bytes
        let out = copy_rgba_with_stride(&data, 16, 4, 2);
        assert_eq!(out, data);
    }

    #[test]
    fn copy_strided_buffer_drops_padding() {
        // 3 wide × 2 tall, stride is 16 (padded to 4-pixel alignment).
        // Row 0: bytes 0..12 (data) + 12..16 (padding)
        // Row 1: bytes 16..28 (data) + 28..32 (padding)
        let mut src = vec![0u8; 32];
        for (i, byte) in src.iter_mut().take(12).enumerate() {
            *byte = u8::try_from(i + 1).unwrap();
        }
        for (i, byte) in src.iter_mut().skip(16).take(12).enumerate() {
            *byte = u8::try_from(100 + i).unwrap();
        }
        let out = copy_rgba_with_stride(&src, 16, 3, 2);
        assert_eq!(out.len(), 3 * 2 * 4);
        assert_eq!(&out[..12], &(1u8..=12).collect::<Vec<_>>()[..]);
        let expected_row1: Vec<u8> = (100u8..=111).collect();
        assert_eq!(&out[12..], &expected_row1[..]);
    }

    #[test]
    fn copy_truncated_source_stops_early_without_panic() {
        // Declared 4×4 with stride 16, but source only carries 2 rows
        // worth of bytes. Helper must return what it can without panicking.
        let src = vec![0u8; 32];
        let out = copy_rgba_with_stride(&src, 16, 4, 4);
        assert_eq!(out.len(), 32);
    }
}
