//! Public configuration values for the framing pipeline.

/// Solid RGB color used to fill the frame background.
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
    /// Render the strip iff EXIF metadata is present in the input. With no
    /// EXIF, the bottom border collapses to the same thickness as the sides
    /// for a symmetric, minimal frame.
    #[default]
    Auto,
    /// Never render the strip; always produce a symmetric thin border.
    Never,
}

/// Configuration for [`crate::frame_image`].
#[derive(Clone, Debug)]
pub struct FrameOptions {
    /// JPEG quality, 1..=100. Default 92 — visually transparent at the
    /// downsample-and-zoom sizes most viewers use.
    pub jpeg_quality: u8,
    /// Frame fill color.
    pub background: Background,
    /// Metadata strip behavior.
    pub meta_policy: MetaPolicy,
    /// If set, downscale the image so its longer edge is at most this many
    /// pixels before framing. Intended for browser previews; release builds
    /// pass `None` for full-resolution output.
    pub max_long_edge: Option<u32>,
}

impl Default for FrameOptions {
    fn default() -> Self {
        Self {
            jpeg_quality: 92,
            background: Background::default(),
            meta_policy: MetaPolicy::default(),
            max_long_edge: None,
        }
    }
}
