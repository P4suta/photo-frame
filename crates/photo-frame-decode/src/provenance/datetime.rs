//! Capture timestamp extraction.
//!
//! Walks the standard EXIF fallback chain:
//! `DateTimeOriginal` → `DateTimeDigitized` → `DateTime`.
//!
//! Returns a [`DateTime`] with each component parsed as a primitive
//! integer (so the renderer can format as needed). Invalid components
//! (out of range, non-numeric) drop the candidate; the next tag in the
//! chain is tried before giving up.

use exif::{Exif, Tag};
use photo_frame_types::DateTime;

use super::ascii;

pub(crate) fn captured_at(exif: &Exif) -> Option<DateTime> {
    const CANDIDATES: [(Tag, &str); 3] = [
        (Tag::DateTimeOriginal, "DateTimeOriginal"),
        (Tag::DateTimeDigitized, "DateTimeDigitized"),
        (Tag::DateTime, "DateTime"),
    ];
    let mut iter = CANDIDATES.iter();
    let primary = iter.next().expect("non-empty candidate list");
    if let Some(dt) = ascii(exif, primary.0).as_deref().and_then(parse) {
        return Some(dt);
    }
    for (tag, name) in iter {
        if let Some(dt) = ascii(exif, *tag).as_deref().and_then(parse) {
            tracing::debug!(source = name, "date resolved via fallback tag");
            return Some(dt);
        }
    }
    tracing::debug!(
        event_id = "decode.exif.datetime.exhausted",
        candidates = CANDIDATES.len(),
        "every EXIF datetime candidate missing or unparsable; captured_at = None"
    );
    None
}

/// Parse an EXIF datetime string. Accepts `"YYYY:MM:DD HH:MM:SS"` (the
/// canonical form) and `"YYYY:MM:DD"` alone (HMS defaults to zero). Any
/// failure to parse a component as the required `u8` / `u16` returns
/// `None`, which lets the fallback chain advance.
fn parse(s: &str) -> Option<DateTime> {
    let trimmed = s.trim().trim_end_matches('\0');
    let (date_part, time_part) = match trimmed.split_once(' ') {
        Some((d, t)) => (d, t),
        None => (trimmed, ""),
    };

    let mut date = date_part.splitn(3, ':');
    let year: u16 = date.next()?.parse().ok()?;
    let month: u8 = date.next()?.parse().ok()?;
    let day: u8 = date.next()?.parse().ok()?;

    let (hour, minute, second) = if time_part.is_empty() {
        (0_u8, 0_u8, 0_u8)
    } else {
        let mut t = time_part.splitn(3, ':');
        let h: u8 = t.next()?.parse().ok()?;
        let m: u8 = t.next()?.parse().ok()?;
        let s: u8 = t.next()?.parse().ok()?;
        (h, m, s)
    };

    Some(DateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
    })
}

#[cfg(test)]
mod tests {
    use super::{captured_at, parse};
    use crate::test_support::{build_tiff, Field};
    use exif::Reader;
    use photo_frame_types::DateTime;
    use tracing_test::traced_test;

    fn exif_with(ifd0: Vec<Field>, exif_ifd: Vec<Field>) -> exif::Exif {
        let mut body = b"Exif\x00\x00".to_vec();
        body.extend_from_slice(&build_tiff(ifd0, exif_ifd));
        Reader::new()
            .read_raw(body[6..].to_vec())
            .expect("synthesized TIFF parses")
    }

    #[test]
    fn parse_full_datetime() {
        let dt = parse("2026:05:24 13:24:55").expect("present");
        assert_eq!(
            dt,
            DateTime {
                year: 2026,
                month: 5,
                day: 24,
                hour: 13,
                minute: 24,
                second: 55,
            }
        );
    }

    #[test]
    fn parse_date_only_zeroes_hms() {
        let dt = parse("2026:05:24").expect("present");
        assert_eq!(dt.year, 2026);
        assert_eq!(dt.month, 5);
        assert_eq!(dt.day, 24);
        assert_eq!(dt.hour, 0);
        assert_eq!(dt.minute, 0);
        assert_eq!(dt.second, 0);
    }

    #[test]
    fn parse_garbage_returns_none() {
        assert!(parse("not a date").is_none());
    }

    #[test]
    fn parse_out_of_range_month_returns_none() {
        // Month "1300" overflows u8 — parse fails, the whole datetime
        // drops and the fallback chain advances.
        assert!(parse("2026:1300:24 10:00:00").is_none());
    }

    #[test]
    fn captured_at_primary_wins() {
        let exif = exif_with(
            vec![Field::ascii(0x0132, "1990:01:01 00:00:00")],
            vec![Field::ascii(0x9003, "2026:05:24 10:00:00")],
        );
        let dt = captured_at(&exif).expect("present");
        assert_eq!(dt.year, 2026);
    }

    #[test]
    fn captured_at_falls_back_to_digitized() {
        let exif = exif_with(vec![], vec![Field::ascii(0x9004, "2026:05:24 10:00:00")]);
        let dt = captured_at(&exif).expect("present");
        assert_eq!((dt.year, dt.month, dt.day), (2026, 5, 24));
    }

    #[test]
    fn captured_at_falls_back_to_ifd0_datetime() {
        let exif = exif_with(vec![Field::ascii(0x0132, "2026:05:24 10:00:00")], vec![]);
        let dt = captured_at(&exif).expect("present");
        assert_eq!((dt.year, dt.month, dt.day), (2026, 5, 24));
    }

    #[test]
    fn captured_at_returns_none_when_absent() {
        let exif = exif_with(vec![], vec![]);
        assert!(captured_at(&exif).is_none());
    }

    #[test]
    #[traced_test]
    fn captured_at_fallback_emits_debug_event() {
        let _ = captured_at(&exif_with(
            vec![],
            vec![Field::ascii(0x9004, "2026:05:24 10:00:00")],
        ));
        assert!(logs_contain("date resolved via fallback tag"));
    }
}
