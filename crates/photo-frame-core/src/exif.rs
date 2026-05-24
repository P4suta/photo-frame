//! EXIF reading and minimal display formatting.
//!
//! We deliberately project the wide EXIF surface onto the small set of fields
//! needed for the caption strip. The reader is forgiving:
//!
//! * every visible field has a fallback chain — if the canonical tag is
//!   absent we try equivalent tags (e.g. APEX-encoded `ApertureValue` for
//!   `FNumber`) before giving up;
//! * missing tags drop out of the rendered caption rather than failing the
//!   pipeline — the design choice from [`feedback-no-allow-lists`] not to
//!   invent placeholder values like `—`.

use std::io::Cursor;

use exif::{Context, Exif, In, Tag, Value};
use tracing::{debug, warn};

use crate::orientation::Orientation;

/// EXIF 2.3 spreads "ISO value" across three tags depending on what the
/// camera authored. We try the canonical [`Tag::PhotographicSensitivity`]
/// first, then fall back to `RecommendedExposureIndex` (0x8832, used by
/// Nikon Z bodies when `SensitivityType = 2`) and `ISOSpeed` (0x8833).
const ISO_TAG_CANDIDATES: [Tag; 3] = [
    Tag::PhotographicSensitivity,
    Tag(Context::Exif, 0x8832),
    Tag(Context::Exif, 0x8833),
];

/// Caption-ready metadata for the framed photo.
///
/// Each field is `Option` so the caller can decide what to render based on
/// what is actually present; the pipeline never invents placeholder values
/// for missing tags.
#[derive(Clone, Debug, Default)]
pub(crate) struct Meta {
    pub(crate) camera: Option<String>,
    pub(crate) lens: Option<String>,
    pub(crate) focal_length: Option<String>,
    pub(crate) aperture: Option<String>,
    pub(crate) shutter: Option<String>,
    pub(crate) iso: Option<String>,
    pub(crate) date: Option<String>,
}

impl Meta {
    pub(crate) const fn is_empty(&self) -> bool {
        self.camera.is_none()
            && self.lens.is_none()
            && self.focal_length.is_none()
            && self.aperture.is_none()
            && self.shutter.is_none()
            && self.iso.is_none()
            && self.date.is_none()
    }

    /// Exposure parameters joined by middle-dot, in shooter-canonical order.
    /// Returns `None` if no exposure fact is known.
    pub(crate) fn exposure_line(&self) -> Option<String> {
        let parts: Vec<&str> = [
            self.focal_length.as_deref(),
            self.aperture.as_deref(),
            self.shutter.as_deref(),
            self.iso.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect();
        (!parts.is_empty()).then(|| parts.join("  ·  "))
    }
}

/// Try to parse EXIF out of the raw input bytes. Returns `None` for inputs
/// without EXIF (e.g. most `.png` files).
pub(crate) fn read(bytes: &[u8]) -> Option<Exif> {
    exif::Reader::new()
        .read_from_container(&mut Cursor::new(bytes))
        .ok()
}

/// Read the EXIF Orientation tag. Defaults to [`Orientation::Normal`] when the
/// tag is absent or unparsable, matching common viewer behaviour.
pub(crate) fn orientation_of(exif: Option<&Exif>) -> Orientation {
    let raw = exif
        .and_then(|e| e.get_field(Tag::Orientation, In::PRIMARY))
        .and_then(|f| f.value.get_uint(0))
        .unwrap_or(1);
    Orientation::from_raw(u16::try_from(raw).unwrap_or(1))
}

/// Project EXIF onto our caption-ready [`Meta`], walking each field's
/// fallback chain.
pub(crate) fn extract(exif: &Exif) -> Meta {
    let meta = Meta {
        camera: ascii(exif, Tag::Model).as_deref().map(strip_corporation),
        lens: ascii(exif, Tag::LensModel),
        focal_length: first_focal_length(exif),
        aperture: first_aperture(exif),
        shutter: first_shutter(exif),
        iso: first_iso(exif).map(|v| format!("ISO {v}")),
        date: first_date(exif),
    };
    if meta.aperture.is_none() && meta.shutter.is_none() && meta.iso.is_none() {
        warn!("EXIF present but no exposure facts (aperture/shutter/ISO) were resolved");
    }
    meta
}

// ─── Field assemblers (primary tag → fallback chain) ─────────────────────────

fn first_focal_length(exif: &Exif) -> Option<String> {
    if let Some(v) = rational_f64(exif, Tag::FocalLength).and_then(format_focal_length) {
        return Some(v);
    }
    // FocalLengthIn35mmFilm is a SHORT, not a Rational. On crop sensors it
    // differs from the optical focal length; on full-frame bodies it's
    // identical. Either way it's the right thing to show when the primary
    // value is unavailable.
    if let Some(v) = uint(exif, Tag::FocalLengthIn35mmFilm)
        .map(f64::from)
        .and_then(format_focal_length)
    {
        debug!(value = %v, "focal length resolved via FocalLengthIn35mmFilm fallback");
        return Some(v);
    }
    None
}

fn first_aperture(exif: &Exif) -> Option<String> {
    if let Some(v) = rational_f64(exif, Tag::FNumber).and_then(format_aperture) {
        return Some(v);
    }
    // ApertureValue is APEX-encoded: F = 2^(Av/2).
    if let Some(av) = rational_f64(exif, Tag::ApertureValue) {
        if let Some(v) = format_aperture(apex_to_f_number(av)) {
            debug!(av, value = %v, "aperture resolved via APEX ApertureValue fallback");
            return Some(v);
        }
    }
    None
}

fn first_shutter(exif: &Exif) -> Option<String> {
    if let Some(v) = rational_f64(exif, Tag::ExposureTime).and_then(format_seconds) {
        return Some(v);
    }
    // ShutterSpeedValue is APEX-encoded: T = 2^(-Tv).
    if let Some(tv) = rational_f64(exif, Tag::ShutterSpeedValue) {
        if let Some(v) = format_seconds(apex_to_seconds(tv)) {
            debug!(tv, value = %v, "shutter resolved via APEX ShutterSpeedValue fallback");
            return Some(v);
        }
    }
    None
}

fn first_iso(exif: &Exif) -> Option<u32> {
    let primary = uint(exif, Tag::PhotographicSensitivity);
    if let Some(v) = primary {
        return Some(v);
    }
    for &tag in ISO_TAG_CANDIDATES.iter().skip(1) {
        if let Some(v) = uint(exif, tag) {
            debug!(
                tag = format!("0x{:04X}", tag.number()),
                value = v,
                "ISO resolved via fallback tag"
            );
            return Some(v);
        }
    }
    None
}

fn first_date(exif: &Exif) -> Option<String> {
    const DATE_TAG_CANDIDATES: [(Tag, &str); 3] = [
        (Tag::DateTimeOriginal, "DateTimeOriginal"),
        (Tag::DateTimeDigitized, "DateTimeDigitized"),
        (Tag::DateTime, "DateTime"),
    ];
    let mut iter = DATE_TAG_CANDIDATES.iter();
    let primary = iter.next().expect("non-empty candidate list");
    if let Some(s) = ascii(exif, primary.0).as_deref().and_then(format_date) {
        return Some(s);
    }
    for (tag, name) in iter {
        if let Some(s) = ascii(exif, *tag).as_deref().and_then(format_date) {
            debug!(source = name, value = %s, "date resolved via fallback tag");
            return Some(s);
        }
    }
    None
}

// ─── Low-level field helpers ─────────────────────────────────────────────────

fn ascii(exif: &Exif, tag: Tag) -> Option<String> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    let Value::Ascii(ref bytes) = field.value else {
        return None;
    };
    let first = bytes.first()?;
    let s = std::str::from_utf8(first).ok()?;
    let cleaned = s.trim_end_matches('\0').trim();
    (!cleaned.is_empty()).then(|| cleaned.to_owned())
}

/// Read the first element of a `Rational` *or* `SRational` field as `f64`.
/// EXIF lets some tags switch type between signed and unsigned (e.g. some
/// writers store `ShutterSpeedValue` as RATIONAL despite the spec calling
/// for SRATIONAL); accepting both shields the caller from that quirk.
fn rational_f64(exif: &Exif, tag: Tag) -> Option<f64> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    match &field.value {
        Value::Rational(vs) => vs.first().map(exif::Rational::to_f64),
        Value::SRational(vs) => vs.first().map(exif::SRational::to_f64),
        _ => None,
    }
}

fn uint(exif: &Exif, tag: Tag) -> Option<u32> {
    exif.get_field(tag, In::PRIMARY)?.value.get_uint(0)
}

// ─── APEX conversions ───────────────────────────────────────────────────────

/// APEX aperture value → F-number. `F = 2^(Av/2)`.
fn apex_to_f_number(av: f64) -> f64 {
    (av / 2.0).exp2()
}

/// APEX shutter value → exposure time in seconds. `T = 2^(-Tv)`.
fn apex_to_seconds(tv: f64) -> f64 {
    (-tv).exp2()
}

// ─── Display formatters ──────────────────────────────────────────────────────

/// Nikon writes `Make = "NIKON CORPORATION"` and `Model = "NIKON Z 8"`; the
/// Model already carries the brand, so we strip the legalese suffix from any
/// trailing "CORPORATION" the body name might contain.
fn strip_corporation(model: &str) -> String {
    let trimmed = model.trim_end_matches(char::is_whitespace);
    trimmed
        .strip_suffix("CORPORATION")
        .map_or_else(|| trimmed.to_owned(), |s| s.trim_end().to_owned())
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

/// EXIF dates are `"YYYY:MM:DD HH:MM:SS"`; we render only the calendar day
/// as `YYYY-MM-DD` for caption use.
fn format_date(s: &str) -> Option<String> {
    let date_part = s.split_whitespace().next()?;
    let mut parts = date_part.splitn(3, ':');
    let y = parts.next()?;
    let m = parts.next()?;
    let d = parts.next()?;
    Some(format!("{y}-{m}-{d}"))
}

#[cfg(test)]
mod tests {
    use super::{
        apex_to_f_number, apex_to_seconds, format_aperture, format_date, format_focal_length,
        format_seconds, strip_corporation, Meta,
    };

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
        assert_eq!(format_seconds(1.0 / 13.0).as_deref(), Some("1/13s"));
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
    fn apex_aperture_round_trips_canonical_stops() {
        // Av = 0 → f/1, 2 → f/2, 4 → f/4, 6 → f/8 — the round stops.
        assert!((apex_to_f_number(0.0) - 1.0).abs() < 1e-9);
        assert!((apex_to_f_number(2.0) - 2.0).abs() < 1e-9);
        assert!((apex_to_f_number(4.0) - 4.0).abs() < 1e-9);
        assert!((apex_to_f_number(6.0) - 8.0).abs() < 1e-9);
    }

    #[test]
    fn apex_aperture_handles_third_stops() {
        // Av = 2.6 → f/2.46 (a real third-stop on Nikon bodies).
        let f = apex_to_f_number(2.6);
        assert_eq!(format_aperture(f).as_deref(), Some("f/2.5"));
    }

    #[test]
    fn apex_shutter_round_trips_canonical_stops() {
        // Tv = 0 → 1s, 1 → 1/2s, 8 → 1/256s.
        assert!((apex_to_seconds(0.0) - 1.0).abs() < 1e-9);
        assert!((apex_to_seconds(1.0) - 0.5).abs() < 1e-9);
        assert!((apex_to_seconds(8.0) - (1.0 / 256.0)).abs() < 1e-9);
    }

    #[test]
    fn apex_shutter_then_format_matches_canonical_speeds() {
        // Tv ≈ 7.97 = 1/250s on Nikon bodies (APEX rounded to nearest third stop).
        let secs = apex_to_seconds(7.965_784_284_662_087); // log2(250)
        assert_eq!(format_seconds(secs).as_deref(), Some("1/250s"));
    }

    #[test]
    fn date_drops_time_and_uses_dashes() {
        assert_eq!(
            format_date("2026:05:24 13:24:55").as_deref(),
            Some("2026-05-24")
        );
    }

    #[test]
    fn date_handles_missing_time() {
        assert_eq!(format_date("2026:05:24").as_deref(), Some("2026-05-24"));
    }

    #[test]
    fn strip_corporation_handles_nikon_form() {
        assert_eq!(strip_corporation("NIKON CORPORATION"), "NIKON");
        assert_eq!(strip_corporation("NIKON Z 8"), "NIKON Z 8");
        assert_eq!(strip_corporation("SONY"), "SONY");
    }

    #[test]
    fn exposure_line_joins_present_facts_only() {
        let meta = Meta {
            focal_length: Some("50mm".into()),
            shutter: Some("1/250s".into()),
            ..Meta::default()
        };
        assert_eq!(meta.exposure_line().as_deref(), Some("50mm  ·  1/250s"));
    }

    #[test]
    fn exposure_line_empty_when_nothing_known() {
        assert_eq!(Meta::default().exposure_line(), None);
    }

    #[test]
    fn meta_is_empty_when_default() {
        assert!(Meta::default().is_empty());
    }

    // ─── Fallback path tests ─────────────────────────────────────────────
    //
    // These drive `extract()` end-to-end with a synthesized TIFF/EXIF blob,
    // so they exercise the multi-tag fallback chains as a kamadak-exif
    // consumer would see them. Each test only writes the *secondary* tag
    // for the field under test, asserting the primary-absent path resolves
    // correctly.

    use super::{extract, read};

    /// One IFD entry described by tag, EXIF type code, count, and raw
    /// big-endian bytes for the value. Inline-vs-external placement is the
    /// builder's concern, not ours.
    struct Field {
        tag: u16,
        ty: u16,
        count: u32,
        data: Vec<u8>,
    }

    impl Field {
        fn short(tag: u16, v: u16) -> Self {
            Self {
                tag,
                ty: 3,
                count: 1,
                data: v.to_be_bytes().to_vec(),
            }
        }
        fn rational(tag: u16, num: u32, denom: u32) -> Self {
            let mut data = Vec::with_capacity(8);
            data.extend_from_slice(&num.to_be_bytes());
            data.extend_from_slice(&denom.to_be_bytes());
            Self {
                tag,
                ty: 5,
                count: 1,
                data,
            }
        }
        fn srational(tag: u16, num: i32, denom: i32) -> Self {
            let mut data = Vec::with_capacity(8);
            data.extend_from_slice(&num.to_be_bytes());
            data.extend_from_slice(&denom.to_be_bytes());
            Self {
                tag,
                ty: 10,
                count: 1,
                data,
            }
        }
        fn ascii(tag: u16, s: &str) -> Self {
            let mut data = s.as_bytes().to_vec();
            data.push(0);
            let count = u32::try_from(data.len()).expect("ascii field fits u32");
            Self {
                tag,
                ty: 2,
                count,
                data,
            }
        }
    }

    /// Build a minimal big-endian TIFF carrying the given IFD0 / Exif IFD
    /// entries. Returned bytes are directly parseable by `exif::Reader`.
    fn build_tiff(mut ifd0: Vec<Field>, mut exif: Vec<Field>) -> Vec<u8> {
        ifd0.sort_by_key(|f| f.tag);
        exif.sort_by_key(|f| f.tag);

        let has_exif_ifd = !exif.is_empty();
        let ifd0_entry_count = ifd0.len() + usize::from(has_exif_ifd);
        let ifd0_size = 2 + 12 * ifd0_entry_count + 4;
        let ifd0_offset: u32 = 8;
        let ifd0_ext_start = ifd0_offset + u32::try_from(ifd0_size).unwrap();

        let (ifd0_offsets, ifd0_ext_size) = allocate_externals(&ifd0, ifd0_ext_start);
        let exif_ifd_offset = ifd0_ext_start + ifd0_ext_size;
        let exif_ifd_size = if has_exif_ifd {
            2 + 12 * exif.len() + 4
        } else {
            0
        };
        let exif_ext_start = exif_ifd_offset + u32::try_from(exif_ifd_size).unwrap();
        let (exif_offsets, _) = allocate_externals(&exif, exif_ext_start);

        let mut out = Vec::new();
        out.extend_from_slice(b"MM");
        out.extend_from_slice(&0x002A_u16.to_be_bytes());
        out.extend_from_slice(&ifd0_offset.to_be_bytes());

        // Assemble IFD0 entries — inject ExifOffset if needed, then sort.
        let mut ifd0_rendered: Vec<(u16, u16, u32, [u8; 4])> = ifd0
            .iter()
            .zip(&ifd0_offsets)
            .map(|(f, off)| (f.tag, f.ty, f.count, value_field(f, *off)))
            .collect();
        if has_exif_ifd {
            ifd0_rendered.push((0x8769, 4, 1, exif_ifd_offset.to_be_bytes()));
            ifd0_rendered.sort_by_key(|&(tag, _, _, _)| tag);
        }
        write_ifd(&mut out, &ifd0_rendered);

        for (f, off) in ifd0.iter().zip(&ifd0_offsets) {
            if off.is_some() {
                out.extend_from_slice(&f.data);
            }
        }

        if has_exif_ifd {
            let exif_rendered: Vec<(u16, u16, u32, [u8; 4])> = exif
                .iter()
                .zip(&exif_offsets)
                .map(|(f, off)| (f.tag, f.ty, f.count, value_field(f, *off)))
                .collect();
            write_ifd(&mut out, &exif_rendered);
            for (f, off) in exif.iter().zip(&exif_offsets) {
                if off.is_some() {
                    out.extend_from_slice(&f.data);
                }
            }
        }
        out
    }

    fn allocate_externals(fields: &[Field], start: u32) -> (Vec<Option<u32>>, u32) {
        let mut cursor = start;
        let mut offsets = Vec::with_capacity(fields.len());
        for f in fields {
            if f.data.len() <= 4 {
                offsets.push(None);
            } else {
                offsets.push(Some(cursor));
                cursor += u32::try_from(f.data.len()).unwrap();
            }
        }
        (offsets, cursor - start)
    }

    fn value_field(f: &Field, external_offset: Option<u32>) -> [u8; 4] {
        external_offset.map_or_else(
            || {
                let mut v = [0_u8; 4];
                let n = f.data.len().min(4);
                v[..n].copy_from_slice(&f.data[..n]);
                v
            },
            u32::to_be_bytes,
        )
    }

    fn write_ifd(out: &mut Vec<u8>, entries: &[(u16, u16, u32, [u8; 4])]) {
        let count = u16::try_from(entries.len()).expect("IFD entry count fits u16");
        out.extend_from_slice(&count.to_be_bytes());
        for (tag, ty, ct, val) in entries {
            out.extend_from_slice(&tag.to_be_bytes());
            out.extend_from_slice(&ty.to_be_bytes());
            out.extend_from_slice(&ct.to_be_bytes());
            out.extend_from_slice(val);
        }
        out.extend_from_slice(&0_u32.to_be_bytes());
    }

    fn meta_from(ifd0: Vec<Field>, exif: Vec<Field>) -> Meta {
        let bytes = build_tiff(ifd0, exif);
        let parsed = read(&bytes).expect("synthesized TIFF parses");
        extract(&parsed)
    }

    #[test]
    fn builder_round_trips_primary_tags() {
        // Sanity: the synthesizer produces something `extract` can read.
        let meta = meta_from(
            vec![Field::ascii(0x0110, "NIKON Z 5")],
            vec![
                Field::rational(0x829D, 18, 10), // FNumber = 1.8
                Field::rational(0x829A, 1, 250), // ExposureTime = 1/250s
            ],
        );
        assert_eq!(meta.camera.as_deref(), Some("NIKON Z 5"));
        assert_eq!(meta.aperture.as_deref(), Some("f/1.8"));
        assert_eq!(meta.shutter.as_deref(), Some("1/250s"));
    }

    #[test]
    fn aperture_falls_back_to_apex_value() {
        // ApertureValue=4 → F=4. No FNumber present.
        let meta = meta_from(vec![], vec![Field::rational(0x9202, 4, 1)]);
        assert_eq!(meta.aperture.as_deref(), Some("f/4"));
    }

    #[test]
    fn shutter_falls_back_to_apex_value() {
        // ShutterSpeedValue (SRational, APEX): Tv=8 → 1/256s.
        let meta = meta_from(vec![], vec![Field::srational(0x9201, 8, 1)]);
        assert_eq!(meta.shutter.as_deref(), Some("1/256s"));
    }

    #[test]
    fn focal_length_falls_back_to_35mm_equivalent() {
        // FocalLengthIn35mmFilm = 50 (SHORT), no FocalLength.
        let meta = meta_from(vec![], vec![Field::short(0xA405, 50)]);
        assert_eq!(meta.focal_length.as_deref(), Some("50mm"));
    }

    #[test]
    fn iso_falls_back_to_recommended_exposure_index() {
        let meta = meta_from(vec![], vec![Field::short(0x8832, 800)]);
        assert_eq!(meta.iso.as_deref(), Some("ISO 800"));
    }

    #[test]
    fn iso_falls_back_to_iso_speed() {
        let meta = meta_from(vec![], vec![Field::short(0x8833, 1600)]);
        assert_eq!(meta.iso.as_deref(), Some("ISO 1600"));
    }

    #[test]
    fn date_falls_back_to_digitized() {
        let meta = meta_from(vec![], vec![Field::ascii(0x9004, "2026:05:24 10:00:00")]);
        assert_eq!(meta.date.as_deref(), Some("2026-05-24"));
    }

    #[test]
    fn date_falls_back_to_ifd0_datetime() {
        let meta = meta_from(vec![Field::ascii(0x0132, "2026:05:24 10:00:00")], vec![]);
        assert_eq!(meta.date.as_deref(), Some("2026-05-24"));
    }

    #[test]
    fn primary_tag_wins_over_fallback() {
        // FNumber present along with ApertureValue: FNumber wins.
        let meta = meta_from(
            vec![],
            vec![
                Field::rational(0x829D, 18, 10), // FNumber = 1.8
                Field::rational(0x9202, 4, 1),   // ApertureValue = 4
            ],
        );
        assert_eq!(meta.aperture.as_deref(), Some("f/1.8"));
    }

    // ─── Tracing event assertions ────────────────────────────────────────

    #[tracing_test::traced_test]
    #[test]
    fn aperture_fallback_emits_debug_event() {
        let _ = meta_from(vec![], vec![Field::rational(0x9202, 4, 1)]);
        assert!(logs_contain(
            "aperture resolved via APEX ApertureValue fallback"
        ));
    }

    #[tracing_test::traced_test]
    #[test]
    fn shutter_fallback_emits_debug_event() {
        let _ = meta_from(vec![], vec![Field::srational(0x9201, 8, 1)]);
        assert!(logs_contain(
            "shutter resolved via APEX ShutterSpeedValue fallback"
        ));
    }

    #[tracing_test::traced_test]
    #[test]
    fn focal_length_fallback_emits_debug_event() {
        let _ = meta_from(vec![], vec![Field::short(0xA405, 50)]);
        assert!(logs_contain(
            "focal length resolved via FocalLengthIn35mmFilm fallback"
        ));
    }

    #[tracing_test::traced_test]
    #[test]
    fn iso_fallback_emits_debug_event() {
        let _ = meta_from(vec![], vec![Field::short(0x8832, 800)]);
        assert!(logs_contain("ISO resolved via fallback tag"));
    }

    #[tracing_test::traced_test]
    #[test]
    fn date_fallback_emits_debug_event() {
        let _ = meta_from(vec![], vec![Field::ascii(0x9004, "2026:05:24 10:00:00")]);
        assert!(logs_contain("date resolved via fallback tag"));
    }

    #[tracing_test::traced_test]
    #[test]
    fn exposure_absent_emits_warning() {
        // Make+Model are present but nothing exposure-related is — the
        // pipeline should call this out so an operator notices.
        let _ = meta_from(vec![Field::ascii(0x0110, "NIKON Z 5")], vec![]);
        assert!(logs_contain("no exposure facts"));
    }

    #[tracing_test::traced_test]
    #[test]
    fn primary_tags_do_not_emit_fallback_events() {
        let _ = meta_from(
            vec![],
            vec![
                Field::rational(0x829D, 18, 10),
                Field::rational(0x829A, 1, 250),
                Field::rational(0x920A, 50, 1),
                Field::short(0x8827, 200),
                Field::ascii(0x9003, "2026:05:24 10:00:00"),
            ],
        );
        assert!(!logs_contain("fallback"));
    }
}
