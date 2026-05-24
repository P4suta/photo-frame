//! Liit-style golden-ratio photo framing.
//!
//! The public surface is intentionally minimal: one entry point
//! ([`frame_image`]), two configuration types ([`FrameOptions`],
//! [`MetaPolicy`], [`Background`]), and one error type ([`FrameError`]).
//!
//! Internal modules expose the building blocks they use to each other but are
//! not re-exported — a contributor reading [`lib.rs`] should be able to learn
//! the entire user-facing API at a glance.
//!
//! ```no_run
//! use photo_frame_core::{frame_image, FrameOptions};
//!
//! let input  = std::fs::read("photo.jpg").unwrap();
//! let output = frame_image(&input, &FrameOptions::default()).unwrap();
//! std::fs::write("framed.jpg", output).unwrap();
//! ```

mod decode;
mod encode;
mod error;
mod exif;
mod frame;
mod geometry;
mod num;
mod options;
mod orientation;
mod pipeline;
mod text;

pub use crate::error::{ErrorCategory, FrameError};
pub use crate::options::{Background, FrameOptions, MetaPolicy, QualityPreset};
pub use crate::pipeline::frame_image;
