//! Caption layout — how caption text is arranged inside the
//! standard-style frame.
//!
//! Independent of [`crate::FrameStyle`] (the canvas silhouette).
//! `Polaroid` lives over there: a Polaroid frame always centres its
//! caption, so this enum stays a pure "Edges vs Centered" choice
//! that only the standard silhouette consumes.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use tsify::Tsify;

use crate::spec::theme::unknown_label_error;

/// How the caption text is arranged inside the standard-style frame.
///
/// Both variants share the same standard frame geometry (photo
/// centred in a uniform-mat canvas, strip below the photo); they
/// differ only in the horizontal composition of the caption.
///
/// - `Edges` keeps the four-corner layout: camera left, lens right on
///   the primary row; exposure left, date right on the secondary row.
///   Left- and right-aligned text snap to the photo's left and right
///   edges so caption and photo share a single visual column.
/// - `Centered` joins each row with a `"  ·  "` separator and centres
///   the result horizontally inside the strip.
///
/// `Polaroid` previously lived here too; it has moved to
/// [`crate::FrameStyle::Polaroid`] because Polaroid changes the
/// canvas silhouette, not just the caption arrangement.
#[derive(Tsify, Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
#[serde(rename_all = "lowercase")]
pub enum CaptionLayout {
    /// Camera/lens at the primary-row corners, exposure/date at the
    /// secondary-row corners; both rows anchored to the photo's left
    /// and right edges so caption text shares a column with the photo.
    #[default]
    Edges,
    /// Both rows centred under the photo, with the same `"  ·  "`
    /// separator used inside the exposure line.
    Centered,
}

impl CaptionLayout {
    /// Short kebab-case label used in tracing events and CLI / WASM
    /// flag parsing. Pair with the `FromStr` impl for the inverse.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Edges => "edges",
            Self::Centered => "centered",
        }
    }

    /// Every variant in canonical declaration order.
    pub const ALL: &'static [Self] = &[Self::Edges, Self::Centered];
}

impl FromStr for CaptionLayout {
    type Err = String;

    /// Parse the canonical kebab-case label produced by
    /// [`CaptionLayout::label`].
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
            "caption layout",
            s,
            Self::ALL.iter().map(|v| v.label()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::CaptionLayout;
    use std::str::FromStr;

    #[test]
    fn layout_labels_match_canonical_kebab_case() {
        assert_eq!(CaptionLayout::Edges.label(), "edges");
        assert_eq!(CaptionLayout::Centered.label(), "centered");
    }

    #[test]
    fn layout_all_lists_every_variant_in_declaration_order() {
        assert_eq!(
            CaptionLayout::ALL,
            &[CaptionLayout::Edges, CaptionLayout::Centered],
        );
    }

    #[test]
    fn layout_from_str_accepts_canonical_labels() {
        assert_eq!(
            CaptionLayout::from_str("edges").unwrap(),
            CaptionLayout::Edges,
        );
        assert_eq!(
            CaptionLayout::from_str("centered").unwrap(),
            CaptionLayout::Centered,
        );
    }

    #[test]
    fn layout_from_str_rejects_unknown_labels_with_actionable_message() {
        let err = CaptionLayout::from_str("polaroid").unwrap_err();
        // Polaroid is no longer a caption layout — surface it explicitly
        // so anyone passing the old label gets pointed at `FrameStyle`.
        assert!(err.contains("polaroid"));
        assert!(err.contains("edges"));
        assert!(err.contains("centered"));
    }
}
