//! Caller-facing options for [`crate::render()`].
//!
//! Quality/encoding knobs (`jpeg_quality`) live in `photo-frame-encode`;
//! this struct holds only what the renderer itself consumes — the
//! [`FrameTheme`] (frame colour + ink colour paired as one preset),
//! whether to draw the caption strip, and the optional preview
//! downscale cap.

use image::Rgba;

/// Paired frame colour + ink colour preset.
///
/// Project policy is that these two values *travel together* — a "white
/// frame with black text" vs. "black frame with white text" is one
/// decision, not two — so we expose them as one enum rather than
/// independent fields.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum FrameTheme {
    /// White frame, black ink.
    #[default]
    Paper,
    /// Black frame, white ink.
    Ink,
}

impl FrameTheme {
    /// RGBA8 fill colour for the frame border.
    #[must_use]
    pub const fn background(self) -> Rgba<u8> {
        match self {
            Self::Paper => Rgba([255, 255, 255, 255]),
            Self::Ink => Rgba([0, 0, 0, 255]),
        }
    }

    /// RGBA8 colour for caption text.
    #[must_use]
    pub const fn ink(self) -> Rgba<u8> {
        match self {
            Self::Paper => Rgba([0, 0, 0, 255]),
            Self::Ink => Rgba([255, 255, 255, 255]),
        }
    }

    /// Short kebab-case label used in tracing events + CLI flag parsing.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Paper => "paper",
            Self::Ink => "ink",
        }
    }
}

/// How the caption is arranged inside the framed print.
///
/// The first two variants share the same standard frame geometry
/// (photo centred in a uniform-mat canvas, strip below the photo);
/// they differ only in horizontal composition of the caption. The
/// third variant switches to a Polaroid-style geometry: photo
/// top-anchored, large bottom band with caption centred inside.
///
/// - `Edges` keeps the four-corner layout — camera left, lens right
///   on the primary row; exposure left, date right on the secondary
///   row. Left- and right-aligned text snap to the photo's left and
///   right edges, so caption and photo share a single visual column.
/// - `Centered` joins each row with a `"  ·  "` separator and centres
///   the result horizontally inside the strip.
/// - `Polaroid` selects the Polaroid frame geometry (photo at top,
///   thick bottom band) and centres the caption inside the band.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
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
    /// Short kebab-case label used in tracing events + CLI/WASM parsing.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Edges => "edges",
            Self::Centered => "centered",
            Self::Polaroid => "polaroid",
        }
    }
}

/// Controls whether the metadata strip is rendered.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum MetaPolicy {
    /// Render the strip iff the [`Provenance`] carries any caller-visible
    /// fact. With an empty Provenance, the bottom border collapses to
    /// the same thickness as the sides for a symmetric, minimal frame.
    ///
    /// [`Provenance`]: photo_frame_types::Provenance
    #[default]
    Auto,
    /// Never render the strip; always produce a symmetric thin border.
    Never,
}

/// Configuration for [`crate::render()`].
#[derive(Clone, Debug, Default)]
pub struct FrameOptions {
    /// Paired frame colour + caption ink. See [`FrameTheme`].
    pub theme: FrameTheme,
    /// How the caption strip distributes its facets. See [`CaptionLayout`].
    pub layout: CaptionLayout,
    /// Whether the metadata strip is drawn at all. See [`MetaPolicy`].
    pub meta_policy: MetaPolicy,
    /// If set, downscale the photo so its longer edge is at most this many
    /// pixels before framing. Intended for browser previews and the SNS
    /// preset; release builds pass `None` for full-resolution output.
    pub max_long_edge: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::{CaptionLayout, FrameOptions, FrameTheme, MetaPolicy};
    use image::Rgba;

    #[test]
    fn default_options_are_paper_edges_auto_meta_no_downscale() {
        let opts = FrameOptions::default();
        assert_eq!(opts.theme, FrameTheme::Paper);
        assert_eq!(opts.layout, CaptionLayout::Edges);
        assert_eq!(opts.meta_policy, MetaPolicy::Auto);
        assert!(opts.max_long_edge.is_none());
    }

    #[test]
    fn layout_labels_are_short_lowercase() {
        assert_eq!(CaptionLayout::Edges.label(), "edges");
        assert_eq!(CaptionLayout::Centered.label(), "centered");
        assert_eq!(CaptionLayout::Polaroid.label(), "polaroid");
    }

    #[test]
    fn paper_theme_is_pure_white_on_pure_black() {
        assert_eq!(FrameTheme::Paper.background(), Rgba([255, 255, 255, 255]));
        assert_eq!(FrameTheme::Paper.ink(), Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn ink_theme_is_pure_black_on_pure_white() {
        assert_eq!(FrameTheme::Ink.background(), Rgba([0, 0, 0, 255]));
        assert_eq!(FrameTheme::Ink.ink(), Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn theme_labels_are_short_lowercase() {
        assert_eq!(FrameTheme::Paper.label(), "paper");
        assert_eq!(FrameTheme::Ink.label(), "ink");
    }

    #[test]
    fn theme_is_const_friendly() {
        const _PAPER_BG: Rgba<u8> = FrameTheme::Paper.background();
        const _INK_INK: Rgba<u8> = FrameTheme::Ink.ink();
    }
}
