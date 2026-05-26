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

use serde::{Deserialize, Serialize};
use tsify::Tsify;

use crate::primitives::{JpegQuality, LongEdge};
use crate::spec::frame_style::FrameStyle;
use crate::spec::layout::CaptionLayout;
use crate::spec::theme::{unknown_label_error, FrameTheme, MetaPolicy};

/// Canonical end-to-end configuration for one pipeline run.
///
/// Holds every choice the pipeline needs in one struct so callers
/// resolve `--preset` / `--quality` / `--max-long-edge` once, then
/// hand the same bundle to every batch item without intermediate
/// conversion types.
///
/// ## Wire shape
///
/// `serde` emits / accepts a flat object with `snake_case` keys
/// (`frame_style`, `theme`, `layout`, `meta_policy`, `jpeg_quality`,
/// `max_long_edge`).
/// `tsify` exports the same shape as a typed TypeScript
/// `interface PipelineSpec` to JS consumers — both directions
/// (`into_wasm_abi` for return values, `from_wasm_abi` for arguments).
#[derive(Tsify, Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct PipelineSpec {
    /// Outer canvas silhouette. See [`FrameStyle`].
    pub frame_style: FrameStyle,
    /// Paired frame colour + caption ink.
    pub theme: FrameTheme,
    /// How the caption text is arranged inside the standard-style
    /// frame. Ignored when [`Self::frame_style`] is
    /// [`FrameStyle::Polaroid`] (a Polaroid silhouette always centres
    /// its caption).
    pub layout: CaptionLayout,
    /// Whether the metadata strip is drawn at all.
    pub meta_policy: MetaPolicy,
    /// JPEG quality the encoder uses (`1..=100`).
    //
    // `tsify` doesn't see through `#[serde(try_from / into)]` —
    // it would otherwise emit the wrapper type by name. We override the
    // declared TS type so JS sees the raw number it actually receives
    // on the wire; the validating `TryFrom<u8>` impl still runs on
    // inbound deserialise.
    #[tsify(type = "number")]
    pub jpeg_quality: JpegQuality,
    /// If set, downscale the photo so its longer edge is at most this
    /// many pixels before framing. `None` keeps the source resolution.
    //
    // Same override reason as `jpeg_quality`. We also force `null`
    // instead of tsify's default `undefined` so JSON round-trip and
    // the TS type agree (Serde emits `null` for `Option::None`).
    #[tsify(type = "number | null")]
    pub max_long_edge: Option<LongEdge>,
}

impl PipelineSpec {
    /// Optimised for posting to social media: small file, modest
    /// quality, long edge clamped to 2048 px so platforms do not
    /// aggressively re-compress the upload.
    pub const SNS: Self = Self {
        frame_style: FrameStyle::Standard,
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
        frame_style: FrameStyle::Standard,
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
        frame_style: FrameStyle::Standard,
        theme: FrameTheme::Paper,
        layout: CaptionLayout::Edges,
        meta_policy: MetaPolicy::Auto,
        jpeg_quality: match JpegQuality::new(98) {
            Some(q) => q,
            None => panic!("JPEG quality 98 is in range"),
        },
        max_long_edge: None,
    };

    /// Builder: override [`Self::frame_style`].
    #[must_use]
    pub const fn with_frame_style(mut self, style: FrameStyle) -> Self {
        self.frame_style = style;
        self
    }

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

/// A named preset paired with its resolved [`PipelineSpec`].
///
/// `PipelineSpec::PRESETS` is the static `(label, spec)` table that
/// every front-end reads from; `Preset` is the owned, Serialize-able
/// view that crosses the WASM boundary as a typed array element, so
/// the JS UI can render preset names + drive its quality/long-edge
/// pickers without hand-duplicating the Rust source-of-truth values.
#[derive(Tsify, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[tsify(into_wasm_abi)]
pub struct Preset {
    /// Canonical kebab-case label (e.g. `"sns"`, `"standard"`,
    /// `"maximum"`). Matches the keys in `PipelineSpec::PRESETS` and
    /// the labels `PipelineSpec::from_str` accepts.
    pub label: String,
    /// Concrete pipeline configuration this preset names.
    pub spec: PipelineSpec,
}

impl PipelineSpec {
    /// Materialise [`Self::PRESETS`] as owned [`Preset`] rows. The WASM
    /// `get_presets` export calls this so JS receives a typed
    /// `Preset[]` it can iterate at init time.
    #[must_use]
    pub fn presets() -> Vec<Preset> {
        Self::PRESETS
            .iter()
            .map(|(label, spec)| Preset {
                label: (*label).to_owned(),
                spec: *spec,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameStyle, PipelineSpec};
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

    #[test]
    fn sns_spec_round_trips_as_snake_case_object_with_raw_primitives() {
        // Pin the JSON / WASM-FFI shape `PipelineSpec` exposes. The
        // JS-side `pkg/photo_frame_wasm.d.ts` is generated from the
        // same Serde derive (via tsify), so a shape drift caught
        // here is the same drift a `bun run typecheck` would catch on
        // the web side — but at Rust unit-test speed.
        let sns = PipelineSpec::SNS;
        let wire = serde_json::to_string(&sns).unwrap();
        assert_eq!(
            wire,
            r#"{"frame_style":"standard","theme":"paper","layout":"edges","meta_policy":"auto","jpeg_quality":78,"max_long_edge":2048}"#,
        );
        let round: PipelineSpec = serde_json::from_str(&wire).unwrap();
        assert_eq!(round, sns);
    }

    #[test]
    fn deserialize_rejects_out_of_range_primitives_at_the_wire_boundary() {
        // A malformed JSON payload (`jpeg_quality: 0` or
        // `max_long_edge: 0`) must fail at deserialise, not after —
        // the typed primitive's validating constructor is the only way
        // to construct a value, including across the FFI.
        let bad_quality = r#"{"frame_style":"standard","theme":"paper","layout":"edges","meta_policy":"auto","jpeg_quality":0,"max_long_edge":null}"#;
        assert!(serde_json::from_str::<PipelineSpec>(bad_quality).is_err());
        let bad_edge = r#"{"frame_style":"standard","theme":"paper","layout":"edges","meta_policy":"auto","jpeg_quality":92,"max_long_edge":0}"#;
        assert!(serde_json::from_str::<PipelineSpec>(bad_edge).is_err());
    }

    #[test]
    fn with_frame_style_polaroid_overrides_default_standard() {
        let spec = PipelineSpec::STANDARD.with_frame_style(FrameStyle::Polaroid);
        assert_eq!(spec.frame_style, FrameStyle::Polaroid);
        // Other fields preserved.
        assert_eq!(spec.jpeg_quality.get(), 92);
    }

    #[test]
    fn standard_spec_omits_long_edge_as_null() {
        let wire = serde_json::to_string(&PipelineSpec::STANDARD).unwrap();
        // `Option::None` serialises as JSON null — the JS side reads
        // it as `null` and the typed `max_long_edge: number | null`
        // field from tsify handles both arms naturally.
        assert!(wire.contains(r#""max_long_edge":null"#));
    }
}
