//! Typed primitives that pin invariants into the type system.
//!
//! Each newtype wraps a raw value behind a constructor that enforces its
//! invariant once. Downstream code that takes the newtype as a parameter
//! receives that invariant as a static guarantee — there is no second
//! place to re-check, and no way to construct a violating value without
//! going through `new()`.
//!
//! The shape mirrors the standard library's `NonZeroU32`: `new` returns
//! `Option<Self>`, accessors are zero-cost, and the wrapped value is
//! private so the only way in is the validating constructor.

use std::num::NonZeroU32;

/// JPEG encoder quality on the canonical `1..=100` scale.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JpegQuality(u8);

impl JpegQuality {
    /// Minimum valid JPEG quality.
    pub const MIN: u8 = 1;
    /// Maximum valid JPEG quality.
    pub const MAX: u8 = 100;

    /// Construct from a raw quality value. Returns `None` outside `1..=100`.
    #[must_use]
    pub const fn new(value: u8) -> Option<Self> {
        if value >= Self::MIN && value <= Self::MAX {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Unwrap the wrapped quality value.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Longest-edge pixel cap for a downscale pass. Non-zero by construction.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LongEdge(NonZeroU32);

impl LongEdge {
    /// Construct from a raw pixel count. Returns `None` for zero.
    #[must_use]
    pub const fn new(value: u32) -> Option<Self> {
        match NonZeroU32::new(value) {
            Some(n) => Some(Self(n)),
            None => None,
        }
    }

    /// Unwrap the pixel count.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

/// A free-form EXIF string, guaranteed non-empty after trimming whitespace.
///
/// Wraps the raw `String` so call sites can rely on "this is a name a
/// renderer can show" without re-checking for empty / whitespace-only
/// inputs scattered across the pipeline.
///
/// `Deref<Target = str>` lets call sites treat `ExifString` the same
/// way they would treat `String` — `Option<ExifString>::as_deref()`
/// returns `Option<&str>`, methods like `.split('/')` and `.starts_with`
/// reach through, and the `&str` borrow flows into any
/// `impl AsRef<str>` API.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExifString(String);

impl ExifString {
    /// Construct from a raw EXIF string. Returns `None` when the value
    /// is empty or whitespace-only — those are not displayable names.
    #[must_use]
    pub fn new(value: String) -> Option<Self> {
        if value.trim().is_empty() {
            None
        } else {
            Some(Self(value))
        }
    }

    /// Borrowed view of the underlying string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume and return the underlying `String`.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::ops::Deref for ExifString {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for ExifString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// An aperture f-number — a positive, finite lens setting (e.g. `1.8`, `4.0`).
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Fnumber(f64);

impl Fnumber {
    /// Construct from a raw f-number. Returns `None` for non-positive
    /// values or non-finite (`NaN`, `±∞`) inputs.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        positive_finite(value).map(Self)
    }

    /// Unwrap the f-number as a raw `f64`.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// Lens focal length in millimetres — positive, finite. Stored as
/// the *actual* (un-cropped) focal length the camera reported.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct FocalLengthMm(f64);

impl FocalLengthMm {
    /// Construct from a raw millimetre value. Returns `None` for
    /// non-positive values or non-finite (`NaN`, `±∞`) inputs.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        positive_finite(value).map(Self)
    }

    /// Unwrap the focal length as a raw `f64` in millimetres.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// Exposure time in seconds — positive, finite.
///
/// Spans the practical camera range (~`1.0 / 8000.0` for fast
/// electronic shutters up to minutes for bulb exposures); the newtype
/// only refuses obviously nonsensical zeros, negatives, and NaN/∞.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct ShutterSeconds(f64);

impl ShutterSeconds {
    /// Construct from a raw seconds value. Returns `None` for
    /// non-positive values or non-finite (`NaN`, `±∞`) inputs.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        positive_finite(value).map(Self)
    }

    /// Unwrap the exposure time as a raw `f64` in seconds.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// Shared invariant the three physical f64 newtypes (`Fnumber`,
/// `FocalLengthMm`, `ShutterSeconds`) all enforce.
fn positive_finite(value: f64) -> Option<f64> {
    if value.is_finite() && value > 0.0 {
        Some(value)
    } else {
        None
    }
}

/// ISO sensitivity reading (non-zero).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IsoSensitivity(NonZeroU32);

impl IsoSensitivity {
    /// Construct from a raw ISO reading. Returns `None` for zero.
    #[must_use]
    pub const fn new(value: u32) -> Option<Self> {
        match NonZeroU32::new(value) {
            Some(n) => Some(Self(n)),
            None => None,
        }
    }

    /// Unwrap the ISO reading.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

/// Image dimensions in pixels with both axes guaranteed non-zero.
///
/// A `Dimensions` value is the canonical "size of a raster grid"
/// vocabulary across the workspace — anywhere a `(width, height)` pair
/// needs to carry the "both axes positive" invariant, the newtype takes
/// over from the bare tuple.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Dimensions {
    width: NonZeroU32,
    height: NonZeroU32,
}

impl Dimensions {
    /// Construct from a `(width, height)` pair. Returns `None` when
    /// either axis is zero — those grids are meaningless downstream.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Option<Self> {
        match (NonZeroU32::new(width), NonZeroU32::new(height)) {
            (Some(w), Some(h)) => Some(Self {
                width: w,
                height: h,
            }),
            _ => None,
        }
    }

    /// Width in pixels.
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width.get()
    }

    /// Height in pixels.
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height.get()
    }

    /// `width * height * 4` — the canonical byte length of an RGBA8
    /// buffer that exactly fills this grid.
    #[must_use]
    pub const fn rgba8_bytes(self) -> usize {
        (self.width.get() as usize) * (self.height.get() as usize) * 4
    }
}

/// An 8-bit RGBA colour, with the same packed `[u8; 4]` byte order
/// (`[red, green, blue, alpha]`) used by every renderer downstream.
///
/// Kept as a value type — there is no construction-time invariant beyond
/// "four bytes" — so it stays a thin vocabulary item that other crates
/// can convert into their own colour types (`image::Rgba`, CSS strings,
/// canvas paint settings) without dragging `image` into `photo-frame-types`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Rgba8 {
    /// Red channel, 0..=255.
    pub r: u8,
    /// Green channel, 0..=255.
    pub g: u8,
    /// Blue channel, 0..=255.
    pub b: u8,
    /// Alpha channel, 0..=255 (`255` = fully opaque).
    pub a: u8,
}

impl Rgba8 {
    /// Construct from explicit channel values.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Pure-opaque white (`[255, 255, 255, 255]`).
    pub const WHITE: Self = Self::new(255, 255, 255, 255);
    /// Pure-opaque black (`[0, 0, 0, 255]`).
    pub const BLACK: Self = Self::new(0, 0, 0, 255);

    /// Return the colour as a `[u8; 4]` in RGBA byte order — convenient
    /// for the boundary with `image::Rgba` and similar APIs.
    #[must_use]
    pub const fn to_array(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Dimensions, ExifString, Fnumber, FocalLengthMm, IsoSensitivity, JpegQuality, LongEdge,
        Rgba8, ShutterSeconds,
    };

    #[test]
    fn jpeg_quality_accepts_canonical_range() {
        assert_eq!(JpegQuality::new(1).map(JpegQuality::get), Some(1));
        assert_eq!(JpegQuality::new(50).map(JpegQuality::get), Some(50));
        assert_eq!(JpegQuality::new(100).map(JpegQuality::get), Some(100));
    }

    #[test]
    fn jpeg_quality_rejects_out_of_range() {
        assert!(JpegQuality::new(0).is_none());
        assert!(JpegQuality::new(101).is_none());
        assert!(JpegQuality::new(255).is_none());
    }

    #[test]
    fn long_edge_rejects_zero() {
        assert!(LongEdge::new(0).is_none());
        assert_eq!(LongEdge::new(2048).map(LongEdge::get), Some(2048));
    }

    #[test]
    fn exif_string_rejects_empty_and_whitespace() {
        assert!(ExifString::new(String::new()).is_none());
        assert!(ExifString::new("   ".into()).is_none());
        assert_eq!(
            ExifString::new("NIKON Z 5".into())
                .as_ref()
                .map(ExifString::as_str),
            Some("NIKON Z 5"),
        );
    }

    #[test]
    fn exif_string_keeps_internal_whitespace_verbatim() {
        // Display formatting is the renderer's job; the primitive
        // never massages the inside of a non-empty string.
        let raw = "  NIKON   Z 5  ".to_owned();
        let s = ExifString::new(raw.clone()).expect("non-empty after trim");
        assert_eq!(s.as_str(), raw);
    }

    #[test]
    fn fnumber_rejects_non_positive_and_nan() {
        assert!(Fnumber::new(0.0).is_none());
        assert!(Fnumber::new(-1.4).is_none());
        assert!(Fnumber::new(f64::NAN).is_none());
        assert!(Fnumber::new(f64::INFINITY).is_none());
        assert_eq!(Fnumber::new(1.8).map(Fnumber::get), Some(1.8));
    }

    #[test]
    fn focal_length_rejects_non_positive_and_nan() {
        assert!(FocalLengthMm::new(0.0).is_none());
        assert!(FocalLengthMm::new(-50.0).is_none());
        assert!(FocalLengthMm::new(f64::NAN).is_none());
        assert!(FocalLengthMm::new(f64::NEG_INFINITY).is_none());
        assert_eq!(FocalLengthMm::new(50.0).map(FocalLengthMm::get), Some(50.0));
    }

    #[test]
    fn shutter_seconds_rejects_non_positive_and_nan() {
        assert!(ShutterSeconds::new(0.0).is_none());
        assert!(ShutterSeconds::new(-0.5).is_none());
        assert!(ShutterSeconds::new(f64::NAN).is_none());
        assert!(ShutterSeconds::new(f64::INFINITY).is_none());
        assert_eq!(
            ShutterSeconds::new(1.0 / 250.0).map(ShutterSeconds::get),
            Some(1.0 / 250.0),
        );
    }

    #[test]
    fn iso_sensitivity_rejects_zero() {
        assert!(IsoSensitivity::new(0).is_none());
        assert_eq!(
            IsoSensitivity::new(6400).map(IsoSensitivity::get),
            Some(6400),
        );
    }

    #[test]
    fn dimensions_rejects_zero_axis() {
        assert!(Dimensions::new(0, 100).is_none());
        assert!(Dimensions::new(100, 0).is_none());
        assert!(Dimensions::new(0, 0).is_none());
        let d = Dimensions::new(1920, 1080).expect("both axes non-zero");
        assert_eq!(d.width(), 1920);
        assert_eq!(d.height(), 1080);
    }

    #[test]
    fn dimensions_rgba8_byte_count_matches_geometry() {
        let d = Dimensions::new(4, 3).expect("non-zero");
        assert_eq!(d.rgba8_bytes(), 4 * 3 * 4);
    }

    #[test]
    fn rgba8_to_array_matches_construction() {
        let c = Rgba8::new(10, 20, 30, 40);
        assert_eq!(c.to_array(), [10, 20, 30, 40]);
    }

    #[test]
    fn rgba8_predefined_palette_matches_pure_values() {
        assert_eq!(Rgba8::WHITE.to_array(), [255, 255, 255, 255]);
        assert_eq!(Rgba8::BLACK.to_array(), [0, 0, 0, 255]);
    }
}
