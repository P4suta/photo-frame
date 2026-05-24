//! Stable categorisation for every workspace error.
//!
//! Every error variant in every workspace crate maps to exactly one
//! [`Category`] via the [`Categorize`] trait. The CLI consumes this
//! mapping to derive its exit code, observability instrumentation tags
//! every event with the category, and miette's diagnostic renderer
//! lifts the category into the severity colour.
//!
//! Keeping the enum here (in `photo-frame-types`) means *no* downstream
//! crate has to invent its own ad-hoc classification — the contract is
//! shared.

use std::fmt;

/// Coarse-grained error classification.
///
/// Categories are deliberately few — the goal is "what part of the
/// pipeline blew up" granularity, not "exactly which variant", which
/// the individual error enum already names precisely.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    /// Bad input on the caller's side: empty bytes, unknown format,
    /// invalid CLI flag value. The right fix is usually outside the
    /// program (user re-runs with a different argument).
    Input,
    /// Decoding failed midway: corrupt JPEG, unsupported HEIC variant,
    /// HEIF feature disabled. The pixel data could not be produced.
    Decode,
    /// Rendering / framing failed. Currently unused (frame is
    /// infallible by construction) — reserved for forward
    /// compatibility (e.g., font load failure once we externalise the
    /// embedded font).
    Render,
    /// Encoding failed: invalid JPEG quality, encoder I/O error. The
    /// upstream `Pixels` is fine; only the serialisation went wrong.
    Encode,
    /// Anything else — an internal invariant breach the user can't
    /// directly act on. Should be rare and always actionable for
    /// the project's maintainers.
    Internal,
}

impl Category {
    /// Stable CLI exit code per category. The numeric mapping is part
    /// of the CLI's public contract — shell scripts and CI may rely on
    /// it. Keep this in sync with `docs/EVENTS.md`.
    #[must_use]
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Input => 2,
            Self::Decode => 3,
            Self::Render => 4,
            Self::Encode => 5,
            Self::Internal => 1,
        }
    }

    /// Short snake-case label used in tracing events and JSON logs.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Decode => "decode",
            Self::Render => "render",
            Self::Encode => "encode",
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Anything that classifies into a [`Category`]. Every workspace error
/// enum implements this so downstream renderers / CLIs can ask the
/// question generically.
pub trait Categorize {
    fn category(&self) -> Category;
}

#[cfg(test)]
mod tests {
    use super::Category;

    #[test]
    fn exit_codes_cover_every_variant() {
        // If we add a variant we want to be forced to assign it an
        // exit code; this test wedges the contract by listing every
        // variant explicitly.
        for cat in [
            Category::Input,
            Category::Decode,
            Category::Render,
            Category::Encode,
            Category::Internal,
        ] {
            assert!(
                cat.exit_code() > 0,
                "category {cat:?} has invalid exit code"
            );
        }
    }

    #[test]
    fn labels_are_lowercase_words() {
        for cat in [
            Category::Input,
            Category::Decode,
            Category::Render,
            Category::Encode,
            Category::Internal,
        ] {
            let label = cat.label();
            assert_eq!(
                label,
                label.to_lowercase(),
                "label must be lowercase: {label}"
            );
            assert!(!label.is_empty(), "label must be non-empty");
        }
    }

    #[test]
    fn input_exits_with_2_by_convention() {
        // Sysexits.h convention: 2 is "misuse of shell builtins"; many
        // CLI tools repurpose it for "user input invalid". Lock this
        // here so a refactor can't silently renumber.
        assert_eq!(Category::Input.exit_code(), 2);
    }
}
