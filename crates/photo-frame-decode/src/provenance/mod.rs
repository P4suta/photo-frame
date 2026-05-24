//! Project an `exif::Exif` onto a structured [`Provenance`].
//!
//! Each sub-extractor is responsible for *one* outer `Option<T>` field of
//! the [`Provenance`] (`camera`, `lens`, `exposure`, `captured_at`) and
//! must enforce the **collapse rule**: if no inner field could be filled,
//! return `None` so [`Provenance::is_empty()`] keeps telling the truth.
//!
//! Display formatting (`"105mm"`, `"f/1.8"`, `"1/250s"`, `"2026-05-24"`)
//! lives in the frame crate — this module deals in primitives only.

pub(crate) mod camera;
pub(crate) mod datetime;
pub(crate) mod exposure;

use exif::{Exif, In, Tag, Value};
use photo_frame_types::Provenance;

pub(crate) fn extract(exif: &Exif) -> Provenance {
    let prov = Provenance {
        camera: camera::camera(exif),
        lens: camera::lens(exif),
        exposure: exposure::exposure(exif),
        captured_at: datetime::captured_at(exif),
    };
    if prov.is_empty() {
        tracing::info!("EXIF parsed but no caller-visible facts resolved");
    }
    prov
}

// ── shared tag readers ────────────────────────────────────────────────────

/// Read an ASCII tag, trimming the trailing NUL and whitespace. Returns
/// `None` for absent, non-ASCII, or empty-after-trim values.
pub(super) fn ascii(exif: &Exif, tag: Tag) -> Option<String> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    let Value::Ascii(ref bytes) = field.value else {
        return None;
    };
    let first = bytes.first()?;
    let s = std::str::from_utf8(first).ok()?;
    let cleaned = s.trim_end_matches('\0').trim();
    (!cleaned.is_empty()).then(|| cleaned.to_owned())
}

/// Read the first element of a `Rational` *or* `SRational` tag as `f64`.
/// Some writers store APEX values as RATIONAL despite the spec calling
/// for SRATIONAL — accepting both shields us from that quirk.
pub(super) fn rational_f64(exif: &Exif, tag: Tag) -> Option<f64> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    match &field.value {
        Value::Rational(vs) => vs.first().map(exif::Rational::to_f64),
        Value::SRational(vs) => vs.first().map(exif::SRational::to_f64),
        _ => None,
    }
}

/// Read a SHORT/LONG tag as `u32`.
pub(super) fn uint(exif: &Exif, tag: Tag) -> Option<u32> {
    exif.get_field(tag, In::PRIMARY)?.value.get_uint(0)
}
