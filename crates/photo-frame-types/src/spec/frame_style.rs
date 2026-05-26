//! Frame style — the silhouette of the framed canvas.
//!
//! Independent of [`crate::CaptionLayout`] (which only describes how
//! caption text is arranged inside a frame). A Polaroid silhouette
//! changes the canvas geometry (photo top-anchored, thick bottom band)
//! regardless of what the caption picker says; conversely, the
//! Standard silhouette is shared by the two caption-arrangement
//! variants `Edges` and `Centered`.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use tsify::Tsify;

use crate::spec::theme::unknown_label_error;

/// Outer silhouette of the framed canvas. Picks the geometry the
/// renderer composes; the actual caption text arrangement (when one
/// is drawn) is controlled by [`crate::CaptionLayout`].
///
/// - `Standard` centres the photo in a uniform-mat canvas with the
///   caption strip below. Both `Edges` and `Centered` caption layouts
///   live inside this silhouette.
/// - `Polaroid` switches to the Polaroid-style frame: photo
///   top-anchored, large bottom band absorbing the caption. The
///   caption text inside is always centred horizontally; the
///   [`crate::CaptionLayout`] picker has no effect here.
#[derive(Tsify, Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
#[serde(rename_all = "lowercase")]
pub enum FrameStyle {
    /// Uniform-mat frame; caption strip sits below the photo.
    #[default]
    Standard,
    /// Polaroid silhouette; thick bottom band carries the caption.
    Polaroid,
}

impl FrameStyle {
    /// Short kebab-case label used in tracing events and CLI / WASM
    /// flag parsing. Pair with the `FromStr` impl for the inverse.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Polaroid => "polaroid",
        }
    }

    /// Every variant in canonical declaration order.
    pub const ALL: &'static [Self] = &[Self::Standard, Self::Polaroid];
}

impl FromStr for FrameStyle {
    type Err = String;

    /// Parse the canonical kebab-case label produced by
    /// [`FrameStyle::label`].
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
            "frame style",
            s,
            Self::ALL.iter().map(|v| v.label()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::FrameStyle;
    use std::str::FromStr;

    #[test]
    fn labels_match_canonical_kebab_case() {
        assert_eq!(FrameStyle::Standard.label(), "standard");
        assert_eq!(FrameStyle::Polaroid.label(), "polaroid");
    }

    #[test]
    fn all_lists_every_variant_in_declaration_order() {
        assert_eq!(
            FrameStyle::ALL,
            &[FrameStyle::Standard, FrameStyle::Polaroid]
        );
    }

    #[test]
    fn default_is_standard() {
        assert_eq!(FrameStyle::default(), FrameStyle::Standard);
    }

    #[test]
    fn from_str_accepts_canonical_labels() {
        assert_eq!(
            FrameStyle::from_str("standard").unwrap(),
            FrameStyle::Standard
        );
        assert_eq!(
            FrameStyle::from_str("polaroid").unwrap(),
            FrameStyle::Polaroid
        );
    }

    #[test]
    fn from_str_rejects_unknown_labels_with_actionable_message() {
        let err = FrameStyle::from_str("baroque").unwrap_err();
        assert!(err.contains("baroque"));
        assert!(err.contains("standard"));
        assert!(err.contains("polaroid"));
    }

    #[test]
    fn serializes_as_lowercase_string() {
        assert_eq!(
            serde_json::to_string(&FrameStyle::Standard).unwrap(),
            "\"standard\"",
        );
        assert_eq!(
            serde_json::to_string(&FrameStyle::Polaroid).unwrap(),
            "\"polaroid\"",
        );
    }
}
