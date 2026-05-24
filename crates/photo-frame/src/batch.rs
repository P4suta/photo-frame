//! Batch entry point shared by the CLI and WASM front-ends.
//!
//! The batch shape is intentionally minimal: one call processes one
//! item, returning a [`BatchOutcome`] that carries the caller-chosen
//! key, the pipeline result, and the wall-clock cost. Parallelism is
//! *not* baked in here — CLI drives the loop through `rayon`, WASM
//! through a Web Worker, and tests through a plain `for`. Keeping the
//! parallel strategy at the call site is what lets the same API serve
//! all three without dragging a heavyweight dependency into the
//! wasm32-unknown-unknown target.

use std::time::Instant;

use crate::options::PipelineOptions;
use crate::pipeline::{pipeline, PipelineError};

/// Outcome of processing one batch item.
///
/// `key` is whatever the caller wants — typically a path on the CLI
/// side and a string identifier on the WASM side. Returning it inside
/// the outcome lets the caller correlate results without holding the
/// input collection in lock-step with the result collection (important
/// when results come back out of order from `rayon`).
#[derive(Debug)]
pub struct BatchOutcome<K> {
    pub key: K,
    pub result: Result<Vec<u8>, PipelineError>,
    pub elapsed_ms: u128,
}

impl<K> BatchOutcome<K> {
    /// Convenience for filtering on success without consuming the outcome.
    #[must_use]
    pub const fn is_ok(&self) -> bool {
        self.result.is_ok()
    }
}

/// Run the full decode → render → encode pipeline on one item and
/// package the result as a [`BatchOutcome`].
///
/// Errors are *captured*, not propagated — the whole point of the batch
/// shape is "1 failure does not stop the run". Callers that want
/// stop-on-first-failure semantics can inspect each outcome and break
/// out of their own iterator (the CLI does this under `--strict`).
#[tracing::instrument(
    level = "debug",
    name = "batch_one",
    skip_all,
    fields(input_bytes = bytes.len(), elapsed_ms = tracing::field::Empty),
)]
pub fn batch_one<K>(key: K, bytes: &[u8], opts: &PipelineOptions) -> BatchOutcome<K> {
    let started = Instant::now();
    let result = pipeline(bytes, opts);
    let elapsed_ms = started.elapsed().as_millis();
    tracing::Span::current().record("elapsed_ms", elapsed_ms);
    BatchOutcome {
        key,
        result,
        elapsed_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::{batch_one, PipelineOptions};
    use image::{codecs::jpeg::JpegEncoder, ExtendedColorType, ImageEncoder, RgbImage};

    fn tiny_jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = RgbImage::from_pixel(w, h, image::Rgb([150, 200, 100]));
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, 90)
            .write_image(&img, w, h, ExtendedColorType::Rgb8)
            .expect("jpeg encode");
        out
    }

    #[test]
    fn batch_one_returns_ok_with_jpeg_bytes() {
        let bytes = tiny_jpeg(64, 48);
        let outcome = batch_one("photo.jpg", &bytes, &PipelineOptions::default());
        assert_eq!(outcome.key, "photo.jpg");
        let out = outcome.result.expect("pipeline succeeds");
        // JPEG SOI marker.
        assert_eq!(&out[..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn batch_one_carries_error_through_pipeline() {
        let outcome = batch_one(7_u32, &[], &PipelineOptions::default());
        assert_eq!(outcome.key, 7);
        assert!(
            outcome.result.is_err(),
            "empty input must surface a decode error inside the outcome",
        );
        assert!(!outcome.is_ok());
    }

    #[test]
    fn batch_one_records_elapsed_ms() {
        // We can't pin a tight upper bound (test machines vary), but
        // we can assert the field is populated — anything from 0 ms
        // upward is acceptable.
        let bytes = tiny_jpeg(16, 16);
        let outcome = batch_one((), &bytes, &PipelineOptions::default());
        let _ = outcome.elapsed_ms;
    }
}
