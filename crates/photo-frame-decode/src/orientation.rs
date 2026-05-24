//! Apply the EXIF `Orientation` tag to a [`DynamicImage`].
//!
//! EXIF Orientation is a 1..=8 integer that tells viewers how the camera
//! was held. Until the pixel buffer is rotated/mirrored accordingly, every
//! downstream consumer sees the wrong layout. We normalize *here* so the
//! [`Photograph`] handed to the frame crate is always upright.
//!
//! Strategy: delegate to `image::metadata::Orientation` (the variant set
//! exactly matches EXIF 1..=8) and `DynamicImage::apply_orientation`. We
//! deliberately do *not* keep a parallel match table — the workspace pins
//! `image = 0.25`, so the abstraction cost of maintaining our own enum is
//! no longer justified.
//!
//! [`Photograph`]: photo_frame_types::Photograph

use image::{metadata::Orientation, DynamicImage};

/// Apply the EXIF orientation value to `img` in place.
///
/// `raw` is the raw EXIF tag value as kamadak-exif reports it (`u32`). A
/// `None` means the tag was absent; `Some(0)` means the tag was present
/// but explicitly unspecified — both are silent no-ops.
pub(crate) fn apply(img: &mut DynamicImage, raw: Option<u32>) {
    let Some(raw) = raw else {
        return;
    };
    if raw == 0 {
        return;
    }
    let Ok(raw_u8) = u8::try_from(raw) else {
        tracing::warn!(raw, "EXIF Orientation value out of u8 range");
        return;
    };
    let Some(orientation) = Orientation::from_exif(raw_u8) else {
        tracing::warn!(raw, "unknown EXIF Orientation; treating as identity");
        return;
    };
    tracing::debug!(raw, applied = ?orientation, "orientation applied");
    img.apply_orientation(orientation);
}

#[cfg(test)]
mod tests {
    use super::apply;
    use image::{DynamicImage, Rgba, RgbaImage};
    use tracing_test::traced_test;

    /// 4×2 image where each pixel encodes its (x, y) so rotations are easy
    /// to verify by sampling a single corner.
    fn coded() -> DynamicImage {
        let mut img = RgbaImage::new(4, 2);
        for y in 0..2u32 {
            for x in 0..4u32 {
                let r = u8::try_from(x).unwrap();
                let g = u8::try_from(y).unwrap();
                img.put_pixel(x, y, Rgba([r, g, 0xAA, 0xFF]));
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn none_raw_is_identity() {
        let original = coded();
        let mut img = original.clone();
        apply(&mut img, None);
        assert_eq!(img, original);
    }

    #[test]
    fn raw_zero_is_silent_identity() {
        let original = coded();
        let mut img = original.clone();
        apply(&mut img, Some(0));
        assert_eq!(img, original);
    }

    #[test]
    fn raw_1_is_identity() {
        let original = coded();
        let mut img = original.clone();
        apply(&mut img, Some(1));
        assert_eq!(img, original);
    }

    #[test]
    fn raw_2_flips_horizontally() {
        let mut img = coded();
        apply(&mut img, Some(2));
        let buf = img.to_rgba8();
        // x=0 in the output came from x=3 in the source
        assert_eq!(buf.get_pixel(0, 0), &Rgba([3, 0, 0xAA, 0xFF]));
        assert_eq!(buf.get_pixel(3, 0), &Rgba([0, 0, 0xAA, 0xFF]));
    }

    #[test]
    fn raw_3_rotates_180() {
        let mut img = coded();
        apply(&mut img, Some(3));
        let buf = img.to_rgba8();
        assert_eq!(buf.get_pixel(0, 0), &Rgba([3, 1, 0xAA, 0xFF]));
        assert_eq!(buf.get_pixel(3, 1), &Rgba([0, 0, 0xAA, 0xFF]));
    }

    #[test]
    fn raw_6_rotates_to_portrait() {
        // 4x2 (landscape) → 2x4 (portrait), 90° CW
        let mut img = coded();
        apply(&mut img, Some(6));
        assert_eq!(img.width(), 2);
        assert_eq!(img.height(), 4);
    }

    #[test]
    fn raw_8_rotates_to_portrait_other_way() {
        // 4x2 (landscape) → 2x4 (portrait), 90° CCW
        let mut img = coded();
        apply(&mut img, Some(8));
        assert_eq!(img.width(), 2);
        assert_eq!(img.height(), 4);
    }

    #[test]
    #[traced_test]
    fn unknown_raw_emits_warn_and_is_identity() {
        let original = coded();
        let mut img = original.clone();
        apply(&mut img, Some(99));
        assert_eq!(img, original);
        assert!(logs_contain("unknown EXIF Orientation"));
    }

    #[test]
    #[traced_test]
    fn raw_overflow_u8_emits_warn_and_is_identity() {
        let original = coded();
        let mut img = original.clone();
        apply(&mut img, Some(99_999));
        assert_eq!(img, original);
        assert!(logs_contain("out of u8 range"));
    }

    #[test]
    #[traced_test]
    fn raw_zero_does_not_warn() {
        let mut img = coded();
        apply(&mut img, Some(0));
        assert!(!logs_contain("EXIF Orientation"));
    }
}
