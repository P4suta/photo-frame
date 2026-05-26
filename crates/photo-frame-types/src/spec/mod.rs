//! Specification types — the canonical shape of "what a pipeline run is".
//!
//! Where the rest of `photo-frame-types` carries facts that arrived with
//! the photograph (pixels, provenance, dimensions), `spec` carries the
//! inputs and observable events of the pipeline that processes one.
//! They sit side by side because they share the same role: a thin
//! vocabulary every crate in the workspace can pivot around.

mod stage;

pub use crate::spec::stage::{Stage, StageEvent};
