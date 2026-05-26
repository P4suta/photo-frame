use crate::primitives::{ExifString, Fnumber, FocalLengthMm, IsoSensitivity, ShutterSeconds};

/// "Where did this photograph come from?" — capture-time metadata as a
/// structured, primitive-typed record.
///
/// Every field is `Option` because not every source supplies every fact:
/// a PNG from a screenshot has no `Camera`; a HEIC from an old iPhone may
/// have `Camera` but no `Lens` model. Renderers downstream are expected
/// to gracefully drop missing facts from the caption.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Provenance {
    /// Camera body that produced the image (make + model, both
    /// individually optional).
    pub camera: Option<Camera>,
    /// Lens mounted on the camera (make + model, both individually
    /// optional).
    pub lens: Option<Lens>,
    /// Exposure values reported by the camera (focal length, aperture,
    /// shutter speed, ISO).
    pub exposure: Option<Exposure>,
    /// Capture timestamp as the camera recorded it, in the camera's
    /// local time (not normalised to UTC — see [`DateTime`]).
    pub captured_at: Option<DateTime>,
}

impl Provenance {
    /// `true` when no caller-visible facts are present. Useful for the
    /// `MetaPolicy::Auto` collapse path in the frame crate.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.camera.is_none()
            && self.lens.is_none()
            && self.exposure.is_none()
            && self.captured_at.is_none()
    }
}

/// Camera body, as a free-form make / model pair (both optional).
///
/// EXIF's `Make` and `Model` tags are strings with no canonical format
/// (vendor cleanup like `"NIKON CORPORATION"` → `"NIKON"` is a
/// rendering concern, not a data concern). They ride as
/// [`ExifString`] so the "non-empty after trimming" invariant is
/// pinned in the type system rather than re-checked at every caption
/// formatter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Camera {
    /// Manufacturer (EXIF `Make`), e.g. `"NIKON CORPORATION"`.
    pub make: Option<ExifString>,
    /// Model designation (EXIF `Model`), e.g. `"NIKON Z 5"`.
    pub model: Option<ExifString>,
}

/// Lens, as a free-form make / model pair (both optional).
///
/// Same shape as [`Camera`] for the same reasons — EXIF lens
/// information is unstructured strings, so [`ExifString`] pins the
/// "non-empty after trimming" invariant once at the boundary.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lens {
    /// Lens manufacturer (EXIF `LensMake`), e.g. `"NIKON"`.
    pub make: Option<ExifString>,
    /// Lens model (EXIF `LensModel`), e.g. `"NIKKOR Z 50mm f/1.8 S"`.
    pub model: Option<ExifString>,
}

/// Exposure facts in their canonical *physical* form, each wrapped in
/// a typed primitive that pins the relevant invariant:
/// - `focal_length_mm`: actual lens focal length, [`FocalLengthMm`]
///   (positive, finite).
/// - `aperture`: f-number, [`Fnumber`] (positive, finite).
/// - `shutter_seconds`: exposure time in seconds, [`ShutterSeconds`]
///   (positive, finite).
/// - `iso`: ISO sensitivity reading, [`IsoSensitivity`] (non-zero).
///
/// Renderers format these for display; the data itself stays as
/// already-validated primitives so non-display consumers (JSON
/// serializers, command-line dumpers, statistical analyses) can use
/// them directly without re-running each fact's "is this a sane value"
/// check.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Exposure {
    /// Lens focal length in millimetres, as the camera reported it (no
    /// crop-factor correction applied).
    pub focal_length_mm: Option<FocalLengthMm>,
    /// Aperture as an f-number (e.g. `1.8`, `4.0`).
    pub aperture: Option<Fnumber>,
    /// Shutter time in seconds (e.g. `1.0 / 250.0`, `2.0`).
    pub shutter_seconds: Option<ShutterSeconds>,
    /// ISO sensitivity reading (e.g. `200`, `6400`).
    pub iso: Option<IsoSensitivity>,
}

/// Capture moment, broken into calendar components.
///
/// We deliberately do *not* normalize to UTC — EXIF dates rarely carry
/// timezone information and presenting "as captured" matches what a
/// photographer expects to see in the caption.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct DateTime {
    /// Calendar year (e.g. `2026`).
    pub year: u16,
    /// Calendar month, `1..=12`.
    pub month: u8,
    /// Calendar day of month, `1..=31`.
    pub day: u8,
    /// Hour of day, `0..=23`.
    pub hour: u8,
    /// Minute of hour, `0..=59`.
    pub minute: u8,
    /// Second of minute, `0..=59` (no leap-second representation — EXIF
    /// doesn't carry one either).
    pub second: u8,
}

#[cfg(test)]
mod tests {
    use super::{Camera, DateTime, Exposure, Lens, Provenance};
    use crate::primitives::{ExifString, Fnumber, FocalLengthMm, IsoSensitivity, ShutterSeconds};

    fn exif(s: &str) -> ExifString {
        ExifString::new(s.to_owned()).expect("non-empty after trim")
    }

    #[test]
    fn default_is_empty() {
        assert!(Provenance::default().is_empty());
    }

    #[test]
    fn camera_alone_is_not_empty() {
        let p = Provenance {
            camera: Some(Camera {
                make: Some(exif("NIKON")),
                model: Some(exif("Z 5")),
            }),
            ..Default::default()
        };
        assert!(!p.is_empty());
    }

    #[test]
    fn types_construct_via_field_init() {
        // Compile-time check that the public field shape matches what
        // downstream crates expect to be able to write.
        let _ = Lens {
            make: None,
            model: Some(exif("NIKKOR Z 50mm")),
        };
        let _ = Exposure {
            focal_length_mm: FocalLengthMm::new(50.0),
            aperture: Fnumber::new(1.8),
            shutter_seconds: ShutterSeconds::new(1.0 / 250.0),
            iso: IsoSensitivity::new(200),
        };
        let _ = DateTime {
            year: 2026,
            month: 5,
            day: 24,
            hour: 10,
            minute: 0,
            second: 0,
        };
    }
}
