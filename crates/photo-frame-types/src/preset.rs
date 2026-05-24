/// Named quality / size bundle. Used by the facade crate to populate a
/// full `PipelineOptions` from a single user-facing word.
///
/// Numeric values are part of the public contract — locked-table tests
/// in this crate pin them, and any change must bump the major version
/// and ripple to CLI help / WASM UI labels.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum QualityPreset {
    /// Optimised for posting to social media: small file, modest quality,
    /// long edge clamped to 2048 px so platforms don't aggressively
    /// re-compress the upload.
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

    /// Longer-edge pixel cap, or `None` for "do not downscale".
    #[must_use]
    pub const fn max_long_edge(self) -> Option<u32> {
        match self {
            Self::Sns => Some(2048),
            Self::Standard | Self::Maximum => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::QualityPreset;

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
    fn default_is_standard() {
        assert_eq!(QualityPreset::default(), QualityPreset::Standard);
    }
}
