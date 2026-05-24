//! Read EXIF metadata from an arbitrary container or a bare blob.
//!
//! The reader distinguishes three outcomes:
//! - [`ExifReadOutcome::Absent`] — the container simply has no EXIF
//!   segment (most PNG files, screenshots, etc.). Silent: no event emitted.
//! - [`ExifReadOutcome::Parsed`] — kamadak-exif handed back a valid
//!   [`exif::Exif`] we can walk.
//! - [`ExifReadOutcome::Malformed`] — EXIF was present but the parser
//!   rejected it. We emit a `warn` so the failure is *observable* (the
//!   "no silent fallback" project rule) but we do not fail the decode —
//!   the pixels are still usable.

use std::io::Cursor;

use exif::{Exif, In, Tag};

/// What the EXIF reader found.
pub(crate) enum ExifReadOutcome {
    Absent,
    Parsed(Exif),
    Malformed(exif::Error),
}

impl std::fmt::Debug for ExifReadOutcome {
    // `exif::Exif` doesn't implement `Debug`, so we summarise without
    // dumping the parsed contents — what callers care about for diagnostics
    // is which arm we ended up in, not which tags were inside.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Absent => f.write_str("ExifReadOutcome::Absent"),
            Self::Parsed(_) => f.write_str("ExifReadOutcome::Parsed(..)"),
            Self::Malformed(e) => f
                .debug_tuple("ExifReadOutcome::Malformed")
                .field(e)
                .finish(),
        }
    }
}

impl ExifReadOutcome {
    /// Get the parsed EXIF if one was successfully read.
    pub(crate) const fn as_parsed(&self) -> Option<&Exif> {
        match self {
            Self::Parsed(e) => Some(e),
            Self::Absent | Self::Malformed(_) => None,
        }
    }

    /// `true` when EXIF was present at all (whether parseable or not).
    /// Useful for the `exif_present` tracing field.
    pub(crate) const fn was_present(&self) -> bool {
        matches!(self, Self::Parsed(_) | Self::Malformed(_))
    }
}

/// Read EXIF from a container-format input (JPEG, TIFF, ...).
pub(crate) fn read(bytes: &[u8]) -> ExifReadOutcome {
    match exif::Reader::new().read_from_container(&mut Cursor::new(bytes)) {
        Ok(parsed) => ExifReadOutcome::Parsed(parsed),
        Err(exif::Error::NotFound(_)) => ExifReadOutcome::Absent,
        Err(error) => {
            tracing::warn!(?error, "EXIF segment present but failed to parse");
            ExifReadOutcome::Malformed(error)
        },
    }
}

/// Read EXIF from a bare TIFF blob. Used by the HEIF path after stripping
/// the 4-byte `tiff_header_offset` prefix that HEIF wraps the EXIF item in.
#[cfg_attr(
    not(feature = "heif"),
    allow(
        dead_code,
        reason = "only the heif decode path consumes raw TIFF blobs"
    )
)]
pub(crate) fn read_raw(blob: &[u8]) -> ExifReadOutcome {
    match exif::Reader::new().read_raw(blob.to_vec()) {
        Ok(parsed) => ExifReadOutcome::Parsed(parsed),
        Err(exif::Error::NotFound(_)) => ExifReadOutcome::Absent,
        Err(error) => {
            tracing::warn!(?error, "EXIF segment present but failed to parse");
            ExifReadOutcome::Malformed(error)
        },
    }
}

/// Return the raw EXIF `Orientation` tag value, if any. Pass directly to
/// [`crate::orientation::apply`].
pub(crate) fn orientation_value(exif: Option<&Exif>) -> Option<u32> {
    exif?
        .get_field(Tag::Orientation, In::PRIMARY)?
        .value
        .get_uint(0)
}

#[cfg(test)]
mod tests {
    use super::{orientation_value, read, ExifReadOutcome};
    use crate::test_support::{jpeg_with_app1, tiff_with_orientation};
    use tracing_test::traced_test;

    #[test]
    fn read_absent_when_no_exif_in_container() {
        // A bare PNG signature without any IDAT — kamadak only cares about
        // whether the container exposes an EXIF segment.
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let outcome = read(&png);
        assert!(matches!(outcome, ExifReadOutcome::Absent));
        assert!(!outcome.was_present());
        assert!(outcome.as_parsed().is_none());
    }

    #[test]
    fn read_parses_jpeg_with_exif() {
        let bytes = jpeg_with_app1(8, 8, &tiff_with_orientation(6));
        let outcome = read(&bytes);
        let exif = outcome.as_parsed().expect("parsed");
        assert_eq!(orientation_value(Some(exif)), Some(6));
        assert!(outcome.was_present());
    }

    #[test]
    #[traced_test]
    fn read_malformed_emits_warn() {
        // APP1 segment with the "Exif\0\0" identifier kamadak looks for,
        // followed by bytes that can't be parsed as a TIFF header.
        let mut body = b"Exif\x00\x00".to_vec();
        body.extend_from_slice(b"\x00\x01\x02\x03\x04");
        let bytes = jpeg_with_app1(8, 8, &body);
        let outcome = read(&bytes);
        assert!(matches!(outcome, ExifReadOutcome::Malformed(_)));
        assert!(outcome.was_present());
        assert!(logs_contain("EXIF segment present but failed to parse"));
    }

    #[test]
    fn orientation_value_returns_none_for_no_exif() {
        assert_eq!(orientation_value(None), None);
    }
}
