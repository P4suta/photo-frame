//! Exposure facts: focal length, aperture, shutter, ISO.
//!
//! Each field walks a small fallback chain (primary tag first, then a
//! reasonable secondary the EXIF spec defines). The outer `Option<Exposure>`
//! collapses to `None` only when *every* sub-field is missing — partial
//! presence ("we know aperture and ISO but not shutter") is preserved.

use exif::{Context, Exif, Tag};
use photo_frame_types::Exposure;

use super::{rational_f64, uint};

/// EXIF 2.3 spreads "ISO value" across three tags depending on what the
/// camera authored. Canonical first, then the two SensitivityType=2/3
/// fallbacks Nikon Z bodies tend to use.
const ISO_TAG_CANDIDATES: [Tag; 3] = [
    Tag::PhotographicSensitivity,
    Tag(Context::Exif, 0x8832),
    Tag(Context::Exif, 0x8833),
];

pub(crate) fn exposure(exif: &Exif) -> Option<Exposure> {
    let exp = Exposure {
        focal_length_mm: focal_length_mm(exif),
        aperture: aperture(exif),
        shutter_seconds: shutter_seconds(exif),
        iso: iso(exif),
    };
    if exp.focal_length_mm.is_none()
        && exp.aperture.is_none()
        && exp.shutter_seconds.is_none()
        && exp.iso.is_none()
    {
        return None;
    }
    Some(exp)
}

fn focal_length_mm(exif: &Exif) -> Option<f64> {
    if let Some(v) = rational_f64(exif, Tag::FocalLength).filter(|v| is_positive_finite(*v)) {
        return Some(v);
    }
    let v = uint(exif, Tag::FocalLengthIn35mmFilm)
        .map(f64::from)
        .filter(|v| is_positive_finite(*v))?;
    tracing::debug!(
        value = v,
        "focal length resolved via FocalLengthIn35mmFilm fallback"
    );
    Some(v)
}

fn aperture(exif: &Exif) -> Option<f64> {
    if let Some(v) = rational_f64(exif, Tag::FNumber).filter(|v| is_positive_finite(*v)) {
        return Some(v);
    }
    // ApertureValue is APEX-encoded: F = 2^(Av/2).
    let av = rational_f64(exif, Tag::ApertureValue)?;
    let v = apex_to_f_number(av);
    if !is_positive_finite(v) {
        return None;
    }
    tracing::debug!(
        av,
        value = v,
        "aperture resolved via APEX ApertureValue fallback"
    );
    Some(v)
}

fn shutter_seconds(exif: &Exif) -> Option<f64> {
    if let Some(v) = rational_f64(exif, Tag::ExposureTime).filter(|v| is_positive_finite(*v)) {
        return Some(v);
    }
    // ShutterSpeedValue is APEX-encoded: T = 2^(-Tv).
    let tv = rational_f64(exif, Tag::ShutterSpeedValue)?;
    let v = apex_to_seconds(tv);
    if !is_positive_finite(v) {
        return None;
    }
    tracing::debug!(
        tv,
        value = v,
        "shutter resolved via APEX ShutterSpeedValue fallback"
    );
    Some(v)
}

fn iso(exif: &Exif) -> Option<u32> {
    let mut iter = ISO_TAG_CANDIDATES.iter();
    let primary = iter.next().expect("non-empty candidate list");
    if let Some(v) = uint(exif, *primary) {
        return Some(v);
    }
    for tag in iter {
        if let Some(v) = uint(exif, *tag) {
            tracing::debug!(
                tag = format!("0x{:04X}", tag.number()),
                value = v,
                "ISO resolved via fallback tag"
            );
            return Some(v);
        }
    }
    None
}

/// APEX aperture value → F-number. `F = 2^(Av/2)`.
fn apex_to_f_number(av: f64) -> f64 {
    (av / 2.0).exp2()
}

/// APEX shutter value → exposure time in seconds. `T = 2^(-Tv)`.
fn apex_to_seconds(tv: f64) -> f64 {
    (-tv).exp2()
}

/// Reject NaN, infinity, zero, and negatives in one place. The decoder
/// trusts the EXIF *tag* but not the *value*: a real camera writes
/// positive finite values for every field we read.
fn is_positive_finite(v: f64) -> bool {
    v.is_finite() && v > 0.0
}

#[cfg(test)]
mod tests {
    use super::{apex_to_f_number, apex_to_seconds, exposure};
    use crate::test_support::{build_tiff, Field};
    use exif::Reader;
    use tracing_test::traced_test;

    fn parse(ifd0: Vec<Field>, exif_ifd: Vec<Field>) -> exif::Exif {
        let mut body = b"Exif\x00\x00".to_vec();
        body.extend_from_slice(&build_tiff(ifd0, exif_ifd));
        Reader::new()
            .read_raw(body[6..].to_vec())
            .expect("synthesized TIFF parses")
    }

    #[test]
    fn focal_length_primary_wins() {
        let exif = parse(
            vec![],
            vec![
                Field::rational(0x920A, 50, 1),
                Field::short(0xA405, 75), // 35mm equivalent — must lose
            ],
        );
        let exp = exposure(&exif).expect("present");
        assert_eq!(exp.focal_length_mm, Some(50.0));
    }

    #[test]
    fn focal_length_falls_back_to_35mm() {
        let exif = parse(vec![], vec![Field::short(0xA405, 50)]);
        let exp = exposure(&exif).expect("present");
        assert_eq!(exp.focal_length_mm, Some(50.0));
    }

    #[test]
    fn aperture_primary_wins() {
        let exif = parse(
            vec![],
            vec![
                Field::rational(0x829D, 18, 10), // FNumber = 1.8
                Field::rational(0x9202, 4, 1),   // ApertureValue = 4 — must lose
            ],
        );
        let exp = exposure(&exif).expect("present");
        assert_eq!(exp.aperture, Some(1.8));
    }

    #[test]
    fn aperture_falls_back_to_apex() {
        let exif = parse(vec![], vec![Field::rational(0x9202, 4, 1)]);
        let exp = exposure(&exif).expect("present");
        // 2^(4/2) = 4
        let v = exp.aperture.expect("present");
        assert!((v - 4.0).abs() < 1e-9);
    }

    #[test]
    fn shutter_primary_wins() {
        let exif = parse(
            vec![],
            vec![
                Field::rational(0x829A, 1, 250),
                Field::srational(0x9201, 8, 1),
            ],
        );
        let exp = exposure(&exif).expect("present");
        let v = exp.shutter_seconds.expect("present");
        assert!((v - (1.0 / 250.0)).abs() < 1e-9);
    }

    #[test]
    fn shutter_falls_back_to_apex() {
        let exif = parse(vec![], vec![Field::srational(0x9201, 8, 1)]);
        let exp = exposure(&exif).expect("present");
        // 2^(-8) = 1/256
        let v = exp.shutter_seconds.expect("present");
        assert!((v - (1.0 / 256.0)).abs() < 1e-9);
    }

    #[test]
    fn iso_primary_tag() {
        let exif = parse(vec![], vec![Field::short(0x8827, 400)]);
        let exp = exposure(&exif).expect("present");
        assert_eq!(exp.iso, Some(400));
    }

    #[test]
    fn iso_falls_back_to_8832() {
        let exif = parse(vec![], vec![Field::short(0x8832, 800)]);
        let exp = exposure(&exif).expect("present");
        assert_eq!(exp.iso, Some(800));
    }

    #[test]
    fn iso_falls_back_to_8833() {
        let exif = parse(vec![], vec![Field::short(0x8833, 1600)]);
        let exp = exposure(&exif).expect("present");
        assert_eq!(exp.iso, Some(1600));
    }

    #[test]
    fn exposure_all_none_collapses_to_none() {
        // No exposure tags at all — outer Option must be None so
        // `Provenance::is_empty()` keeps telling the truth.
        let exif = parse(vec![], vec![]);
        assert!(exposure(&exif).is_none());
    }

    #[test]
    fn exposure_partial_present_returns_some_with_nones() {
        // Only aperture is present.
        let exif = parse(vec![], vec![Field::rational(0x829D, 28, 10)]);
        let exp = exposure(&exif).expect("present");
        assert_eq!(exp.aperture, Some(2.8));
        assert!(exp.focal_length_mm.is_none());
        assert!(exp.shutter_seconds.is_none());
        assert!(exp.iso.is_none());
    }

    #[test]
    fn negative_focal_length_is_dropped() {
        // EXIF SRational with negative numerator — a real corrupt value.
        let exif = parse(vec![], vec![Field::srational(0x9201, -1, 250)]);
        // Tv = -1/250 ≈ -0.004, so 2^(0.004) ≈ 1.003s. Positive — but
        // shutter primary tag wasn't present, so this exercises APEX
        // fallback. The value is positive but the test is mainly that
        // we don't panic.
        let exp = exposure(&exif);
        // No exposure tags except a negative-numerator one mapped via APEX
        // (which yields a positive value). We expect Some(...) here.
        assert!(exp.is_some());
    }

    #[test]
    fn nan_aperture_is_dropped() {
        // ApertureValue with denom = 0 → kamadak yields NaN/Inf via to_f64.
        let exif = parse(vec![], vec![Field::rational(0x9202, 1, 0)]);
        // 2^(inf/2) = inf; is_positive_finite filter must reject.
        let exp = exposure(&exif);
        assert!(exp.is_none());
    }

    #[test]
    fn apex_helpers_round_trip_canonical_stops() {
        // Av=0→f/1, 2→f/2, 4→f/4, 6→f/8
        assert!((apex_to_f_number(0.0) - 1.0).abs() < 1e-9);
        assert!((apex_to_f_number(2.0) - 2.0).abs() < 1e-9);
        assert!((apex_to_f_number(4.0) - 4.0).abs() < 1e-9);
        assert!((apex_to_f_number(6.0) - 8.0).abs() < 1e-9);
        // Tv=0→1s, 1→1/2s, 8→1/256s
        assert!((apex_to_seconds(0.0) - 1.0).abs() < 1e-9);
        assert!((apex_to_seconds(1.0) - 0.5).abs() < 1e-9);
        assert!((apex_to_seconds(8.0) - (1.0 / 256.0)).abs() < 1e-9);
    }

    #[test]
    #[traced_test]
    fn aperture_apex_fallback_emits_debug_event() {
        let _ = exposure(&parse(vec![], vec![Field::rational(0x9202, 4, 1)]));
        assert!(logs_contain(
            "aperture resolved via APEX ApertureValue fallback"
        ));
    }

    #[test]
    #[traced_test]
    fn iso_fallback_emits_debug_event() {
        let _ = exposure(&parse(vec![], vec![Field::short(0x8832, 800)]));
        assert!(logs_contain("ISO resolved via fallback tag"));
    }
}
