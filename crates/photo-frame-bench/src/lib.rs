//! Shared fixtures and helpers for the photo-frame bench harness.
//!
//! The lib is intentionally tiny: bench code lives under `benches/` and
//! every public surface here exists so multiple bench binaries (the
//! divan wall-clock harness today, an iai-callgrind harness in Phase
//! A3) can share one canonical fixture corpus.

pub mod fixtures;
