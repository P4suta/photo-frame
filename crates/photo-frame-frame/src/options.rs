//! Caller-facing options for [`crate::render()`].
//!
//! The renderer's per-image knobs ([`FrameTheme`], [`CaptionLayout`],
//! [`MetaPolicy`]) live in `photo-frame-types::spec` so the CLI, WASM
//! bridge, and renderer all speak the same vocabulary at the same
//! address. This module wraps them into [`FrameOptions`] — the struct
//! [`crate::render()`] actually reads — and provides the conversion
//! helper for turning the typed [`Rgba8`] colour into `image::Rgba`,
//! which is the renderer's only `image`-crate-flavoured leak.

use image::Rgba;
use photo_frame_types::Rgba8;

pub use photo_frame_types::{CaptionLayout, FrameStyle, FrameTheme, MetaPolicy};

/// Configuration for [`crate::render()`].
#[derive(Clone, Debug, Default)]
pub struct FrameOptions {
    /// Outer canvas silhouette. See [`FrameStyle`].
    pub frame_style: FrameStyle,
    /// Paired frame colour + caption ink. See [`FrameTheme`].
    pub theme: FrameTheme,
    /// How the caption text is arranged inside the standard-style
    /// frame. Ignored when [`Self::frame_style`] is
    /// [`FrameStyle::Polaroid`] (Polaroid always centres its caption).
    /// See [`CaptionLayout`].
    pub layout: CaptionLayout,
    /// Whether the metadata strip is drawn at all. See [`MetaPolicy`].
    pub meta_policy: MetaPolicy,
    /// If set, downscale the photo so its longer edge is at most this many
    /// pixels before framing. Intended for browser previews and the SNS
    /// preset; release builds pass `None` for full-resolution output.
    pub max_long_edge: Option<u32>,
}

/// Pack the typed [`Rgba8`] into `image::Rgba<u8>` — the renderer's
/// drawing routines (`draw_text_mut`, `RgbaImage`) speak the latter.
#[must_use]
pub(crate) const fn to_image_rgba(color: Rgba8) -> Rgba<u8> {
    Rgba(color.to_array())
}

#[cfg(test)]
mod tests {
    use super::{to_image_rgba, CaptionLayout, FrameOptions, FrameStyle, FrameTheme, MetaPolicy};
    use image::Rgba;
    use photo_frame_types::Rgba8;

    #[test]
    fn default_options_are_standard_paper_edges_auto_meta_no_downscale() {
        let opts = FrameOptions::default();
        assert_eq!(opts.frame_style, FrameStyle::Standard);
        assert_eq!(opts.theme, FrameTheme::Paper);
        assert_eq!(opts.layout, CaptionLayout::Edges);
        assert_eq!(opts.meta_policy, MetaPolicy::Auto);
        assert!(opts.max_long_edge.is_none());
    }

    #[test]
    fn to_image_rgba_preserves_channels() {
        assert_eq!(to_image_rgba(Rgba8::WHITE), Rgba([255, 255, 255, 255]));
        assert_eq!(to_image_rgba(Rgba8::BLACK), Rgba([0, 0, 0, 255]));
        assert_eq!(
            to_image_rgba(Rgba8::new(12, 34, 56, 78)),
            Rgba([12, 34, 56, 78]),
        );
    }
}
