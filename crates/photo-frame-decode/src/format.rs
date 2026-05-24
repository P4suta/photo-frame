//! Sniff the input format from its first bytes.
//!
//! We check the HEIC family (ISO BMFF `ftyp` box) *before* delegating to
//! `image::guess_format`, because the `image` crate does not recognize HEIF
//! and would return `Unsupported`. Splitting the detection lets the caller
//! cleanly route HEIC to the (feature-gated) libheif path and everything
//! else to the pure-Rust `image` path.

use image::ImageFormat;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DetectedFormat {
    /// HEIC / HEIF family. Decoded via `libheif-rs` under `heif`.
    Heic,
    /// One of the formats the `image` crate handles natively.
    Image(ImageFormat),
    Unknown,
}

pub(crate) fn detect(bytes: &[u8]) -> DetectedFormat {
    if is_heic(bytes) {
        return DetectedFormat::Heic;
    }
    // `image::guess_format` recognises magic for formats whose decoders we
    // may not have compiled in (AVIF, HDR, OpenEXR …). Filter to the exact
    // set the workspace enables in `Cargo.toml` to avoid handing the caller
    // a `DetectedFormat` we cannot actually decode.
    match image::guess_format(bytes) {
        Ok(fmt) if is_supported(fmt) => DetectedFormat::Image(fmt),
        Ok(fmt) => {
            tracing::debug!(
                event_id = "decode.format.unsupported",
                detected = ?fmt,
                "image::guess_format recognised an unsupported variant; treating as Unknown"
            );
            DetectedFormat::Unknown
        },
        Err(_) => DetectedFormat::Unknown,
    }
}

const fn is_supported(fmt: ImageFormat) -> bool {
    matches!(
        fmt,
        ImageFormat::Jpeg
            | ImageFormat::Png
            | ImageFormat::Tiff
            | ImageFormat::Bmp
            | ImageFormat::WebP
    )
}

/// Short, stable name used in tracing fields. Keeps the field domain to a
/// small, predictable string set (`jpeg`, `png`, `tiff`, `bmp`, `webp`,
/// `heic`, `unknown`).
pub(crate) const fn name(detected: DetectedFormat) -> &'static str {
    match detected {
        DetectedFormat::Heic => "heic",
        DetectedFormat::Image(ImageFormat::Jpeg) => "jpeg",
        DetectedFormat::Image(ImageFormat::Png) => "png",
        DetectedFormat::Image(ImageFormat::Tiff) => "tiff",
        DetectedFormat::Image(ImageFormat::Bmp) => "bmp",
        DetectedFormat::Image(ImageFormat::WebP) => "webp",
        DetectedFormat::Unknown => "unknown",
        // `detect` filters to the variants above, so this arm is structurally
        // unreachable. Keeping it explicit makes the `name` function total
        // without relying on a wildcard that could quietly mislabel a new
        // `ImageFormat` variant added in a future `image` release.
        DetectedFormat::Image(_) => "other",
    }
}

/// ISO/IEC 14496-12 `ftyp` box check. The first 12 bytes are:
///
/// ```text
/// [0..4]   = box size  (any value)
/// [4..8]   = "ftyp"
/// [8..12]  = major brand
/// ```
///
/// We treat the following major brands as HEIC: `heic`, `heix`, `hevc`,
/// `hevx`, `heim`, `heis`, `hevm`, `hevs`, `mif1`, `msf1`. AVIF (`avif`,
/// `avis`) is deliberately *not* included — when AVIF support arrives it
/// will get its own code path; today it is treated as Unknown so it
/// surfaces as a clean error instead of silently going down the HEIC path.
fn is_heic(bytes: &[u8]) -> bool {
    if bytes.len() < 12 {
        return false;
    }
    if &bytes[4..8] != b"ftyp" {
        return false;
    }
    matches!(
        &bytes[8..12],
        b"heic"
            | b"heix"
            | b"hevc"
            | b"hevx"
            | b"heim"
            | b"heis"
            | b"hevm"
            | b"hevs"
            | b"mif1"
            | b"msf1"
    )
}

#[cfg(test)]
mod tests {
    use super::{detect, name, DetectedFormat};
    use image::ImageFormat;

    fn ftyp(brand: [u8; 4]) -> Vec<u8> {
        let mut v = vec![0u8, 0, 0, 0x20]; // size field (arbitrary)
        v.extend_from_slice(b"ftyp");
        v.extend_from_slice(&brand);
        // pad to look like a real box; detection only reads first 12 bytes
        v.extend(std::iter::repeat(0u8).take(16));
        v
    }

    #[test]
    fn detect_jpeg_via_image_guess() {
        let bytes = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(detect(&bytes), DetectedFormat::Image(ImageFormat::Jpeg));
    }

    #[test]
    fn detect_png_via_image_guess() {
        let bytes = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(detect(&bytes), DetectedFormat::Image(ImageFormat::Png));
    }

    #[test]
    fn detect_heic_ftypheic() {
        assert_eq!(detect(&ftyp(*b"heic")), DetectedFormat::Heic);
    }

    #[test]
    fn detect_heic_ftypheix() {
        assert_eq!(detect(&ftyp(*b"heix")), DetectedFormat::Heic);
    }

    #[test]
    fn detect_heic_ftyphevc() {
        assert_eq!(detect(&ftyp(*b"hevc")), DetectedFormat::Heic);
    }

    #[test]
    fn detect_heic_ftypmif1() {
        assert_eq!(detect(&ftyp(*b"mif1")), DetectedFormat::Heic);
    }

    #[test]
    fn detect_avif_is_unknown() {
        // AVIF is a future code path. Until then, it should not silently
        // be routed to the HEIC pipeline; surface it as Unknown.
        assert_eq!(detect(&ftyp(*b"avif")), DetectedFormat::Unknown);
    }

    #[test]
    fn detect_empty_is_unknown() {
        assert_eq!(detect(&[]), DetectedFormat::Unknown);
    }

    #[test]
    fn detect_random_bytes_is_unknown() {
        let bytes = b"This is plainly not an image file.";
        assert_eq!(detect(bytes), DetectedFormat::Unknown);
    }

    #[test]
    fn detect_short_ftyp_is_not_heic() {
        // Less than 12 bytes — must not classify as HEIC.
        let bytes = [0, 0, 0, 0, b'f', b't', b'y', b'p'];
        assert_eq!(detect(&bytes), DetectedFormat::Unknown);
    }

    #[test]
    fn name_covers_known_formats() {
        assert_eq!(name(DetectedFormat::Heic), "heic");
        assert_eq!(name(DetectedFormat::Image(ImageFormat::Jpeg)), "jpeg");
        assert_eq!(name(DetectedFormat::Image(ImageFormat::Png)), "png");
        assert_eq!(name(DetectedFormat::Image(ImageFormat::Tiff)), "tiff");
        assert_eq!(name(DetectedFormat::Image(ImageFormat::Bmp)), "bmp");
        assert_eq!(name(DetectedFormat::Image(ImageFormat::WebP)), "webp");
        assert_eq!(name(DetectedFormat::Unknown), "unknown");
    }
}
