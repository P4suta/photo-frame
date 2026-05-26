//! Golden-ratio layout calculation.
//!
//! All composition starts from a single quantum,
//!
//! ```text
//! quantum = photo_short_edge / φ⁶
//! ```
//!
//! the φ-spiral residue six levels below the photo's short edge. The
//! same quantum drives the mat width (`2·quantum`), the primary
//! caption font (`quantum`), the secondary caption font (`quantum/φ`),
//! and the line gap (`quantum/φ²`). Every visible measurement in the
//! framed output is a small-integer multiple of the quantum or its
//! `1/φ` divisions.
//!
//! Two `LayoutStyle`s share that quantum:
//!
//! - **Standard** centres the photo with a uniform `2·quantum` mat on
//!   every interior boundary and renders the caption in a strip the
//!   same height as the mat.
//! - **Polaroid** top-anchors the photo and replaces the strip + bottom
//!   mat pair with a single `4·mat` band beneath, the classical
//!   thick-bottom signature of Polaroid prints.
//!
//! See [`composition`] for the construction in full.

mod composition;
mod rectangle;
mod spiral;

pub(crate) use composition::{compute, Composition, LayoutStyle, MetaLayout};
