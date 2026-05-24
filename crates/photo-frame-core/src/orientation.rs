//! EXIF Orientation normalization.
//!
//! The contract is: callers hand us a freshly decoded image plus the raw
//! Orientation tag (1..=8) read from the source EXIF, and we return a
//! pixel-upright copy. Any subsequent stage of the pipeline can assume the
//! image is already correctly oriented and that no Orientation tag needs
//! re-emitting on encode.

use image::DynamicImage;
use tracing::warn;

/// EXIF Orientation tag value, as defined in EXIF 2.32 §4.6.4.
///
/// Constructed from the raw `u16` read out of EXIF; unknown values fall back to
/// [`Orientation::Normal`] rather than failing, matching what most viewers do.
/// An unknown value emits a `WARN`-level tracing event so operators can spot
/// nonconformant cameras / files.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Orientation {
    Normal,
    FlipH,
    Rotate180,
    FlipV,
    Transpose,
    Rotate90Cw,
    Transverse,
    Rotate270Cw,
}

impl Orientation {
    #[must_use]
    pub(crate) fn from_raw(raw: u16) -> Self {
        let parsed = Self::map_raw(raw);
        // Tag 0 means "unspecified" in some writers — treat as Normal silently.
        // Anything else outside 1..=8 is a genuine spec violation worth surfacing.
        if raw != 0 && !(1..=8).contains(&raw) {
            warn!(
                raw,
                "EXIF Orientation has unknown value; defaulting to Normal"
            );
        }
        parsed
    }

    const fn map_raw(raw: u16) -> Self {
        match raw {
            2 => Self::FlipH,
            3 => Self::Rotate180,
            4 => Self::FlipV,
            5 => Self::Transpose,
            6 => Self::Rotate90Cw,
            7 => Self::Transverse,
            8 => Self::Rotate270Cw,
            _ => Self::Normal,
        }
    }
}

/// Return a pixel-upright copy of `img`, applying the rotation/flip dictated by
/// the EXIF Orientation tag.
#[must_use]
pub(crate) fn normalize(img: DynamicImage, orientation: Orientation) -> DynamicImage {
    match orientation {
        Orientation::Normal => img,
        Orientation::FlipH => img.fliph(),
        Orientation::Rotate180 => img.rotate180(),
        Orientation::FlipV => img.flipv(),
        Orientation::Transpose => img.rotate90().fliph(),
        Orientation::Rotate90Cw => img.rotate90(),
        Orientation::Transverse => img.rotate270().fliph(),
        Orientation::Rotate270Cw => img.rotate270(),
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize, Orientation};
    use image::{DynamicImage, Rgba, RgbaImage};

    /// Build a 4×2 image where every pixel is identifiable so we can verify
    /// rotations and flips by inspecting corner pixels after transformation.
    ///
    /// ```text
    ///   x=0  x=1  x=2  x=3
    /// y=0 (0,0) (1,0) (2,0) (3,0)
    /// y=1 (0,1) (1,1) (2,1) (3,1)
    /// ```
    fn coded() -> DynamicImage {
        let mut img = RgbaImage::new(4, 2);
        for (x, y, p) in img.enumerate_pixels_mut() {
            let r = u8::try_from(x * 60).expect("4×2 test grid: x*60 fits u8");
            let g = u8::try_from(y * 120).expect("4×2 test grid: y*120 fits u8");
            *p = Rgba([r, g, 0, 255]);
        }
        DynamicImage::ImageRgba8(img)
    }

    fn pixel(img: &DynamicImage, x: u32, y: u32) -> [u8; 4] {
        let rgba = img.to_rgba8();
        rgba.get_pixel(x, y).0
    }

    #[test]
    fn normal_is_identity() {
        let src = coded();
        let out = normalize(src.clone(), Orientation::Normal);
        assert_eq!(pixel(&src, 0, 0), pixel(&out, 0, 0));
        assert_eq!(pixel(&src, 3, 1), pixel(&out, 3, 1));
    }

    #[test]
    fn fliph_mirrors_horizontally() {
        let src = coded();
        let out = normalize(src.clone(), Orientation::FlipH);
        assert_eq!(pixel(&src, 0, 0), pixel(&out, 3, 0));
        assert_eq!(pixel(&src, 3, 1), pixel(&out, 0, 1));
    }

    #[test]
    fn rotate180_swaps_opposite_corners() {
        let src = coded();
        let out = normalize(src.clone(), Orientation::Rotate180);
        assert_eq!(pixel(&src, 0, 0), pixel(&out, 3, 1));
        assert_eq!(pixel(&src, 3, 1), pixel(&out, 0, 0));
    }

    #[test]
    fn flipv_mirrors_vertically() {
        let src = coded();
        let out = normalize(src.clone(), Orientation::FlipV);
        assert_eq!(pixel(&src, 0, 0), pixel(&out, 0, 1));
        assert_eq!(pixel(&src, 3, 1), pixel(&out, 3, 0));
    }

    #[test]
    fn rotate90cw_changes_dimensions_and_corners() {
        let src = coded();
        let out = normalize(src.clone(), Orientation::Rotate90Cw);
        assert_eq!(out.width(), 2);
        assert_eq!(out.height(), 4);
        // (0,0) lands at (1, 0) after a 90° CW rotation of a 4×2 image.
        assert_eq!(pixel(&src, 0, 0), pixel(&out, 1, 0));
        assert_eq!(pixel(&src, 3, 1), pixel(&out, 0, 3));
    }

    #[test]
    fn rotate270cw_changes_dimensions_and_corners() {
        let src = coded();
        let out = normalize(src.clone(), Orientation::Rotate270Cw);
        assert_eq!(out.width(), 2);
        assert_eq!(out.height(), 4);
        assert_eq!(pixel(&src, 0, 0), pixel(&out, 0, 3));
        assert_eq!(pixel(&src, 3, 1), pixel(&out, 1, 0));
    }

    #[test]
    fn transpose_and_transverse_change_dimensions() {
        let src = coded();
        let t = normalize(src.clone(), Orientation::Transpose);
        let tv = normalize(src, Orientation::Transverse);
        assert_eq!((t.width(), t.height()), (2, 4));
        assert_eq!((tv.width(), tv.height()), (2, 4));
    }

    #[test]
    fn from_raw_accepts_all_eight_known_values() {
        for raw in 1_u16..=8 {
            // Each is distinct from the next; we just need to know the conversion
            // doesn't panic and returns a sensible variant.
            let _ = Orientation::from_raw(raw);
        }
    }

    #[test]
    fn from_raw_defaults_unknown_to_normal() {
        assert_eq!(Orientation::from_raw(0), Orientation::Normal);
        assert_eq!(Orientation::from_raw(9), Orientation::Normal);
        assert_eq!(Orientation::from_raw(42), Orientation::Normal);
    }

    #[tracing_test::traced_test]
    #[test]
    fn unknown_orientation_value_emits_warning() {
        let _ = Orientation::from_raw(99);
        assert!(logs_contain("EXIF Orientation has unknown value"));
    }

    #[tracing_test::traced_test]
    #[test]
    fn zero_orientation_value_is_silent() {
        // 0 is "unspecified" — common enough that it would be noisy to warn.
        let _ = Orientation::from_raw(0);
        assert!(!logs_contain("EXIF Orientation has unknown value"));
    }

    #[tracing_test::traced_test]
    #[test]
    fn valid_orientation_value_is_silent() {
        for raw in 1_u16..=8 {
            let _ = Orientation::from_raw(raw);
        }
        assert!(!logs_contain("EXIF Orientation has unknown value"));
    }
}
