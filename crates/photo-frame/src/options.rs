//! End-to-end pipeline configuration.
//!
//! Wraps the per-stage knobs ([`FrameOptions`] for the renderer,
//! [`JpegOptions`] for the encoder) and resolves a [`PipelineSpec`]
//! in one place so the CLI and WASM front-ends agree on what
//! `PipelineSpec::SNS` (or any other named preset) means.

use photo_frame_encode::JpegOptions;
use photo_frame_frame::FrameOptions;
use photo_frame_types::{LongEdge, PipelineSpec};

/// End-to-end options consumed by [`crate::pipeline()`].
///
/// Holds the per-stage option structs side by side so a caller resolves
/// a [`PipelineSpec`] once, then hands the bundle to every batch item.
#[derive(Clone, Debug)]
pub struct PipelineOptions {
    /// Frame-stage options (theme, caption layout, downscale cap).
    pub frame: FrameOptions,
    /// JPEG encoder options (quality 1..=100).
    pub jpeg: JpegOptions,
}

impl PipelineOptions {
    /// Build options from a [`PipelineSpec`] — the canonical entry
    /// point. Both front-ends compose a `PipelineSpec` from one of the
    /// `const` presets ([`PipelineSpec::SNS`], `STANDARD`, `MAXIMUM`)
    /// plus optional `with_*` overrides, then pass it through here.
    #[must_use]
    pub fn from_spec(spec: PipelineSpec) -> Self {
        Self {
            frame: FrameOptions {
                theme: spec.theme,
                layout: spec.layout,
                meta_policy: spec.meta_policy,
                max_long_edge: spec.max_long_edge.map(LongEdge::get),
            },
            jpeg: JpegOptions {
                quality: spec.jpeg_quality.get(),
            },
        }
    }
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self::from_spec(PipelineSpec::default())
    }
}

#[cfg(test)]
mod tests {
    use super::PipelineOptions;
    use photo_frame_types::{JpegQuality, LongEdge, PipelineSpec};

    #[test]
    fn default_resolves_to_standard_spec() {
        let opts = PipelineOptions::default();
        assert_eq!(opts.jpeg.quality, PipelineSpec::STANDARD.jpeg_quality.get());
        assert_eq!(
            opts.frame.max_long_edge,
            PipelineSpec::STANDARD.max_long_edge.map(LongEdge::get),
        );
    }

    #[test]
    fn sns_spec_caps_long_edge_at_2048() {
        let opts = PipelineOptions::from_spec(PipelineSpec::SNS);
        assert_eq!(opts.frame.max_long_edge, Some(2048));
        assert_eq!(opts.jpeg.quality, 78);
    }

    #[test]
    fn maximum_spec_keeps_full_resolution() {
        let opts = PipelineOptions::from_spec(PipelineSpec::MAXIMUM);
        assert!(opts.frame.max_long_edge.is_none());
        assert_eq!(opts.jpeg.quality, 98);
    }

    #[test]
    fn with_quality_override_flows_through_to_jpeg_options() {
        let custom = JpegQuality::new(85).expect("85 in 1..=100");
        let opts = PipelineOptions::from_spec(PipelineSpec::SNS.with_jpeg_quality(custom));
        assert_eq!(opts.jpeg.quality, 85);
        assert_eq!(opts.frame.max_long_edge, Some(2048));
    }
}
