//! Canonical data types shared across the photo-frame pipeline.
//!
//! Every crate in the workspace (decode, frame, encode, the facade, the CLI,
//! the WASM bridge) speaks in terms of the types defined here. The crate
//! stays a *thin vocabulary* — its behaviour is limited to validating
//! constructors, label↔enum parsers, percent-complete tables, and other
//! lookups that follow mechanically from the data the types already carry.
//! There is no domain logic, no I/O, and no dependency on the heavier
//! pipeline crates, so the rest of the workspace can pivot around it
//! without circular imports or upstream-version coupling.
//!
//! The canonical intermediate is [`Photograph`]: a normalized (upright)
//! pixel grid plus a structured [`Provenance`] that holds capture metadata
//! as *primitives*, never as pre-formatted display strings. Format choice
//! (`"105 mm"` vs `"105mm"` vs `"105"`) belongs to the renderer, not to
//! the data carrier.

mod category;
mod photograph;
mod pixels;
mod primitives;
mod provenance;
mod spec;

pub use crate::category::{Categorize, Category};
pub use crate::photograph::Photograph;
pub use crate::pixels::{PixelError, Pixels};
pub use crate::primitives::{
    Dimensions, ExifString, Fnumber, FocalLengthMm, IsoSensitivity, JpegQuality, LongEdge, Rgba8,
    ShutterSeconds,
};
pub use crate::provenance::{Camera, DateTime, Exposure, Lens, Provenance};
pub use crate::spec::{
    CaptionLayout, FrameStyle, FrameTheme, MetaPolicy, PipelineSpec, Preset, Stage, StageEvent,
};
