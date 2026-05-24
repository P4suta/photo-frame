//! Run the three pipeline stages back to back: decode → render → encode.

use photo_frame_decode::DecodeError;
use photo_frame_encode::EncodeError;
use thiserror::Error;

use crate::options::PipelineOptions;

/// Errors the end-to-end [`pipeline`] can surface. Each variant wraps the
/// originating stage's error so the caller can `match` on which stage
/// failed without traversing source chains.
#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("decode failed")]
    Decode(#[from] DecodeError),
    #[error("encode failed")]
    Encode(#[from] EncodeError),
}

/// Decode `bytes`, frame the image, and JPEG-encode the result.
///
/// # Errors
/// See [`PipelineError`].
#[tracing::instrument(
    level = "info",
    name = "pipeline",
    skip(bytes, opts),
    fields(
        input_bytes = bytes.len(),
        output_bytes = tracing::field::Empty,
    ),
)]
pub fn pipeline(bytes: &[u8], opts: &PipelineOptions) -> Result<Vec<u8>, PipelineError> {
    let photo = photo_frame_decode::from_bytes(bytes)?;
    let framed = photo_frame_frame::render(&photo, &opts.frame);
    let out = photo_frame_encode::jpeg(&framed, &opts.jpeg)?;
    tracing::Span::current().record("output_bytes", out.len());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{pipeline, PipelineError, PipelineOptions};
    use image::{
        codecs::jpeg::JpegEncoder, ExtendedColorType, ImageEncoder, ImageReader, RgbImage,
    };
    use photo_frame_types::QualityPreset;
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
        let out = pipeline(&input, &PipelineOptions::default()).expect("pipeline");
        let decoded = ImageReader::new(Cursor::new(&out))
            .with_guessed_format()
            .expect("guess")
            .decode()
            .expect("decode");
        assert!(decoded.width() > 80);
        assert!(decoded.height() > 60);
    }

    #[test]
    fn pipeline_propagates_decode_errors() {
        let err = pipeline(&[], &PipelineOptions::default()).expect_err("empty must fail");
        assert!(matches!(err, PipelineError::Decode(_)));
    }

    #[test]
    fn sns_preset_caps_output_dimensions() {
        // 4000x3000 source → SNS caps long edge at 2048. Framing then adds
        // ~2*side on each axis, so the long edge of the output sits a bit
        // above 2048 but well below 4000.
        let input = tiny_jpeg(4000, 3000);
        let out =
            pipeline(&input, &PipelineOptions::from_preset(QualityPreset::Sns)).expect("pipeline");
        let decoded = ImageReader::new(Cursor::new(&out))
            .with_guessed_format()
            .expect("guess")
            .decode()
            .expect("decode");
        assert!(decoded.width() < 4000, "SNS preset must downscale");
    }
}
