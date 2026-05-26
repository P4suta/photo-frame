//! Paired frame colour + ink colour preset and the meta-strip policy
//! enum that travels with it.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::primitives::Rgba8;

/// Paired frame colour + ink colour preset.
///
/// Project policy is that these two values *travel together* — "white
/// frame with black text" vs "black frame with white text" is one
/// decision, not two — so the renderer exposes them as one enum
/// rather than two independent colour fields.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrameTheme {
    /// White frame, black ink.
    #[default]
    Paper,
    /// Black frame, white ink.
    Ink,
}

impl FrameTheme {
    /// Fill colour for the frame border.
    #[must_use]
    pub const fn background(self) -> Rgba8 {
        match self {
            Self::Paper => Rgba8::WHITE,
            Self::Ink => Rgba8::BLACK,
        }
    }

    /// Colour for caption text painted over the frame.
    #[must_use]
    pub const fn ink(self) -> Rgba8 {
        match self {
            Self::Paper => Rgba8::BLACK,
            Self::Ink => Rgba8::WHITE,
        }
    }

    /// Short kebab-case label used in tracing events and CLI / WASM
    /// flag parsing. Pair with [`FrameTheme::from_label`].
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Paper => "paper",
            Self::Ink => "ink",
        }
    }

    /// Every variant in canonical declaration order. Front-ends use
    /// this to build `--help` enumerations or UI option lists without
    /// hand-listing the variants and risking drift.
    pub const ALL: &'static [Self] = &[Self::Paper, Self::Ink];
}

impl FromStr for FrameTheme {
    type Err = String;

    /// Parse the canonical kebab-case label produced by
    /// [`FrameTheme::label`]. Reverse of `label()` — the only valid
    /// inputs are the strings `label()` itself produces.
    ///
    /// # Errors
    /// Returns a human-readable `String` that lists every accepted
    /// label, suitable for surfacing through clap's `value_parser` or
    /// the WASM bridge as a [`JsError`] message.
    ///
    /// [`JsError`]: https://docs.rs/wasm-bindgen/latest/wasm_bindgen/struct.JsError.html
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for &variant in Self::ALL {
            if variant.label() == s {
                return Ok(variant);
            }
        }
        Err(unknown_label_error(
            "frame theme",
            s,
            Self::ALL.iter().map(|v| v.label()),
        ))
    }
}

/// Controls whether the metadata strip is rendered.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetaPolicy {
    /// Render the strip iff the [`crate::Provenance`] carries any
    /// caller-visible fact. With an empty Provenance, the bottom border
    /// collapses to the same thickness as the sides for a symmetric,
    /// minimal frame.
    #[default]
    Auto,
    /// Never render the strip; always produce a symmetric thin border.
    Never,
}

impl MetaPolicy {
    /// Short kebab-case label used in tracing events and CLI flag
    /// parsing. Pair with [`MetaPolicy::from_label`].
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Never => "never",
        }
    }

    /// Every variant in canonical declaration order.
    pub const ALL: &'static [Self] = &[Self::Auto, Self::Never];
}

impl FromStr for MetaPolicy {
    type Err = String;

    /// Parse the canonical kebab-case label produced by
    /// [`MetaPolicy::label`].
    ///
    /// # Errors
    /// Returns a human-readable `String` listing every accepted label.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for &variant in Self::ALL {
            if variant.label() == s {
                return Ok(variant);
            }
        }
        Err(unknown_label_error(
            "meta policy",
            s,
            Self::ALL.iter().map(|v| v.label()),
        ))
    }
}

/// Build the shared "unknown label" diagnostic so each `FromStr` impl
/// in this module produces the same shape of error message.
pub(super) fn unknown_label_error<'a, I>(kind: &str, got: &str, allowed: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    let mut joined = String::new();
    for (i, label) in allowed.into_iter().enumerate() {
        if i > 0 {
            joined.push_str(", ");
        }
        joined.push('`');
        joined.push_str(label);
        joined.push('`');
    }
    format!("unknown {kind} `{got}`: expected one of {joined}")
}

#[cfg(test)]
mod tests {
    use super::{FrameTheme, MetaPolicy, Rgba8};
    use std::str::FromStr;

    #[test]
    fn paper_theme_is_pure_white_on_pure_black() {
        assert_eq!(FrameTheme::Paper.background(), Rgba8::WHITE);
        assert_eq!(FrameTheme::Paper.ink(), Rgba8::BLACK);
    }

    #[test]
    fn ink_theme_is_pure_black_on_pure_white() {
        assert_eq!(FrameTheme::Ink.background(), Rgba8::BLACK);
        assert_eq!(FrameTheme::Ink.ink(), Rgba8::WHITE);
    }

    #[test]
    fn theme_labels_match_canonical_kebab_case() {
        assert_eq!(FrameTheme::Paper.label(), "paper");
        assert_eq!(FrameTheme::Ink.label(), "ink");
    }

    #[test]
    fn theme_all_lists_every_variant_in_declaration_order() {
        assert_eq!(FrameTheme::ALL, &[FrameTheme::Paper, FrameTheme::Ink]);
    }

    #[test]
    fn meta_policy_labels_match_canonical_kebab_case() {
        assert_eq!(MetaPolicy::Auto.label(), "auto");
        assert_eq!(MetaPolicy::Never.label(), "never");
    }

    #[test]
    fn theme_from_str_accepts_canonical_labels() {
        assert_eq!(FrameTheme::from_str("paper").unwrap(), FrameTheme::Paper);
        assert_eq!(FrameTheme::from_str("ink").unwrap(), FrameTheme::Ink);
    }

    #[test]
    fn theme_from_str_rejects_unknown_labels_with_actionable_message() {
        let err = FrameTheme::from_str("midnight").unwrap_err();
        assert!(err.contains("midnight"));
        assert!(err.contains("paper"));
        assert!(err.contains("ink"));
    }

    #[test]
    fn meta_policy_from_str_accepts_canonical_labels() {
        assert_eq!(MetaPolicy::from_str("auto").unwrap(), MetaPolicy::Auto);
        assert_eq!(MetaPolicy::from_str("never").unwrap(), MetaPolicy::Never);
    }
}
