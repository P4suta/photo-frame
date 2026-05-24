//! Pipeline orchestration.
//!
//! The order — decode → orientation-normalize → exif → geometry → frame →
//! encode — is the contract callers depend on. Each stage is implemented in
//! its own module; this file does composition only.
//!
//! Observability: the entry point opens a top-level INFO span and emits
//! per-stage DEBUG events with timings. Unusual paths (fallback firing,
//! caption suppression, EXIF-less inputs) emit WARN events at the stage
//! that decides them.

use std::time::Instant;

use image::{imageops::FilterType, DynamicImage};
use tracing::{debug, info, instrument, warn};

use crate::decode;
use crate::encode;
use crate::error::FrameError;
use crate::exif::{self, Meta};
use crate::frame;
use crate::geometry;
use crate::num::round_to_u32;
use crate::options::{FrameOptions, MetaPolicy};
use crate::orientation;

/// Frame a single image. The input is raw bytes of a JPEG or PNG; the output
/// is a JPEG byte stream containing the framed photo.
///
/// # Errors
/// See [`FrameError`]. Common cases:
/// - [`FrameError::EmptyInput`]: the byte slice was empty.
/// - [`FrameError::Decode`]: input wasn't a valid JPEG/PNG.
/// - [`FrameError::ZeroDimension`]: decoded (or downscaled) image was 0×N.
/// - [`FrameError::QualityOutOfRange`]: `opts.jpeg_quality` outside 1..=100.
/// - [`FrameError::Encode`]: JPEG encoder failed (typically OOM).
#[instrument(
    name = "frame_image",
    skip(bytes, opts),
    fields(
        input_bytes  = bytes.len(),
        jpeg_quality = opts.jpeg_quality,
        meta_policy  = ?opts.meta_policy,
        max_long_edge = ?opts.max_long_edge,
    ),
)]
pub fn frame_image(bytes: &[u8], opts: &FrameOptions) -> Result<Vec<u8>, FrameError> {
    let started = Instant::now();
    info!("framing started");

    if bytes.is_empty() {
        warn!("rejecting empty input");
        return Err(FrameError::EmptyInput);
    }

    let result = run(bytes, opts);

    let elapsed_ms = started.elapsed().as_millis();
    match &result {
        Ok(out) => info!(elapsed_ms, output_bytes = out.len(), "framing complete"),
        Err(err) => warn!(elapsed_ms, error = %err, category = ?err.category(), "framing failed"),
    }
    result
}

fn run(bytes: &[u8], opts: &FrameOptions) -> Result<Vec<u8>, FrameError> {
    let (raw_image, exif_data) = decode::decode(bytes)?;
    let exif_present = exif_data.is_some();
    debug!(exif_present, "decode stage complete");

    let orientation_value = exif::orientation_of(exif_data.as_ref());
    let upright = orientation::normalize(raw_image, orientation_value);
    debug!(
        ?orientation_value,
        width = upright.width(),
        height = upright.height(),
        "orientation stage complete"
    );

    let upright = downscale(upright, opts.max_long_edge);
    if upright.width() == 0 || upright.height() == 0 {
        return Err(FrameError::ZeroDimension {
            width: upright.width(),
            height: upright.height(),
        });
    }

    let meta = exif_data.as_ref().map(exif::extract);
    let visible_meta = choose_visible_meta(opts.meta_policy, meta.as_ref());
    if exif_present && visible_meta.is_none() {
        warn!(policy = ?opts.meta_policy, "EXIF parsed but caption strip will be suppressed");
    }

    let layout = geometry::compute((upright.width(), upright.height()), visible_meta.is_some());
    debug!(
        canvas_w = layout.canvas_size.0,
        canvas_h = layout.canvas_size.1,
        caption = visible_meta.is_some(),
        "geometry stage complete",
    );

    let canvas = frame::render(&layout, &upright, visible_meta, opts.background);
    debug!("composite stage complete");

    let bytes = encode::jpeg(&canvas, opts.jpeg_quality)?;
    Ok(bytes)
}

fn downscale(img: DynamicImage, max_long_edge: Option<u32>) -> DynamicImage {
    let Some(max) = max_long_edge else {
        return img;
    };
    let long = img.width().max(img.height());
    if long <= max {
        return img;
    }
    let ratio = f64::from(max) / f64::from(long);
    let new_w = round_to_u32(f64::from(img.width()) * ratio).max(1);
    let new_h = round_to_u32(f64::from(img.height()) * ratio).max(1);
    debug!(
        from_w = img.width(),
        from_h = img.height(),
        to_w = new_w,
        to_h = new_h,
        "downscaled"
    );
    img.resize(new_w, new_h, FilterType::Lanczos3)
}

fn choose_visible_meta(policy: MetaPolicy, meta: Option<&Meta>) -> Option<&Meta> {
    match policy {
        MetaPolicy::Never => None,
        MetaPolicy::Auto => meta.filter(|m| !m.is_empty()),
    }
}
