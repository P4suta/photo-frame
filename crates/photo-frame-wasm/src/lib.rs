//! WASM bindings for the `photo-frame` facade.
//!
//! Exposes a single function — [`frame`] — that takes raw image bytes
//! and a JS-side options object, runs the full decode → render → encode
//! pipeline, and returns the framed JPEG bytes. The browser handles all
//! `File` / `ArrayBuffer` / `Blob` conversions.
//!
//! Observability: at module load time we install
//! `console_error_panic_hook` and `tracing-wasm` so every `tracing` event
//! from the underlying crates lands in the browser console with
//! structured fields. Failures come back as `JsError`s whose message
//! preserves the full source chain.

use std::error::Error;
use std::fmt::Write;

use js_sys::{Array, Object, Reflect, Uint8Array};
use photo_frame::encode::JpegOptions;
use photo_frame::frame::{CaptionLayout, FrameOptions, FrameTheme, MetaPolicy};
use photo_frame::{batch_one, pipeline, PipelineError, PipelineOptions};
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

/// Frame `bytes` and return the framed JPEG bytes.
///
/// # Errors
/// Returns a [`JsError`] whose message contains the full source chain of
/// the underlying [`PipelineError`] (or a `serde-wasm-bindgen`
/// deserialisation error if `options` cannot be parsed).
#[wasm_bindgen]
pub fn frame(bytes: &[u8], options: JsValue) -> Result<Vec<u8>, JsError> {
    // wasm_bindgen rewrites the fn signature in ways that confuse
    // `#[tracing::instrument]` (the JsValue param is gone by the time
    // the macro sees it). Hand-roll the span so the same structured
    // contract still holds.
    let span = tracing::info_span!(
        "wasm_frame",
        input_bytes = bytes.len(),
        output_bytes = tracing::field::Empty,
    );
    let _enter = span.enter();
    let pipeline_opts = build_pipeline_options(options)?;
    let result = pipeline(bytes, &pipeline_opts).map_err(|e| JsError::new(&display_chain(&e)));
    if let Ok(out) = &result {
        tracing::Span::current().record("output_bytes", out.len());
    }
    result
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

/// Decode the JS-side options object into the typed
/// [`PipelineOptions`]. Reused by both single ([`frame`]) and batch
/// ([`frame_batch`]) entry points — the shape is identical.
fn build_pipeline_options(options: JsValue) -> Result<PipelineOptions, JsError> {
    let opts: JsOptions = serde_wasm_bindgen::from_value(options).map_err(|e| {
        error!(
            event_id = "wasm.options_invalid",
            error = %e,
            "failed to parse JS options"
        );
        JsError::new(&format!("invalid options: {e}"))
    })?;
    let theme = parse_theme(&opts.theme).map_err(|unknown| {
        error!(
            event_id = "wasm.theme_invalid",
            theme = %unknown,
            "unknown theme; expected `paper` or `ink`",
        );
        JsError::new(&format!(
            "invalid theme `{unknown}`: expected `paper` or `ink`"
        ))
    })?;
    let layout = parse_layout(&opts.layout).map_err(|unknown| {
        error!(
            event_id = "wasm.layout_invalid",
            layout = %unknown,
            "unknown layout; expected `edges` or `centered`",
        );
        JsError::new(&format!(
            "invalid layout `{unknown}`: expected `edges` or `centered`"
        ))
    })?;
    Ok(PipelineOptions {
        frame: FrameOptions {
            theme,
            layout,
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

/// JS-facing options shape. `theme` / `layout` are the kebab-case
/// labels exposed by [`FrameTheme::label`] and [`CaptionLayout::label`];
/// see [`parse_theme`] / [`parse_layout`].
#[derive(Debug, Deserialize)]
struct JsOptions {
    jpeg_quality: u8,
    theme: String,
    layout: String,
    show_meta: bool,
    max_long_edge: Option<u32>,
}

/// Map the JS-side theme label back to the typed enum. Returns the
/// offending string so the caller can name it in the diagnostic.
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
