//! Caller-facing options for [`crate::render`].
//!
//! Quality/encoding knobs (`jpeg_quality`) live in `photo-frame-encode`;
//! this struct holds only what the renderer itself consumes — background
//! colour, whether to draw the caption strip, and the optional preview
//! downscale cap.

/// Solid RGB colour used to fill the frame background.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Background([u8; 3]);

impl Background {
    /// Paper-white #FFFFFF.
    pub const WHITE: Self = Self([255, 255, 255]);

    #[must_use]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b])
    }

    #[must_use]
    pub const fn rgb(self) -> [u8; 3] {
        self.0
    }
}

impl Default for Background {
    fn default() -> Self {
        Self::WHITE
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
#[derive(Clone, Debug)]
pub struct FrameOptions {
    pub background: Background,
    pub meta_policy: MetaPolicy,
    /// If set, downscale the photo so its longer edge is at most this many
    /// pixels before framing. Intended for browser previews and the SNS
    /// preset; release builds pass `None` for full-resolution output.
    pub max_long_edge: Option<u32>,
}

impl Default for FrameOptions {
    fn default() -> Self {
        Self {
            background: Background::WHITE,
            meta_policy: MetaPolicy::Auto,
            max_long_edge: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Background, FrameOptions, MetaPolicy};

    #[test]
    fn default_options_are_paper_white_auto_meta_no_downscale() {
        let opts = FrameOptions::default();
        assert_eq!(opts.background, Background::WHITE);
        assert_eq!(opts.meta_policy, MetaPolicy::Auto);
        assert!(opts.max_long_edge.is_none());
    }

    #[test]
    fn background_is_a_const_friendly_value_type() {
        const _BG: Background = Background::from_rgb(10, 20, 30);
        assert_eq!(Background::from_rgb(10, 20, 30).rgb(), [10, 20, 30]);
    }
}
