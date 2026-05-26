//! Run the three pipeline stages back to back: decode → render → encode.

use miette::Diagnostic;
use photo_frame_decode::DecodeError;
use photo_frame_encode::EncodeError;
use photo_frame_types::{Categorize, Category, Stage, StageEvent};
use thiserror::Error;

use crate::options::PipelineOptions;

/// Errors the end-to-end [`pipeline`] can surface. Each variant wraps the
/// originating stage's error so the caller can `match` on which stage
/// failed without traversing source chains.
///
/// `#[diagnostic(transparent)]` defers diagnostic info (code, help,
/// labels) to the wrapped error — the pipeline itself adds no new
/// diagnostic content beyond "which stage".
#[derive(Debug, Error, Diagnostic)]
pub enum PipelineError {
    /// Decode stage failed. Wraps the typed [`DecodeError`] verbatim.
    #[error("decode failed")]
    #[diagnostic(transparent)]
    Decode(#[from] DecodeError),

    /// Encode stage failed. Wraps the typed [`EncodeError`] verbatim.
    #[error("encode failed")]
    #[diagnostic(transparent)]
    Encode(#[from] EncodeError),
}

impl Categorize for PipelineError {
    fn category(&self) -> Category {
        match self {
            Self::Decode(e) => e.category(),
            Self::Encode(e) => e.category(),
        }
    }
}

/// Sink for the per-stage progress events that `batch_one` (and any
/// future batch-aware caller) emits during a run.
///
/// The trait exists so the CLI's indicatif observer, the WASM bridge's
/// JS-function observer, and a test capturer all satisfy the same
/// type bound, eliminating each front-end's parallel implementation of
/// "decode percent + frame percent + encode percent". The blanket impl
/// for `FnMut(StageEvent)` keeps the closure spelling that callers
/// already use ergonomic.
pub trait PipelineObserver {
    /// Called once each pipeline stage completes for the item.
    fn on_stage(&mut self, event: StageEvent);
}

impl<F: FnMut(StageEvent)> PipelineObserver for F {
    fn on_stage(&mut self, event: StageEvent) {
        self(event);
    }
}

/// Decode `bytes`, frame the image, and JPEG-encode the result.
///
/// The `on_stage` callback fires once each stage completes (decode
/// → frame → encode) — front-ends use it to drive a real-time
/// progress bar. Tests and CLI pass `|_| {}` when they don't need
/// stage events.
///
/// `on_stage` is `FnMut` so a single closure can accumulate state
/// across calls (emitting JS events from the WASM bridge, for
/// example). The callback never receives a marker for a stage that
/// failed — if decode errors, only the error returns; if frame
/// succeeds but encode fails, the callback has seen `Decode` and
/// `Frame` before the encode error surfaces.
///
/// # Errors
/// See [`PipelineError`].
#[tracing::instrument(
    level = "info",
    name = "pipeline",
    skip(bytes, opts, on_stage),
    fields(
        input_bytes = bytes.len(),
        output_bytes = tracing::field::Empty,
    ),
)]
pub fn pipeline<F>(
    bytes: &[u8],
    opts: &PipelineOptions,
    mut on_stage: F,
) -> Result<Vec<u8>, PipelineError>
where
    F: FnMut(Stage),
{
    let photo = photo_frame_decode::from_bytes(bytes)?;
    on_stage(Stage::Decode);
    let framed = photo_frame_frame::render(&photo, &opts.frame);
    on_stage(Stage::Frame);
    let out = photo_frame_encode::jpeg(&framed, &opts.jpeg)?;
    on_stage(Stage::Encode);
    tracing::Span::current().record("output_bytes", out.len());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{pipeline, PipelineError, PipelineOptions, Stage};
    use image::{
        codecs::jpeg::JpegEncoder, ExtendedColorType, ImageEncoder, ImageReader, RgbImage,
    };
    use photo_frame_types::PipelineSpec;
    use std::io::Cursor;

    fn tiny_jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = RgbImage::from_pixel(w, h, image::Rgb([200, 60, 60]));
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, 90)
            .write_image(&img, w, h, ExtendedColorType::Rgb8)
            .expect("jpeg encode");
        out
    }

    #[test]
    fn pipeline_round_trips_jpeg_to_framed_jpeg() {
        let input = tiny_jpeg(80, 60);
        let out = pipeline(&input, &PipelineOptions::default(), |_| {}).expect("pipeline");
        let decoded = ImageReader::new(Cursor::new(&out))
            .with_guessed_format()
            .expect("guess")
            .decode()
            .expect("decode");
        // The synthetic JPEG has no EXIF → no caption strip. The
        // canvas is `photo + uniform mat on every side`, so it widens
        // and heightens by the same `2·mat` quantum.
        let (w, h) = (decoded.width(), decoded.height());
        assert!(w > 80, "canvas must widen by 2·mat");
        assert!(h > 60, "canvas must heighten by 2·mat");
        // No-meta layout adds the same mat to both axes, so canvas
        // aspect tracks photo aspect.
        let photo_aspect = 80.0_f64 / 60.0;
        let canvas_aspect = f64::from(w) / f64::from(h);
        assert!(
            (canvas_aspect / photo_aspect - 1.0).abs() < 0.1,
            "canvas aspect {canvas_aspect} should be near photo aspect {photo_aspect}",
        );
    }

    #[test]
    fn pipeline_propagates_decode_errors() {
        let err = pipeline(&[], &PipelineOptions::default(), |_| {}).expect_err("empty must fail");
        assert!(matches!(err, PipelineError::Decode(_)));
    }

    #[test]
    fn pipeline_invokes_on_stage_in_order() {
        let input = tiny_jpeg(80, 60);
        let mut stages = Vec::new();
        let out = pipeline(&input, &PipelineOptions::default(), |stage| {
            stages.push(stage);
        })
        .expect("pipeline");
        assert!(!out.is_empty());
        assert_eq!(stages, vec![Stage::Decode, Stage::Frame, Stage::Encode]);
    }

    #[test]
    fn pipeline_skips_on_stage_when_decode_fails() {
        let mut stages: Vec<Stage> = Vec::new();
        let err = pipeline(&[], &PipelineOptions::default(), |stage| stages.push(stage))
            .expect_err("empty must fail at decode");
        assert!(matches!(err, PipelineError::Decode(_)));
        assert!(
            stages.is_empty(),
            "no stage should complete on decode failure"
        );
    }

    #[test]
    fn sns_preset_caps_output_dimensions() {
        // 4000x3000 source → SNS caps long edge at 2048. Framing then adds
        // ~2*side on each axis, so the long edge of the output sits a bit
        // above 2048 but well below 4000.
        let input = tiny_jpeg(4000, 3000);
        let out = pipeline(
            &input,
            &PipelineOptions::from_spec(PipelineSpec::SNS),
            |_| {},
        )
        .expect("pipeline");
        let decoded = ImageReader::new(Cursor::new(&out))
            .with_guessed_format()
            .expect("guess")
            .decode()
            .expect("decode");
        assert!(decoded.width() < 4000, "SNS preset must downscale");
    }
}
