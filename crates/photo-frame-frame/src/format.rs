//! Format Provenance primitives into caption-ready display strings.
//!
//! The decoder hands us [`Provenance`] in *physical* units — `f64`
//! focal lengths, `Option<u32>` ISO, calendar components in [`DateTime`].
//! Caption rendering is a presentation concern: the difference between
//! `"50mm"`, `"50 mm"`, or `"105 mm"` is a *typographic* choice, not a
//! data shape. Living downstream of the data carrier means this module
//! can change format without touching either the decoder or the
//! intermediate.
//!
//! [`Provenance`]: photo_frame_types::Provenance
//! [`DateTime`]: photo_frame_types::DateTime

use photo_frame_types::{Camera, DateTime, Exposure, Lens, Provenance};

/// The two caption rows the renderer draws. Each side is `Option` so the
/// renderer can suppress a single corner without re-centering the row.
#[derive(Default, Debug, Clone)]
pub(crate) struct Caption {
    pub(crate) top_left: Option<String>,     // camera body
    pub(crate) top_right: Option<String>,    // lens
    pub(crate) bottom_left: Option<String>,  // exposure facts
    pub(crate) bottom_right: Option<String>, // capture date
}

impl Caption {
    /// `true` when there isn't a single facet to render.
    pub(crate) const fn is_empty(&self) -> bool {
        self.top_left.is_none()
            && self.top_right.is_none()
            && self.bottom_left.is_none()
            && self.bottom_right.is_none()
    }

    /// Top row collapsed into a single string for centred layouts —
    /// `"<camera> · <lens>"`, omitting either side when absent.
    pub(crate) fn top_combined(&self) -> Option<String> {
        join_with_separator(self.top_left.as_deref(), self.top_right.as_deref())
    }

    /// Bottom row collapsed for centred layouts — `"<exposure> · <date>"`.
    pub(crate) fn bottom_combined(&self) -> Option<String> {
        join_with_separator(self.bottom_left.as_deref(), self.bottom_right.as_deref())
    }
}

/// Join two optional facets with the canonical `"  ·  "` separator
/// (same one used inside the exposure line, so the centre layout
/// reads as visually congruent with the edge layout). When only one
/// side is present, returns it verbatim — never adds a dangling
/// separator.
fn join_with_separator(left: Option<&str>, right: Option<&str>) -> Option<String> {
    match (left, right) {
        (Some(l), Some(r)) => Some(format!("{l}  ·  {r}")),
        (Some(s), None) | (None, Some(s)) => Some(s.to_owned()),
        (None, None) => None,
    }
}

pub(crate) fn caption_from(provenance: &Provenance) -> Caption {
    Caption {
        top_left: provenance.camera.as_ref().and_then(camera_label),
        top_right: provenance.lens.as_ref().and_then(lens_label),
        bottom_left: provenance.exposure.as_ref().and_then(exposure_line),
        bottom_right: provenance.captured_at.map(format_date),
    }
}

// ── camera / lens ────────────────────────────────────────────────────────

fn camera_label(camera: &Camera) -> Option<String> {
    let model = camera.model.as_deref().map(strip_corporation);
    if let Some(m) = model.filter(|s| !s.is_empty()) {
        return Some(m);
    }
    // Model missing — fall back to bare make.
    camera
        .make
        .as_deref()
        .map(strip_corporation)
        .filter(|s| !s.is_empty())
}

fn lens_label(lens: &Lens) -> Option<String> {
    lens.model
        .as_deref()
        .map(strip_corporation)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            lens.make
                .as_deref()
                .map(strip_corporation)
                .filter(|s| !s.is_empty())
        })
}

/// Nikon writes `Make = "NIKON CORPORATION"` and `Model = "NIKON Z 8"` —
/// the legalese suffix is signal-free noise in a caption.
fn strip_corporation(model: &str) -> String {
    let trimmed = model.trim_end_matches(char::is_whitespace);
    trimmed
        .strip_suffix("CORPORATION")
        .map_or_else(|| trimmed.to_owned(), |s| s.trim_end().to_owned())
}

// ── exposure line ────────────────────────────────────────────────────────

fn exposure_line(exposure: &Exposure) -> Option<String> {
    let parts: Vec<String> = [
        exposure.focal_length_mm.and_then(format_focal_length),
        exposure.aperture.and_then(|f| format_aperture(f.get())),
        exposure.shutter_seconds.and_then(format_seconds),
        exposure.iso.map(|v| format!("ISO {iso}", iso = v.get())),
    ]
    .into_iter()
    .flatten()
    .collect();
    (!parts.is_empty()).then(|| parts.join("  ·  "))
}

fn format_focal_length(mm: f64) -> Option<String> {
    (mm.is_finite() && mm > 0.0).then(|| format!("{mm:.0}mm"))
}

fn format_aperture(f: f64) -> Option<String> {
    if !f.is_finite() || f <= 0.0 {
        return None;
    }
    if (f - f.round()).abs() < 0.05 {
        Some(format!("f/{f:.0}"))
    } else {
        Some(format!("f/{f:.1}"))
    }
}

fn format_seconds(secs: f64) -> Option<String> {
    if !secs.is_finite() || secs <= 0.0 {
        return None;
    }
    if secs < 1.0 {
        Some(format!("1/{:.0}s", 1.0 / secs))
    } else if (secs - secs.round()).abs() < 0.05 {
        Some(format!("{secs:.0}s"))
    } else {
        Some(format!("{secs:.1}s"))
    }
}

// ── date ────────────────────────────────────────────────────────────────

fn format_date(dt: DateTime) -> String {
    format!("{:04}-{:02}-{:02}", dt.year, dt.month, dt.day)
}

#[cfg(test)]
mod tests {
    use super::{
        caption_from, format_aperture, format_date, format_focal_length, format_seconds,
        strip_corporation,
    };
    use photo_frame_types::{
        Camera, DateTime, ExifString, Exposure, Fnumber, IsoSensitivity, Lens, Provenance,
    };

    fn exif(s: &str) -> ExifString {
        ExifString::new(s.to_owned()).expect("non-empty fixture string")
    }

    fn fstop(v: f64) -> Fnumber {
        Fnumber::new(v).expect("positive finite f-number fixture")
    }

    fn iso(v: u32) -> IsoSensitivity {
        IsoSensitivity::new(v).expect("non-zero ISO fixture")
    }

    #[test]
    fn focal_length_rounds_to_integer_mm() {
        assert_eq!(format_focal_length(50.0).as_deref(), Some("50mm"));
        assert_eq!(format_focal_length(105.0).as_deref(), Some("105mm"));
    }

    #[test]
    fn focal_length_rejects_zero_and_nonfinite() {
        assert!(format_focal_length(0.0).is_none());
        assert!(format_focal_length(-1.0).is_none());
        assert!(format_focal_length(f64::INFINITY).is_none());
        assert!(format_focal_length(f64::NAN).is_none());
    }

    #[test]
    fn aperture_uses_one_decimal_when_needed() {
        assert_eq!(format_aperture(1.8).as_deref(), Some("f/1.8"));
        assert_eq!(format_aperture(5.6).as_deref(), Some("f/5.6"));
        assert_eq!(format_aperture(2.0).as_deref(), Some("f/2"));
    }

    #[test]
    fn aperture_rejects_invalid() {
        assert!(format_aperture(0.0).is_none());
        assert!(format_aperture(f64::NAN).is_none());
    }

    #[test]
    fn seconds_fast_renders_as_fraction() {
        assert_eq!(format_seconds(1.0 / 250.0).as_deref(), Some("1/250s"));
        assert_eq!(format_seconds(0.001).as_deref(), Some("1/1000s"));
    }

    #[test]
    fn seconds_slow_renders_as_seconds() {
        assert_eq!(format_seconds(2.0).as_deref(), Some("2s"));
        assert_eq!(format_seconds(2.5).as_deref(), Some("2.5s"));
        assert_eq!(format_seconds(30.0).as_deref(), Some("30s"));
    }

    #[test]
    fn seconds_handles_degenerate_input() {
        assert!(format_seconds(0.0).is_none());
        assert!(format_seconds(-1.0).is_none());
        assert!(format_seconds(f64::INFINITY).is_none());
        assert!(format_seconds(f64::NAN).is_none());
    }

    #[test]
    fn date_uses_iso_dashes() {
        let dt = DateTime {
            year: 2026,
            month: 5,
            day: 24,
            ..Default::default()
        };
        assert_eq!(format_date(dt), "2026-05-24");
    }

    #[test]
    fn date_zero_pads_single_digit_components() {
        let dt = DateTime {
            year: 2026,
            month: 3,
            day: 7,
            ..Default::default()
        };
        assert_eq!(format_date(dt), "2026-03-07");
    }

    #[test]
    fn strip_corporation_handles_nikon_form() {
        assert_eq!(strip_corporation("NIKON CORPORATION"), "NIKON");
        assert_eq!(strip_corporation("NIKON Z 8"), "NIKON Z 8");
        assert_eq!(strip_corporation("SONY"), "SONY");
    }

    #[test]
    fn caption_combines_all_facets() {
        let prov = Provenance {
            camera: Some(Camera {
                make: Some(exif("NIKON CORPORATION")),
                model: Some(exif("NIKON Z 5")),
            }),
            lens: Some(Lens {
                make: None,
                model: Some(exif("NIKKOR Z 50mm f/1.8 S")),
            }),
            exposure: Some(Exposure {
                focal_length_mm: Some(50.0),
                aperture: Some(fstop(1.8)),
                shutter_seconds: Some(1.0 / 250.0),
                iso: Some(iso(200)),
            }),
            captured_at: Some(DateTime {
                year: 2026,
                month: 5,
                day: 24,
                ..Default::default()
            }),
        };
        let c = caption_from(&prov);
        assert_eq!(c.top_left.as_deref(), Some("NIKON Z 5"));
        assert_eq!(c.top_right.as_deref(), Some("NIKKOR Z 50mm f/1.8 S"));
        assert_eq!(
            c.bottom_left.as_deref(),
            Some("50mm  ·  f/1.8  ·  1/250s  ·  ISO 200")
        );
        assert_eq!(c.bottom_right.as_deref(), Some("2026-05-24"));
        assert!(!c.is_empty());
    }

    #[test]
    fn caption_with_partial_exposure_omits_missing_facts() {
        let prov = Provenance {
            exposure: Some(Exposure {
                aperture: Some(fstop(2.8)),
                iso: Some(iso(400)),
                ..Default::default()
            }),
            ..Default::default()
        };
        let c = caption_from(&prov);
        assert_eq!(c.bottom_left.as_deref(), Some("f/2.8  ·  ISO 400"));
    }

    #[test]
    fn caption_from_empty_provenance_is_empty() {
        let c = caption_from(&Provenance::default());
        assert!(c.is_empty());
    }

    #[test]
    fn caption_top_combined_joins_camera_and_lens() {
        let c = caption_from(&Provenance {
            camera: Some(Camera {
                make: None,
                model: Some(exif("NIKON Z 5")),
            }),
            lens: Some(Lens {
                make: None,
                model: Some(exif("NIKKOR Z 50mm f/1.8 S")),
            }),
            ..Default::default()
        });
        assert_eq!(
            c.top_combined().as_deref(),
            Some("NIKON Z 5  ·  NIKKOR Z 50mm f/1.8 S"),
        );
    }

    #[test]
    fn caption_combined_drops_missing_side_without_separator() {
        let c = caption_from(&Provenance {
            camera: Some(Camera {
                make: None,
                model: Some(exif("NIKON Z 5")),
            }),
            ..Default::default()
        });
        assert_eq!(c.top_combined().as_deref(), Some("NIKON Z 5"));
        assert!(c.bottom_combined().is_none());
    }

    #[test]
    fn camera_label_falls_back_to_make_when_model_missing() {
        let prov = Provenance {
            camera: Some(Camera {
                make: Some(exif("SONY")),
                model: None,
            }),
            ..Default::default()
        };
        let c = caption_from(&prov);
        assert_eq!(c.top_left.as_deref(), Some("SONY"));
    }
}
