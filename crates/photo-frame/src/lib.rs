//! Liit-style golden-ratio photo framing.
//!
//! This crate is the **facade**: it re-exports every type a caller needs
//! from the three stage crates (`photo-frame-decode`, `photo-frame-frame`,
//! `photo-frame-encode`) and offers one helper, [`pipeline`], that runs
//! the whole bytes-in / JPEG-out path in three lines.
//!
//! For the WASM and CLI front-ends the helper is enough; the underlying
//! crates are also re-exported for callers that need to keep the
//! intermediate [`Photograph`] around (preview UIs, statistical tooling,
//! anything that wants to inspect [`Provenance`] without re-encoding).
//!
//! [`Provenance`]: photo_frame_types::Provenance

mod options;
mod pipeline;

pub use photo_frame_decode as decode;
pub use photo_frame_encode as encode;
pub use photo_frame_frame as frame;
pub use photo_frame_types as types;

pub use photo_frame_decode::DecodeError;
pub use photo_frame_encode::EncodeError;
pub use photo_frame_types::{
    Camera, DateTime, Exposure, Lens, Photograph, PixelError, Pixels, Provenance, QualityPreset,
};

pub use crate::options::PipelineOptions;
pub use crate::pipeline::{pipeline, PipelineError};
