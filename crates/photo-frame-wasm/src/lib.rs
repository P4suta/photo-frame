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
use std::sync::{Mutex, OnceLock};

use js_sys::{Array, Function, Object, Reflect, Uint8Array};
use photo_frame::encode::JpegOptions;
use photo_frame::frame::{CaptionLayout, FrameOptions, FrameTheme, MetaPolicy};
use photo_frame::{
    batch_one, DecodeError, Photograph, PipelineError, PipelineOptions, Pixels, StageEvent,
};
use serde::Deserialize;
use tracing::{debug, error, info, warn};
use tracing_wasm::{set_as_global_default_with_config, WASMLayerConfigBuilder};
use wasm_bindgen::prelude::{wasm_bindgen, JsCast, JsError, JsValue};

// Phase F2 — re-export `wasm_bindgen_rayon::init_thread_pool` so JS
// callers can `await initThreadPool(N)` after `await init()` to bring
// the rayon worker pool online. Gated on `target_arch = "wasm32"` so
// the workspace's stable-rustc host builds (clippy, tests, etc.)
// don't pull the wasm-only dep into the dep graph for x86_64. JS
// runtime is responsible for checking `SharedArrayBuffer` support
// before calling — when SAB is absent the dep falls back gracefully
// at the `init_thread_pool` JS call site rather than at WASM init.
#[cfg(target_arch = "wasm32")]
pub use wasm_bindgen_rayon::init_thread_pool;

/// WASM module entry point invoked automatically by `wasm-bindgen` on
/// import. Installs the panic-to-console hook and wires `tracing`
/// events to `console.log`. Idempotent — safe to call multiple times.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    let cfg = WASMLayerConfigBuilder::new()
        .set_max_level(tracing::Level::DEBUG)
        .build();
    set_as_global_default_with_config(cfg);
    info!("photo-frame-wasm initialised");
}

// ─── Phase G1 — decoded-photograph cache ────────────────────────────────
//
// Module-level single-entry cache for the most-recently-decoded
// `Photograph`. The web UI calls `render_pixels` every time the user
// toggles `theme`, `layout`, or `show_meta`; none of those affect what
// `from_bytes` returns, so without the cache every toggle re-runs the
// JPEG IDCT + EXIF parse — typically ~100–300 ms at 24 MP, which is
// the dominant chunk of the perceived "wait between click and preview
// update" the user reported.
//
// Design choices that fall out of the use case:
//
// - **One entry, not LRU.** The web UI only shows one photo at a
//   time; "switch to a different image" is a relatively rare action
//   that warrants a real decode. A bigger cache would only matter
//   for batch-mode use cases that already have their own pipeline.
//
// - **Cheap fingerprint, not SHA.** We hash 16 bytes from each end of
//   the input plus the length. Collisions are theoretically possible
//   but the consequence is "we serve a stale photograph" — which we
//   prevent by also clearing the cache when JS uploads a new file
//   (the cache lives behind the same WASM module as `render_pixels`,
//   so it lives or dies with the worker, not with the user's session).
//   Even without that safety net, the fingerprint's failure mode is
//   bounded: a collision would just show the wrong image, never crash.
//   In practice collisions on real JPEG inputs are vanishingly rare.
//
// - **`Mutex<Option<…>>` not `RwLock`.** Reads and writes are 1:1
//   (every render call either hits-and-clones or misses-and-stores),
//   so the simpler primitive wins. Contention is non-existent because
//   the wasm-bindgen-rayon worker pool only enters this code from the
//   main thread; rayon-spawned threads use it only inside the renderer
//   for parallel pixel work, never to call `render_pixels` itself.
//
// - **`Photograph` is `Clone`** (derived in `photo-frame-types`).
//   On a cache hit we clone the cached photo out (one full-image
//   memcpy) and pass it by value to `render`. The clone cost is
//   ~30 ms at 24 MP vs the 100–300 ms decode it replaces — still a
//   big win, and crucially it preserves the Phase C1 zero-copy
//   handoff into `frame::render` (Photograph by value).
type CacheEntry = (u64, Photograph);
static PHOTO_CACHE: OnceLock<Mutex<Option<CacheEntry>>> = OnceLock::new();

fn photo_cache() -> &'static Mutex<Option<CacheEntry>> {
    PHOTO_CACHE.get_or_init(|| Mutex::new(None))
}

/// Cheap content-addressable identifier for an input image. Combines
/// the byte length with up to 16 bytes from each end so the hash sees
/// both the JPEG SOI marker (head) and the typical EOI + EXIF blob
/// (tail) — the two places real-world JPEGs differ even when they
/// share dimensions.
fn fingerprint(bytes: &[u8]) -> u64 {
    let mut h = (bytes.len() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let head_n = bytes.len().min(16);
    let tail_off = bytes.len().saturating_sub(16);
    for &b in bytes[..head_n].iter().chain(bytes[tail_off..].iter()) {
        h = h.wrapping_mul(31).wrapping_add(u64::from(b));
    }
    h
}

/// Either clone the cached `Photograph` (cache hit) or decode + cache
/// (miss). Returns an owned `Photograph` either way — the renderer
/// consumes it by value (Phase C1).
fn cached_or_decode(bytes: &[u8]) -> Result<Photograph, DecodeError> {
    let fp = fingerprint(bytes);
    let cache = photo_cache();
    {
        let guard = cache.lock().expect("photo_cache mutex poisoned");
        if let Some((cached_fp, photo)) = guard.as_ref() {
            if *cached_fp == fp {
                debug!(
                    event_id = "wasm.photo_cache.hit",
                    fp, "photograph cache hit"
                );
                return Ok(photo.clone());
            }
        }
    }
    debug!(
        event_id = "wasm.photo_cache.miss",
        fp, "photograph cache miss, decoding"
    );
    let photo = photo_frame::decode::from_bytes(bytes)?;
    let stored = photo.clone();
    {
        let mut guard = cache.lock().expect("photo_cache mutex poisoned");
        *guard = Some((fp, stored));
    }
    Ok(photo)
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

    let photo = cached_or_decode(bytes)
        .map_err(|e| JsError::new(&display_chain(&PipelineError::Decode(e))))?;
    let framed = photo_frame::frame::render(photo, &frame_opts);
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
/// error chain on failure (rendered the same way `display_chain` does).
///
/// `progress` is a JS function invoked once each pipeline stage
/// completes — front-ends use it to fill a per-item progress bar.
/// The event passed in is `{ index, total, key, stage }` where
/// `stage` is one of `"decode"`, `"frame"`, or `"encode"`. The JS
/// side decides how stage labels map to a numeric percentage.
///
/// # Errors
/// A [`JsError`] is returned only for failures that prevent the batch
/// from running at all (malformed options, malformed item structure).
/// Per-item failures are *captured* in the returned array — a single
/// bad JPEG never aborts the batch.
#[wasm_bindgen]
pub fn frame_batch(items: &Array, options: JsValue, progress: &Function) -> Result<Array, JsError> {
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
        let outcome = batch_one(
            key.clone(),
            index,
            total,
            &bytes,
            &pipeline_opts,
            |event: StageEvent| emit_progress(progress, &event),
        );
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

/// Hand the typed `StageEvent` to the JS-side `progress` callback.
///
/// `serde_wasm_bindgen::to_value` round-trips the struct through the
/// Serde shape that `tsify-next` already emits a `.d.ts` for, so the
/// JS side receives the exact discriminated union the type system
/// describes — no parallel field-by-field copying, no string casts.
fn emit_progress(progress: &Function, event: &StageEvent) {
    let payload = match serde_wasm_bindgen::to_value(event) {
        Ok(value) => value,
        Err(err) => {
            // Serialising a value we built ourselves should be
            // impossible to fail — surface it as an error so the
            // failure isn't a silent dropped progress event.
            error!(
                event_id = "wasm.frame_batch.progress.serialize_failed",
                index = event.index,
                key = %event.key,
                error = %err,
                "failed to serialize StageEvent",
            );
            return;
        },
    };
    if let Err(err) = progress.call1(&JsValue::NULL, &payload) {
        warn!(
            event_id = "wasm.frame_batch.progress.callback_threw",
            index = event.index,
            key = %event.key,
            error = ?err,
        );
    }
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
        theme: opts.theme,
        layout: opts.layout,
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
            theme: opts.theme,
            layout: opts.layout,
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

/// Frame-only options. `jpeg_quality` is intentionally absent: it lives
/// on the encode side ([`encode_jpeg`]).
///
/// `theme` and `layout` deserialize directly into the typed enums via
/// the Serde derive on `FrameTheme` / `CaptionLayout` — the JS side
/// sends the canonical lowercase label and Serde rejects anything else
/// with `unknown variant` at the boundary.
#[derive(Debug, Deserialize)]
struct JsFrameOptions {
    theme: FrameTheme,
    layout: CaptionLayout,
    show_meta: bool,
    max_long_edge: Option<u32>,
}

/// JS-facing batch options shape. Same Serde-driven parsing as
/// [`JsFrameOptions`] with the addition of `jpeg_quality`.
#[derive(Debug, Deserialize)]
struct JsPipelineOptions {
    jpeg_quality: u8,
    theme: FrameTheme,
    layout: CaptionLayout,
    show_meta: bool,
    max_long_edge: Option<u32>,
}
