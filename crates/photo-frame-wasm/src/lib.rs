//! WASM bindings for the `photo-frame` facade.
//!
//! Two exports split the canonical pipeline along the encode/render
//! seam so a UI can cache framed pixels and re-encode independently of
//! the (expensive) decode + render:
//!
//! - [`render_pixels`] — `bytes + frame_opts` → `{ rgba, width, height }`
//! - [`encode_jpeg`]   — `rgba + dims + quality` → JPEG bytes
//!
//! Plus one atomic batch entry for the Worker:
//!
//! - [`frame_batch`] — bytes-in / bytes-out per item, in one round-trip
//!
//! `Photograph` (decoded + EXIF-applied pixels) stays inside `render_pixels`;
//! the JS side never sees it. We only ferry RGBA8 buffers across the WASM
//! boundary — the cheapest representation that both sides can use directly.
//!
//! Observability: at module load time we install `console_error_panic_hook`
//! and `tracing-wasm` so every `tracing` event from the underlying crates
//! lands in the browser console with structured fields. Failures come back
//! as `JsError`s whose message preserves the full source chain.

use std::error::Error;
use std::fmt::Write;

use js_sys::{Array, Object, Reflect, Uint8Array};
use photo_frame::encode::JpegOptions;
use photo_frame::frame::{CaptionLayout, FrameOptions, FrameTheme, MetaPolicy};
use photo_frame::{batch_one, PipelineError, PipelineOptions, Pixels};
use serde::Deserialize;
use tracing::{error, info};
use tracing_wasm::{set_as_global_default_with_config, WASMLayerConfigBuilder};
use wasm_bindgen::prelude::{wasm_bindgen, JsCast, JsError, JsValue};

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    let cfg = WASMLayerConfigBuilder::new()
        .set_max_level(tracing::Level::DEBUG)
        .build();
    set_as_global_default_with_config(cfg);
    info!("photo-frame-wasm initialised");
}

/// Decode `bytes` and render the framed RGBA8 grid.
///
/// Returns a JS object `{ rgba: Uint8Array, width: u32, height: u32 }`.
/// The RGBA buffer is the canonical `Pixels` content — `width * height * 4`
/// bytes — and the caller is expected to paint it onto a canvas via
/// `putImageData(new ImageData(new Uint8ClampedArray(rgba.buffer, ...)))`.
///
/// `frame_options` mirrors the renderer's [`FrameOptions`] minus
/// `jpeg_quality` — quality lives in [`encode_jpeg`], where it belongs.
///
/// # Errors
/// [`JsError`] with the full source chain of the underlying decode/encode
/// pipeline, or a `serde-wasm-bindgen` deserialisation error if
/// `frame_options` cannot be parsed.
#[wasm_bindgen]
pub fn render_pixels(bytes: &[u8], frame_options: JsValue) -> Result<JsValue, JsError> {
    let span = tracing::info_span!(
        "wasm_render_pixels",
        input_bytes = bytes.len(),
        width = tracing::field::Empty,
        height = tracing::field::Empty,
    );
    let _enter = span.enter();
    let frame_opts = build_frame_options(frame_options)?;

    let photo = photo_frame::decode::from_bytes(bytes)
        .map_err(|e| JsError::new(&display_chain(&PipelineError::Decode(e))))?;
    let framed = photo_frame::frame::render(&photo, &frame_opts);
    let (width, height, rgba) = framed.into_parts();
    tracing::Span::current().record("width", width);
    tracing::Span::current().record("height", height);

    let obj = Object::new();
    let buf = Uint8Array::new_with_length(u32::try_from(rgba.len()).unwrap_or(u32::MAX));
    buf.copy_from(&rgba);
    set_or_throw(&obj, "rgba", &buf)?;
    set_or_throw(&obj, "width", &JsValue::from_f64(f64::from(width)))?;
    set_or_throw(&obj, "height", &JsValue::from_f64(f64::from(height)))?;
    Ok(obj.into())
}

/// Encode an RGBA8 buffer at the given JPEG quality.
///
/// The caller owns `rgba` (typically a cached buffer from
/// [`render_pixels`]); we copy it into a `Pixels` only because the encode
/// crate consumes by reference and the buffer length must be validated
/// against the declared dimensions. Quality must be 1..=100.
///
/// # Errors
/// [`JsError`] with the full source chain when the buffer length doesn't
/// match `width * height * 4`, when quality is out of range, or when the
/// JPEG encoder itself fails.
#[wasm_bindgen]
pub fn encode_jpeg(rgba: &[u8], width: u32, height: u32, quality: u8) -> Result<Vec<u8>, JsError> {
    let span = tracing::info_span!(
        "wasm_encode_jpeg",
        width = width,
        height = height,
        quality = quality,
        output_bytes = tracing::field::Empty,
    );
    let _enter = span.enter();
    let pixels = Pixels::from_rgba8(width, height, rgba.to_vec()).map_err(|e| {
        error!(
            event_id = "wasm.pixels_invalid",
            error = %e,
            "RGBA buffer rejected by Pixels constructor",
        );
        JsError::new(&format!("invalid pixel buffer: {e}"))
    })?;
    let out = photo_frame::encode::jpeg(&pixels, &JpegOptions { quality })
        .map_err(|e| JsError::new(&display_chain(&PipelineError::Encode(e))))?;
    tracing::Span::current().record("output_bytes", out.len());
    Ok(out)
}

/// Frame every item in `items` and return one result object per
/// input. Designed to be called from a Web Worker so the main thread
/// stays free during large batches.
///
/// `items` is a JS array of `{ key: string, bytes: Uint8Array }`
/// objects. Each result has the shape `{ key, ok: bool,
/// result: Uint8Array | string, elapsed_ms: number }` — the `result`
/// field is the framed JPEG bytes on success or the human-readable
/// error chain on failure (mirroring [`display_chain`]).
///
/// `options` carries the full [`PipelineOptions`] shape including
/// `jpeg_quality` — batch is atomic per item so splitting encode out
/// would only inflate WASM ↔ JS hops without any caching benefit.
///
/// # Errors
/// A [`JsError`] is returned only for failures that prevent the batch
/// from running at all (malformed options, malformed item structure).
/// Per-item failures are *captured* in the returned array — a single
/// bad JPEG never aborts the batch.
#[wasm_bindgen]
pub fn frame_batch(items: &Array, options: JsValue) -> Result<Array, JsError> {
    let total = items.length();
    let span = tracing::info_span!(
        "wasm_frame_batch",
        total = total,
        succeeded = tracing::field::Empty,
        failed = tracing::field::Empty,
    );
    let _enter = span.enter();
    let pipeline_opts = build_pipeline_options(options)?;

    let results = Array::new();
    let mut succeeded = 0u32;
    let mut failed = 0u32;
    for (index, raw_item) in items.iter().enumerate() {
        let (key, bytes) = parse_batch_item(&raw_item, index)?;
        let outcome = batch_one(key.clone(), &bytes, &pipeline_opts);
        let item_obj = Object::new();
        set_or_throw(&item_obj, "key", &JsValue::from_str(&key))?;
        #[allow(
            clippy::cast_precision_loss,
            reason = "elapsed_ms within a browser tab will never approach 2^53"
        )]
        let elapsed_ms_f = outcome.elapsed_ms as f64;
        set_or_throw(&item_obj, "elapsed_ms", &JsValue::from_f64(elapsed_ms_f))?;
        match outcome.result {
            Ok(jpeg) => {
                succeeded += 1;
                let u8 = Uint8Array::new_with_length(u32::try_from(jpeg.len()).unwrap_or(u32::MAX));
                u8.copy_from(&jpeg);
                set_or_throw(&item_obj, "ok", &JsValue::TRUE)?;
                set_or_throw(&item_obj, "result", &u8)?;
            },
            Err(err) => {
                failed += 1;
                set_or_throw(&item_obj, "ok", &JsValue::FALSE)?;
                set_or_throw(
                    &item_obj,
                    "result",
                    &JsValue::from_str(&display_chain(&err)),
                )?;
            },
        }
        results.push(&item_obj);
    }
    span.record("succeeded", succeeded);
    span.record("failed", failed);
    info!(
        event_id = "wasm.frame_batch.done",
        total, succeeded, failed, "batch complete"
    );
    Ok(results)
}

/// Pull `{ key, bytes }` out of one JS-side batch item. The Worker
/// always builds items this way; failures here mean a programming
/// error on the JS side rather than a per-item processing failure,
/// so we surface them as `JsError` rather than baking them into a
/// per-item record.
fn parse_batch_item(raw: &JsValue, index: usize) -> Result<(String, Vec<u8>), JsError> {
    let obj: &Object = raw
        .dyn_ref::<Object>()
        .ok_or_else(|| JsError::new(&format!("batch item #{index} is not an object")))?;
    let key_val = Reflect::get(obj, &JsValue::from_str("key"))
        .map_err(|_| JsError::new(&format!("batch item #{index} has no `key` field")))?;
    let key = key_val
        .as_string()
        .ok_or_else(|| JsError::new(&format!("batch item #{index}: `key` must be a string")))?;
    let bytes_val = Reflect::get(obj, &JsValue::from_str("bytes"))
        .map_err(|_| JsError::new(&format!("batch item #{index} has no `bytes` field")))?;
    let bytes_arr: Uint8Array = bytes_val.dyn_into().map_err(|_| {
        JsError::new(&format!(
            "batch item #{index}: `bytes` must be a Uint8Array"
        ))
    })?;
    Ok((key, bytes_arr.to_vec()))
}

fn set_or_throw(obj: &Object, key: &str, value: &JsValue) -> Result<(), JsError> {
    Reflect::set(obj, &JsValue::from_str(key), value)
        .map_err(|_| JsError::new(&format!("failed to set `{key}` on result object")))?;
    Ok(())
}

/// Decode JS-side frame-only options (no `jpeg_quality`) into the typed
/// [`FrameOptions`]. Used by [`render_pixels`] only — batch carries the
/// full [`PipelineOptions`] shape, see [`build_pipeline_options`].
fn build_frame_options(options: JsValue) -> Result<FrameOptions, JsError> {
    let opts: JsFrameOptions = serde_wasm_bindgen::from_value(options).map_err(|e| {
        error!(
            event_id = "wasm.options_invalid",
            error = %e,
            "failed to parse JS frame options",
        );
        JsError::new(&format!("invalid frame options: {e}"))
    })?;
    Ok(FrameOptions {
        theme: parse_theme(&opts.theme).map_err(theme_error)?,
        layout: parse_layout(&opts.layout).map_err(layout_error)?,
        meta_policy: if opts.show_meta {
            MetaPolicy::Auto
        } else {
            MetaPolicy::Never
        },
        max_long_edge: opts.max_long_edge,
    })
}

/// Decode JS-side options for the atomic batch path into a full
/// [`PipelineOptions`]. Lives next to `frame_batch` because nothing
/// else needs it.
fn build_pipeline_options(options: JsValue) -> Result<PipelineOptions, JsError> {
    let opts: JsPipelineOptions = serde_wasm_bindgen::from_value(options).map_err(|e| {
        error!(
            event_id = "wasm.options_invalid",
            error = %e,
            "failed to parse JS pipeline options",
        );
        JsError::new(&format!("invalid pipeline options: {e}"))
    })?;
    Ok(PipelineOptions {
        frame: FrameOptions {
            theme: parse_theme(&opts.theme).map_err(theme_error)?,
            layout: parse_layout(&opts.layout).map_err(layout_error)?,
            meta_policy: if opts.show_meta {
                MetaPolicy::Auto
            } else {
                MetaPolicy::Never
            },
            max_long_edge: opts.max_long_edge,
        },
        jpeg: JpegOptions {
            quality: opts.jpeg_quality,
        },
    })
}

/// Convert a [`parse_theme`] failure into a [`JsError`] with a structured
/// tracing event. Kept separate from `parse_theme` so the parser stays a
/// pure function unit-testable on host targets (where `JsError` can't be
/// constructed).
fn theme_error(unknown: &str) -> JsError {
    error!(
        event_id = "wasm.theme_invalid",
        theme = %unknown,
        "unknown theme; expected `paper` or `ink`",
    );
    JsError::new(&format!(
        "invalid theme `{unknown}`: expected `paper` or `ink`"
    ))
}

/// Mirror of [`theme_error`] for the layout enum.
fn layout_error(unknown: &str) -> JsError {
    error!(
        event_id = "wasm.layout_invalid",
        layout = %unknown,
        "unknown layout; expected `edges` or `centered`",
    );
    JsError::new(&format!(
        "invalid layout `{unknown}`: expected `edges` or `centered`"
    ))
}

/// Render `err` as a single string carrying the full cause chain, mirroring
/// the CLI's error reporter.
fn display_chain(err: &PipelineError) -> String {
    let mut message = err.to_string();
    let mut source: Option<&dyn Error> = err.source();
    let mut depth = 0;
    while let Some(cause) = source {
        // Writing into a String never fails.
        let _ = write!(message, "\n  caused by [{depth}]: {cause}");
        source = cause.source();
        depth += 1;
    }
    error!(chain = %message, "framing failed");
    message
}

/// Frame-only options. `jpeg_quality` is intentionally absent: it lives
/// on the encode side ([`encode_jpeg`]).
#[derive(Debug, Deserialize)]
struct JsFrameOptions {
    theme: String,
    layout: String,
    show_meta: bool,
    max_long_edge: Option<u32>,
}

/// JS-facing batch options shape. `theme` / `layout` are the kebab-case
/// labels exposed by [`FrameTheme::label`] and [`CaptionLayout::label`];
/// see [`parse_theme`] / [`parse_layout`].
#[derive(Debug, Deserialize)]
struct JsPipelineOptions {
    jpeg_quality: u8,
    theme: String,
    layout: String,
    show_meta: bool,
    max_long_edge: Option<u32>,
}

/// Map the JS-side theme label back to the typed enum. Returns the
/// offending string on failure so the caller can name it in the
/// diagnostic — this keeps the parser host-testable.
fn parse_theme(raw: &str) -> Result<FrameTheme, &str> {
    match raw {
        s if s == FrameTheme::Paper.label() => Ok(FrameTheme::Paper),
        s if s == FrameTheme::Ink.label() => Ok(FrameTheme::Ink),
        other => Err(other),
    }
}

/// Map the JS-side layout label back to the typed enum.
fn parse_layout(raw: &str) -> Result<CaptionLayout, &str> {
    match raw {
        s if s == CaptionLayout::Edges.label() => Ok(CaptionLayout::Edges),
        s if s == CaptionLayout::Centered.label() => Ok(CaptionLayout::Centered),
        other => Err(other),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_layout, parse_theme, CaptionLayout, FrameTheme};

    #[test]
    fn parse_theme_accepts_known_labels() {
        assert_eq!(parse_theme("paper").unwrap(), FrameTheme::Paper);
        assert_eq!(parse_theme("ink").unwrap(), FrameTheme::Ink);
    }

    #[test]
    fn parse_theme_rejects_unknown() {
        assert_eq!(parse_theme("midnight"), Err("midnight"));
    }

    #[test]
    fn parse_layout_accepts_known_labels() {
        assert_eq!(parse_layout("edges").unwrap(), CaptionLayout::Edges);
        assert_eq!(parse_layout("centered").unwrap(), CaptionLayout::Centered);
    }

    #[test]
    fn parse_layout_rejects_unknown() {
        assert_eq!(parse_layout("stacked"), Err("stacked"));
    }
}
