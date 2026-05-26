//! Render a [`Photograph`] into a framed RGBA8 [`Pixels`] grid.
//!
//! The framing is a single geometric construction: the canvas itself is
//! a golden rectangle. One subdivision partitions it into the square
//! slot the photo occupies and the meta strip that carries the caption;
//! recursive subdivision of the strip's own residue yields the
//! typographic quantum — no `φⁿ` multiplier appears anywhere in the
//! layout. The full construction lives in the crate-private `geometry`
//! module; see `src/geometry/composition.rs` for the call site and
//! `src/geometry/rectangle.rs` for the rectangle primitive.
//!
//! Public surface is intentionally tight: one function ([`render()`])
//! plus its options struct ([`FrameOptions`]). Encoding is the next
//! crate's concern; the renderer returns an in-memory pixel grid.

mod format;
mod geometry;
mod num;
mod options;
mod render;
mod text;

pub use crate::options::{CaptionLayout, FrameOptions, FrameTheme, MetaPolicy};
pub use photo_frame_types::{Photograph, Pixels};

/// Render `photo` into a framed RGBA8 grid.
///
/// Takes `photo` by value so the decoder's pixel buffer can be moved
/// straight into the renderer's `RgbaImage` without an intermediate
/// copy. Callers that need to keep the `Photograph` around can
/// `clone()` it explicitly before invoking — the explicit clone
/// makes the cost visible at the call site, instead of paying for
/// it inside the renderer where every consumer would.
///
/// The result is always a valid [`Pixels`] — the geometry layer
/// guarantees positive canvas dimensions for any non-zero input
/// photo, and [`Photograph`] already carries a non-zero [`Pixels`].
#[must_use]
pub fn render(photo: Photograph, opts: &FrameOptions) -> Pixels {
    render::render(photo, opts)
}
