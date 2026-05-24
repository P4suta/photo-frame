//! End-to-end pipeline configuration.
//!
//! Wraps the per-stage knobs (`FrameOptions` for the renderer, `JpegOptions`
//! for the encoder) and resolves [`QualityPreset`] in one place so the CLI
//! and WASM front-ends agree on what `--preset sns` means.

use photo_frame_encode::JpegOptions;
use photo_frame_frame::FrameOptions;
use photo_frame_types::QualityPreset;

#[derive(Clone, Debug)]
pub struct PipelineOptions {
    pub frame: FrameOptions,
    pub jpeg: JpegOptions,
}

impl PipelineOptions {
    /// Build options from a [`QualityPreset`]. Caller-side overrides
    /// (e.g. `--quality 80`) should be applied to the returned struct
    /// after `from_preset`.
    #[must_use]
    pub fn from_preset(preset: QualityPreset) -> Self {
        Self {
            frame: FrameOptions {
                max_long_edge: preset.max_long_edge(),
                ..FrameOptions::default()
            },
            jpeg: JpegOptions {
                quality: preset.jpeg_quality(),
            },
        }
    }
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self::from_preset(QualityPreset::default())
    }
}

#[cfg(test)]
mod tests {
    use super::PipelineOptions;
    use photo_frame_types::QualityPreset;

    #[test]
    fn default_resolves_to_standard_preset() {
        let opts = PipelineOptions::default();
        assert_eq!(opts.jpeg.quality, QualityPreset::Standard.jpeg_quality());
        assert_eq!(
            opts.frame.max_long_edge,
            QualityPreset::Standard.max_long_edge()
        );
    }

    #[test]
    fn sns_preset_caps_long_edge_at_2048() {
        let opts = PipelineOptions::from_preset(QualityPreset::Sns);
        assert_eq!(opts.frame.max_long_edge, Some(2048));
        assert_eq!(opts.jpeg.quality, 78);
    }

    #[test]
    fn maximum_preset_keeps_full_resolution() {
        let opts = PipelineOptions::from_preset(QualityPreset::Maximum);
        assert!(opts.frame.max_long_edge.is_none());
        assert_eq!(opts.jpeg.quality, 98);
    }
}
