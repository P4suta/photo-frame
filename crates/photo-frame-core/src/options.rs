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

/// Named quality / size preset.
///
/// Bundles the two knobs (`jpeg_quality` and `max_long_edge`) that move
/// together for a given intent, so callers pick one word instead of
/// dialling two sliders. Always overridable: setting `jpeg_quality` or
/// `max_long_edge` explicitly after `from_preset` wins.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum QualityPreset {
    /// Optimised for posting to social media: small file, modest quality,
    /// long edge clamped to 2048 px so platforms like Instagram and X
    /// don't re-compress the upload aggressively.
    Sns,
    /// Balanced default. Visually transparent at the downsample-and-zoom
    /// sizes most viewers use. No downscale.
    #[default]
    Standard,
    /// Print / archive grade. Highest quality JPEG, no downscale.
    Maximum,
}

impl QualityPreset {
    /// JPEG quality (1..=100) this preset prescribes.
    #[must_use]
    pub const fn jpeg_quality(self) -> u8 {
        match self {
            Self::Sns => 78,
            Self::Standard => 92,
            Self::Maximum => 98,
        }
    }

    /// Longer-edge pixel cap this preset prescribes, or `None` for "do not
    /// downscale".
    #[must_use]
    pub const fn max_long_edge(self) -> Option<u32> {
        match self {
            Self::Sns => Some(2048),
            Self::Standard | Self::Maximum => None,
        }
    }
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
    /// pixels before framing. Intended for browser previews and the SNS
    /// preset; release builds pass `None` for full-resolution output.
    pub max_long_edge: Option<u32>,
}

impl FrameOptions {
    /// Build [`FrameOptions`] from a [`QualityPreset`]. Background and
    /// metadata policy stay at their defaults; override after the fact if
    /// you need them.
    #[must_use]
    pub const fn from_preset(preset: QualityPreset) -> Self {
        Self {
            jpeg_quality: preset.jpeg_quality(),
            background: Background::WHITE,
            meta_policy: MetaPolicy::Auto,
            max_long_edge: preset.max_long_edge(),
        }
    }
}

impl Default for FrameOptions {
    fn default() -> Self {
        Self::from_preset(QualityPreset::Standard)
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameOptions, QualityPreset};

    /// Locked-table test: preset numerics are part of the public contract.
    /// If you change them, you must update every doc / UI / CLI surface
    /// that exposes a preset name.
    #[test]
    fn preset_table_is_stable() {
        assert_eq!(QualityPreset::Sns.jpeg_quality(), 78);
        assert_eq!(QualityPreset::Sns.max_long_edge(), Some(2048));

        assert_eq!(QualityPreset::Standard.jpeg_quality(), 92);
        assert_eq!(QualityPreset::Standard.max_long_edge(), None);

        assert_eq!(QualityPreset::Maximum.jpeg_quality(), 98);
        assert_eq!(QualityPreset::Maximum.max_long_edge(), None);
    }

    #[test]
    fn default_preset_is_standard() {
        assert_eq!(QualityPreset::default(), QualityPreset::Standard);
    }

    #[test]
    fn default_options_match_standard_preset() {
        let opts = FrameOptions::default();
        let standard = FrameOptions::from_preset(QualityPreset::Standard);
        assert_eq!(opts.jpeg_quality, standard.jpeg_quality);
        assert_eq!(opts.max_long_edge, standard.max_long_edge);
    }

    #[test]
    fn from_preset_is_a_const_fn() {
        // Compile-time proof: callers can place preset-derived options in
        // a `const` context (e.g. CLI default).
        const _OPTS: FrameOptions = FrameOptions::from_preset(QualityPreset::Sns);
    }
}
