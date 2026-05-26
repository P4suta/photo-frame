//! End-to-end pipeline specification.
//!
//! [`PipelineSpec`] is the single canonical bundle of "what a pipeline
//! run looks like" — the renderer's frame + layout choices, the
//! encoder's JPEG quality, and the optional preview downscale cap.
//!
//! Callers compose a spec one of two ways:
//!
//! 1. Pick a `const` starting point — [`PipelineSpec::SNS`],
//!    [`PipelineSpec::STANDARD`], or [`PipelineSpec::MAXIMUM`] — and
//!    chain `with_*` builders to override individual fields.
//! 2. Build the struct directly when every field is caller-supplied.
//!
//! Either way there is no multi-hop conversion: the same `PipelineSpec`
//! that the CLI parses is what the renderer and encoder ultimately read.

use std::str::FromStr;

use crate::primitives::{JpegQuality, LongEdge};
use crate::spec::layout::CaptionLayout;
use crate::spec::theme::{unknown_label_error, FrameTheme, MetaPolicy};

/// Canonical end-to-end configuration for one pipeline run.
///
/// Holds every choice the pipeline needs in one struct so callers
/// resolve `--preset` / `--quality` / `--max-long-edge` once, then
/// hand the same bundle to every batch item without intermediate
/// conversion types.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PipelineSpec {
    /// Paired frame colour + caption ink.
    pub theme: FrameTheme,
    /// How the caption strip distributes its facets.
    pub layout: CaptionLayout,
    /// Whether the metadata strip is drawn at all.
    pub meta_policy: MetaPolicy,
    /// JPEG quality the encoder uses (`1..=100`).
    pub jpeg_quality: JpegQuality,
    /// If set, downscale the photo so its longer edge is at most this
    /// many pixels before framing. `None` keeps the source resolution.
    pub max_long_edge: Option<LongEdge>,
}

impl PipelineSpec {
    /// Optimised for posting to social media: small file, modest
    /// quality, long edge clamped to 2048 px so platforms do not
    /// aggressively re-compress the upload.
    pub const SNS: Self = Self {
        theme: FrameTheme::Paper,
        layout: CaptionLayout::Edges,
        meta_policy: MetaPolicy::Auto,
        // SAFETY of `expect`: 78 is in `1..=100` and `2048 > 0`; both
        // newtype constructors return `Some`. `expect` panics only if
        // either fact were ever to change, which would deserve a
        // compile-time visible loud failure.
        jpeg_quality: match JpegQuality::new(78) {
            Some(q) => q,
            None => panic!("JPEG quality 78 is in range"),
        },
        max_long_edge: Some(match LongEdge::new(2048) {
            Some(e) => e,
            None => panic!("2048 px is non-zero"),
        }),
    };

    /// Balanced default. Visually transparent at the downsample-and-zoom
    /// sizes most viewers use. No downscale.
    pub const STANDARD: Self = Self {
        theme: FrameTheme::Paper,
        layout: CaptionLayout::Edges,
        meta_policy: MetaPolicy::Auto,
        jpeg_quality: match JpegQuality::new(92) {
            Some(q) => q,
            None => panic!("JPEG quality 92 is in range"),
        },
        max_long_edge: None,
    };

    /// Print / archive grade. Highest quality JPEG, no downscale.
    pub const MAXIMUM: Self = Self {
        theme: FrameTheme::Paper,
        layout: CaptionLayout::Edges,
        meta_policy: MetaPolicy::Auto,
        jpeg_quality: match JpegQuality::new(98) {
            Some(q) => q,
            None => panic!("JPEG quality 98 is in range"),
        },
        max_long_edge: None,
    };

    /// Builder: override [`Self::theme`].
    #[must_use]
    pub const fn with_theme(mut self, theme: FrameTheme) -> Self {
        self.theme = theme;
        self
    }

    /// Builder: override [`Self::layout`].
    #[must_use]
    pub const fn with_layout(mut self, layout: CaptionLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Builder: override [`Self::meta_policy`].
    #[must_use]
    pub const fn with_meta_policy(mut self, policy: MetaPolicy) -> Self {
        self.meta_policy = policy;
        self
    }

    /// Builder: override [`Self::jpeg_quality`].
    #[must_use]
    pub const fn with_jpeg_quality(mut self, quality: JpegQuality) -> Self {
        self.jpeg_quality = quality;
        self
    }

    /// Builder: override [`Self::max_long_edge`].
    #[must_use]
    pub const fn with_max_long_edge(mut self, edge: Option<LongEdge>) -> Self {
        self.max_long_edge = edge;
        self
    }

    /// Named presets, paired with their canonical labels. Front-ends
    /// (CLI, WASM) iterate this slice both to build `--help`-style
    /// option lists and to resolve a user-supplied label, so the
    /// label vocabulary stays in exactly one place.
    pub const PRESETS: &'static [(&'static str, Self)] = &[
        ("sns", Self::SNS),
        ("standard", Self::STANDARD),
        ("maximum", Self::MAXIMUM),
    ];
}

impl FromStr for PipelineSpec {
    type Err = String;

    /// Resolve a named preset by its canonical label
    /// (`"sns"` / `"standard"` / `"maximum"`).
    ///
    /// # Errors
    /// Returns a human-readable `String` listing every accepted
    /// preset label.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for (label, spec) in Self::PRESETS {
            if *label == s {
                return Ok(*spec);
            }
        }
        Err(unknown_label_error(
            "preset",
            s,
            Self::PRESETS.iter().map(|(l, _)| *l),
        ))
    }
}

impl Default for PipelineSpec {
    /// Default resolves to [`Self::STANDARD`] — the "no flags passed"
    /// behaviour both front-ends agree on.
    fn default() -> Self {
        Self::STANDARD
    }
}

#[cfg(test)]
mod tests {
    use super::PipelineSpec;
    use crate::primitives::{JpegQuality, LongEdge};

    #[test]
    fn sns_preset_pins_jpeg_quality_and_long_edge() {
        let spec = PipelineSpec::SNS;
        assert_eq!(spec.jpeg_quality.get(), 78);
        assert_eq!(spec.max_long_edge.map(LongEdge::get), Some(2048));
    }

    #[test]
    fn standard_preset_keeps_full_resolution() {
        let spec = PipelineSpec::STANDARD;
        assert_eq!(spec.jpeg_quality.get(), 92);
        assert!(spec.max_long_edge.is_none());
    }

    #[test]
    fn maximum_preset_keeps_full_resolution() {
        let spec = PipelineSpec::MAXIMUM;
        assert_eq!(spec.jpeg_quality.get(), 98);
        assert!(spec.max_long_edge.is_none());
    }

    #[test]
    fn default_resolves_to_standard() {
        assert_eq!(PipelineSpec::default(), PipelineSpec::STANDARD);
    }

    #[test]
    fn with_quality_overrides_existing_value() {
        let spec = PipelineSpec::SNS.with_jpeg_quality(JpegQuality::new(85).unwrap());
        assert_eq!(spec.jpeg_quality.get(), 85);
        // Other fields preserved.
        assert_eq!(spec.max_long_edge.map(LongEdge::get), Some(2048));
    }

    #[test]
    fn with_max_long_edge_clears_with_none() {
        let spec = PipelineSpec::SNS.with_max_long_edge(None);
        assert!(spec.max_long_edge.is_none());
        // Other fields preserved.
        assert_eq!(spec.jpeg_quality.get(), 78);
    }

    #[test]
    fn from_str_resolves_each_named_preset() {
        use std::str::FromStr;
        assert_eq!(PipelineSpec::from_str("sns").unwrap(), PipelineSpec::SNS);
        assert_eq!(
            PipelineSpec::from_str("standard").unwrap(),
            PipelineSpec::STANDARD,
        );
        assert_eq!(
            PipelineSpec::from_str("maximum").unwrap(),
            PipelineSpec::MAXIMUM,
        );
    }

    #[test]
    fn from_str_rejects_unknown_labels_with_actionable_message() {
        use std::str::FromStr;
        let err = PipelineSpec::from_str("fancy").unwrap_err();
        assert!(err.contains("fancy"));
        assert!(err.contains("sns"));
        assert!(err.contains("standard"));
        assert!(err.contains("maximum"));
    }

    #[test]
    fn presets_table_covers_every_const() {
        // The PRESETS slice is the single source of truth for CLI /
        // WASM label parsing — if a new const preset is added without
        // a row here, parsing won't find it and the help text won't
        // list it. Pin the relationship explicitly.
        let labels: Vec<&str> = PipelineSpec::PRESETS.iter().map(|(l, _)| *l).collect();
        assert_eq!(labels, vec!["sns", "standard", "maximum"]);
        for (_, spec) in PipelineSpec::PRESETS {
            // Each preset's jpeg_quality is at least the JPEG minimum
            // (the const constructor would have panicked otherwise);
            // pin it again here as a smoke check.
            assert!(spec.jpeg_quality.get() >= 1);
        }
    }
}
