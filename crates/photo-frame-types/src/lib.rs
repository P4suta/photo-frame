//! Canonical data types shared across the photo-frame pipeline.
//!
//! Every crate in the workspace (decode, frame, encode, the facade, the CLI,
//! the WASM bridge) speaks in terms of the types defined here. There are
//! deliberately **no behaviours** in this crate beyond constructors and
//! invariant checks — the goal is a thin, dependency-free vocabulary that
//! the rest of the pipeline can pivot around without circular imports or
//! upstream-version coupling.
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
    Dimensions, ExifString, Fnumber, IsoSensitivity, JpegQuality, LongEdge, Rgba8,
};
pub use crate::provenance::{Camera, DateTime, Exposure, Lens, Provenance};
pub use crate::spec::{CaptionLayout, FrameTheme, MetaPolicy, PipelineSpec, Stage, StageEvent};
