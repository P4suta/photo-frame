//! Caption layout — how the caption strip arranges its lines around
//! the photo inside the framed canvas.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::spec::theme::unknown_label_error;

/// How the caption is arranged inside the framed print.
///
/// The first two variants share the same standard frame geometry
/// (photo centred in a uniform-mat canvas, strip below the photo);
/// they differ only in the horizontal composition of the caption.
/// The third variant switches to a Polaroid-style geometry — photo
/// top-anchored, large bottom band with caption centred inside.
///
/// - `Edges` keeps the four-corner layout: camera left, lens right on
///   the primary row; exposure left, date right on the secondary row.
///   Left- and right-aligned text snap to the photo's left and right
///   edges so caption and photo share a single visual column.
/// - `Centered` joins each row with a `"  ·  "` separator and centres
///   the result horizontally inside the strip.
/// - `Polaroid` selects the Polaroid frame geometry (photo at top,
///   thick bottom band) and centres the caption inside the band.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// Polaroid-style frame: photo at top, thick bottom band carries
    /// both caption rows centred horizontally inside the band.
    Polaroid,
}

impl CaptionLayout {
    /// Short kebab-case label used in tracing events and CLI / WASM
    /// flag parsing. Pair with [`CaptionLayout::from_label`].
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Edges => "edges",
            Self::Centered => "centered",
            Self::Polaroid => "polaroid",
        }
    }

    /// Every variant in canonical declaration order.
    pub const ALL: &'static [Self] = &[Self::Edges, Self::Centered, Self::Polaroid];
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
        assert_eq!(CaptionLayout::Polaroid.label(), "polaroid");
    }

    #[test]
    fn layout_all_lists_every_variant_in_declaration_order() {
        assert_eq!(
            CaptionLayout::ALL,
            &[
                CaptionLayout::Edges,
                CaptionLayout::Centered,
                CaptionLayout::Polaroid,
            ],
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
        assert_eq!(
            CaptionLayout::from_str("polaroid").unwrap(),
            CaptionLayout::Polaroid,
        );
    }

    #[test]
    fn layout_from_str_rejects_unknown_labels_with_actionable_message() {
        let err = CaptionLayout::from_str("stacked").unwrap_err();
        assert!(err.contains("stacked"));
        assert!(err.contains("edges"));
        assert!(err.contains("centered"));
        assert!(err.contains("polaroid"));
    }
}
