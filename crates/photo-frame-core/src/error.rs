//! Typed errors for the framing pipeline.
//!
//! The public [`FrameError`] enum is **flat** and **structured** — flat to
//! keep `match` ergonomic for callers, structured so every variant carries
//! the diagnostic context an operator needs to understand the failure
//! without a debugger. Every variant that wraps an upstream error preserves
//! the cause chain via `#[source]`.
//!
//! Stable exit-code semantics for the CLI live in [`FrameError::category`].

use std::ops::RangeInclusive;

use thiserror::Error;

/// Stable category for an error, used by the CLI to map to a deterministic
/// exit code. Numeric values are part of the public contract and must not
/// be renumbered.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorCategory {
    /// Caller supplied invalid arguments / options.
    Input = 2,
    /// Input bytes could not be parsed as a supported image format.
    Decode = 3,
    /// Output image could not be produced (encoder failure, OOM, …).
    Encode = 4,
    /// Pipeline reached a geometry / layout dead end.
    Layout = 5,
}

impl ErrorCategory {
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        self as i32
    }
}

/// Public error type returned by every fallible function in `photo-frame-core`.
///
/// Each variant carries enough context to be actionable on its own:
/// pattern-match the variant, read its fields, and you have the diagnosis.
#[derive(Debug, Error)]
pub enum FrameError {
    /// The caller passed an empty byte slice — there is nothing to decode.
    #[error("input is empty (0 bytes)")]
    EmptyInput,

    /// JPEG quality was out of the supported range.
    #[error("JPEG quality {got} is out of range; must be within {} ..= {}",
            .valid.start(), .valid.end())]
    QualityOutOfRange { got: u8, valid: RangeInclusive<u8> },

    /// The image decoder rejected the input bytes.
    #[error("failed to decode input image as JPEG/PNG")]
    Decode(#[source] image::ImageError),

    /// Decoded image has at least one zero dimension. This can happen after
    /// extreme downscaling or with malformed input that the decoder accepted
    /// but produced nothing visible from.
    #[error("image dimensions are degenerate after decode: {width} x {height}")]
    ZeroDimension { width: u32, height: u32 },

    /// The JPEG encoder failed (typically: out of memory).
    #[error("failed to encode output JPEG")]
    Encode(#[source] image::ImageError),
}

impl FrameError {
    /// Stable category used to map errors to CLI exit codes.
    #[must_use]
    pub const fn category(&self) -> ErrorCategory {
        match self {
            Self::EmptyInput | Self::QualityOutOfRange { .. } => ErrorCategory::Input,
            Self::Decode(_) => ErrorCategory::Decode,
            Self::ZeroDimension { .. } => ErrorCategory::Layout,
            Self::Encode(_) => ErrorCategory::Encode,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorCategory, FrameError};

    #[test]
    fn category_to_exit_code_is_stable() {
        // These values are part of the public contract — if you change them
        // you must also update the CLI man page and bump the major version.
        assert_eq!(ErrorCategory::Input.exit_code(), 2);
        assert_eq!(ErrorCategory::Decode.exit_code(), 3);
        assert_eq!(ErrorCategory::Encode.exit_code(), 4);
        assert_eq!(ErrorCategory::Layout.exit_code(), 5);
    }

    #[test]
    fn empty_input_renders_with_byte_count() {
        let err = FrameError::EmptyInput;
        assert_eq!(err.to_string(), "input is empty (0 bytes)");
        assert_eq!(err.category(), ErrorCategory::Input);
    }

    #[test]
    fn quality_renders_with_bounds() {
        let err = FrameError::QualityOutOfRange {
            got: 150,
            valid: 1..=100,
        };
        assert_eq!(
            err.to_string(),
            "JPEG quality 150 is out of range; must be within 1 ..= 100"
        );
        assert_eq!(err.category(), ErrorCategory::Input);
    }

    #[test]
    fn zero_dimension_includes_both_axes() {
        let err = FrameError::ZeroDimension {
            width: 0,
            height: 10,
        };
        assert_eq!(
            err.to_string(),
            "image dimensions are degenerate after decode: 0 x 10"
        );
        assert_eq!(err.category(), ErrorCategory::Layout);
    }
}
