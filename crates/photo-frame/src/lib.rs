//! Golden-ratio photo framing.
//!
//! The canvas is a golden rectangle; the photo lives in the upper square
//! that subdivision produces, and a meta strip — the subdivision residue
//! — carries the caption. Every dimension downstream is a measurement on
//! a sub-rectangle, not the result of a `φⁿ` multiplication.
//!
//! This crate is the **facade**: it re-exports every type a caller needs
//! from the three stage crates (`photo-frame-decode`, `photo-frame-frame`,
//! `photo-frame-encode`) and offers one helper, [`pipeline()`], that
//! runs the whole bytes-in / JPEG-out path in three lines.
//!
//! For the WASM and CLI front-ends the helper is enough; the underlying
//! crates are also re-exported for callers that need to keep the
//! intermediate [`Photograph`] around (preview UIs, statistical tooling,
//! anything that wants to inspect [`Provenance`] without re-encoding).
//!
//! [`Provenance`]: photo_frame_types::Provenance

mod batch;
mod options;
mod pipeline;

#[cfg(feature = "trace")]
pub mod trace;

pub use photo_frame_decode as decode;
pub use photo_frame_encode as encode;
pub use photo_frame_frame as frame;
pub use photo_frame_types as types;

pub use photo_frame_decode::DecodeError;
pub use photo_frame_encode::EncodeError;
pub use photo_frame_types::{
    Camera, CaptionLayout, Categorize, Category, DateTime, Dimensions, ExifString, Exposure,
    Fnumber, FrameTheme, IsoSensitivity, JpegQuality, Lens, LongEdge, MetaPolicy, Photograph,
    PipelineSpec, PixelError, Pixels, Provenance, Rgba8, Stage, StageEvent,
};

pub use crate::batch::{batch_one, BatchOutcome};
pub use crate::options::PipelineOptions;
pub use crate::pipeline::{pipeline, PipelineError};
