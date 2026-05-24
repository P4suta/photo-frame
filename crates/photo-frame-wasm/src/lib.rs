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

use photo_frame::encode::JpegOptions;
use photo_frame::frame::{Background, FrameOptions, MetaPolicy};
use photo_frame::{pipeline, PipelineError, PipelineOptions};
use serde::Deserialize;
use tracing::{error, info};
use tracing_wasm::{set_as_global_default_with_config, WASMLayerConfigBuilder};
use wasm_bindgen::prelude::{wasm_bindgen, JsError, JsValue};

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
    let opts: JsOptions = serde_wasm_bindgen::from_value(options).map_err(|e| {
        error!(error = %e, "failed to parse JS options");
        JsError::new(&format!("invalid options: {e}"))
    })?;
    let pipeline_opts = PipelineOptions {
        frame: FrameOptions {
            background: Background::from_rgb(opts.bg_r, opts.bg_g, opts.bg_b),
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
    };
    pipeline(bytes, &pipeline_opts).map_err(|e| JsError::new(&display_chain(&e)))
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

/// JS-facing options shape. Flattens the RGB triple so JSON object
/// construction is straightforward on the browser side.
#[derive(Debug, Deserialize)]
struct JsOptions {
    jpeg_quality: u8,
    bg_r: u8,
    bg_g: u8,
    bg_b: u8,
    show_meta: bool,
    max_long_edge: Option<u32>,
}
