/// "Where did this photograph come from?" — capture-time metadata as a
/// structured, primitive-typed record.
///
/// Every field is `Option` because not every source supplies every fact:
/// a PNG from a screenshot has no `Camera`; a HEIC from an old iPhone may
/// have `Camera` but no `Lens` model. Renderers downstream are expected
/// to gracefully drop missing facts from the caption.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Provenance {
    pub camera: Option<Camera>,
    pub lens: Option<Lens>,
    pub exposure: Option<Exposure>,
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Camera {
    pub make: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lens {
    pub make: Option<String>,
    pub model: Option<String>,
}

/// Exposure facts in their canonical *physical* form:
/// - `focal_length_mm`: the actual lens focal length in millimetres.
/// - `aperture`: the f-number (e.g. `1.8`, `4.0`).
/// - `shutter_seconds`: exposure time in seconds (`1.0 / 250.0`, `2.0`).
/// - `iso`: the ISO sensitivity reading the camera reports.
///
/// Renderers format these for display; the data itself stays as numbers
/// so non-display consumers (JSON serializers, command-line dumpers,
/// statistical analyses) can use them directly.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Exposure {
    pub focal_length_mm: Option<f64>,
    pub aperture: Option<f64>,
    pub shutter_seconds: Option<f64>,
    pub iso: Option<u32>,
}

/// Capture moment, broken into calendar components.
///
/// We deliberately do *not* normalize to UTC — EXIF dates rarely carry
/// timezone information and presenting "as captured" matches what a
/// photographer expects to see in the caption.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

#[cfg(test)]
mod tests {
    use super::{Camera, DateTime, Exposure, Lens, Provenance};

    #[test]
    fn default_is_empty() {
        assert!(Provenance::default().is_empty());
    }

    #[test]
    fn camera_alone_is_not_empty() {
        let p = Provenance {
            camera: Some(Camera {
                make: Some("NIKON".into()),
                model: Some("Z 5".into()),
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
            model: Some("NIKKOR Z 50mm".into()),
        };
        let _ = Exposure {
            focal_length_mm: Some(50.0),
            aperture: Some(1.8),
            shutter_seconds: Some(1.0 / 250.0),
            iso: Some(200),
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
