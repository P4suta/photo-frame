//! Caller-facing options for [`crate::render`].
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
/// frame with dark text" vs. "black frame with light text" is one
/// decision, not two — so we expose them as one enum rather than
/// independent fields.
///
/// The colour values aren't pure 0/255 endpoints: pure white can wash
/// out against bright photos and pure black sits noticeably "above" a
/// dark photo. The current values were picked by eye to sit flush
/// against typical photographic content.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum FrameTheme {
    /// White frame (#FFFFFF), dark ink (#3C3C3C). The original v1
    /// look — appropriate for most prints and SNS uploads.
    #[default]
    Paper,
    /// Soft-black frame (#1A1A1A), soft-white ink (#E8E8E8). Reads
    /// well for photos that already feature a lot of light (snow,
    /// sky, paper subjects); the frame doesn't compete.
    Ink,
}

impl FrameTheme {
    /// RGBA8 fill colour for the frame border.
    #[must_use]
    pub const fn background(self) -> Rgba<u8> {
        match self {
            Self::Paper => Rgba([255, 255, 255, 255]),
            Self::Ink => Rgba([0x1A, 0x1A, 0x1A, 255]),
        }
    }

    /// RGBA8 colour for caption text.
    #[must_use]
    pub const fn ink(self) -> Rgba<u8> {
        match self {
            Self::Paper => Rgba([60, 60, 60, 255]),
            Self::Ink => Rgba([0xE8, 0xE8, 0xE8, 255]),
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

/// How the caption strip arranges its facets.
///
/// `Edges` keeps the original liit-style four-corner layout (camera
/// / lens on the top row, exposure / date on the bottom). `Centered`
/// joins each row with a `"  ·  "` separator and centres it under the
/// photo — useful when the photo's subject sits central and a
/// symmetric caption looks more deliberate.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum CaptionLayout {
    /// Camera/lens at the top edges, exposure/date at the bottom
    /// edges. The historical v1 + v2 default.
    #[default]
    Edges,
    /// Both rows centred under the photo, with the same `"  ·  "`
    /// separator used inside the exposure line.
    Centered,
}

impl CaptionLayout {
    /// Short kebab-case label used in tracing events + CLI/WASM parsing.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Edges => "edges",
            Self::Centered => "centered",
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

/// Configuration for [`crate::render`].
#[derive(Clone, Debug, Default)]
pub struct FrameOptions {
    pub theme: FrameTheme,
    pub layout: CaptionLayout,
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
    }

    #[test]
    fn paper_theme_locked_colours() {
        assert_eq!(FrameTheme::Paper.background(), Rgba([255, 255, 255, 255]));
        assert_eq!(FrameTheme::Paper.ink(), Rgba([60, 60, 60, 255]));
    }

    #[test]
    fn ink_theme_locked_colours() {
        assert_eq!(FrameTheme::Ink.background(), Rgba([0x1A, 0x1A, 0x1A, 255]));
        assert_eq!(FrameTheme::Ink.ink(), Rgba([0xE8, 0xE8, 0xE8, 255]));
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
