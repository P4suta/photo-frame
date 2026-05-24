//! WASM bindings for `photo-frame-core`.
//!
//! The single exported function [`frame`] takes raw image bytes plus a
//! JSON-like options object and returns the framed JPEG bytes. The browser
//! side handles `File` / `ArrayBuffer` / `Blob` conversions.
//!
//! Observability: at module load time we install `console_error_panic_hook`
//! and `tracing-wasm` so every `tracing` event from `photo-frame-core` lands
//! in the browser console with structured fields. Failures are returned as
//! `JsError`s whose message preserves the [`FrameError`] cause chain.

use photo_frame_core::{frame_image, Background, FrameError, FrameOptions, MetaPolicy};
use serde::Deserialize;
use tracing::{error, info};
use tracing_wasm::{set_as_global_default_with_config, WASMLayerConfigBuilder};
use wasm_bindgen::prelude::{wasm_bindgen, JsError, JsValue};

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    // `set_as_global_default_with_config` is idempotent against multiple
    // module reloads (HMR); duplicate-init errors are silently absorbed.
    let cfg = WASMLayerConfigBuilder::new()
        .set_max_level(tracing::Level::DEBUG)
        .build();
    set_as_global_default_with_config(cfg);
    info!("photo-frame-wasm initialised");
}

/// Frame `bytes` (JPEG/PNG) and return the framed JPEG bytes.
///
/// # Errors
/// Returns a [`JsError`] whose message contains the full source chain of
/// the underlying [`FrameError`] (or a `serde-wasm-bindgen` deserialisation
/// error if `options` cannot be parsed).
#[wasm_bindgen]
pub fn frame(bytes: &[u8], options: JsValue) -> Result<Vec<u8>, JsError> {
    let opts: JsOptions = serde_wasm_bindgen::from_value(options).map_err(|e| {
        error!(error = %e, "failed to parse JS options");
        JsError::new(&format!("invalid options: {e}"))
    })?;
    let core_opts = FrameOptions {
        jpeg_quality: opts.jpeg_quality,
        background: Background::from_rgb(opts.bg_r, opts.bg_g, opts.bg_b),
        meta_policy: if opts.show_meta {
            MetaPolicy::Auto
        } else {
            MetaPolicy::Never
        },
        max_long_edge: opts.max_long_edge,
    };
    frame_image(bytes, &core_opts).map_err(|e| JsError::new(&display_chain(&e)))
}

/// Render `e` as a single string carrying the full cause chain, mirroring
/// the CLI's error reporter.
fn display_chain(e: &FrameError) -> String {
    use std::error::Error;
    use std::fmt::Write;
    let mut message = e.to_string();
    let mut source: Option<&dyn Error> = e.source();
    let mut depth = 0;
    while let Some(cause) = source {
        // Writing into a String never fails — `unwrap_or_else(|_| ())` is the
        // idiomatic shape for `write!` into a `fmt::Write` here.
        let _ = write!(message, "\n  caused by [{depth}]: {cause}");
        source = cause.source();
        depth += 1;
    }
    error!(category = ?e.category(), chain = %message, "framing failed");
    message
}

/// JS-facing options shape. Mirrors [`FrameOptions`] but flattens the RGB
/// triple so JSON object construction is straightforward on the browser side.
#[derive(Debug, Deserialize)]
struct JsOptions {
    jpeg_quality: u8,
    bg_r: u8,
    bg_g: u8,
    bg_b: u8,
    show_meta: bool,
    max_long_edge: Option<u32>,
}
