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
use photo_frame::frame::{FrameOptions, FrameTheme, MetaPolicy};
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
    let opts: JsOptions = serde_wasm_bindgen::from_value(options).map_err(|e| {
        error!(
            event_id = "wasm.frame.options_invalid",
            error = %e,
            "failed to parse JS options"
        );
        JsError::new(&format!("invalid options: {e}"))
    })?;
    let theme = match parse_theme(&opts.theme) {
        Ok(t) => t,
        Err(unknown) => {
            error!(
                event_id = "wasm.frame.theme_invalid",
                theme = %unknown,
                "unknown theme; expected `paper` or `ink`",
            );
            return Err(JsError::new(&format!(
                "invalid theme `{unknown}`: expected `paper` or `ink`"
            )));
        },
    };
    let pipeline_opts = PipelineOptions {
        frame: FrameOptions {
            theme,
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
    let result = pipeline(bytes, &pipeline_opts).map_err(|e| JsError::new(&display_chain(&e)));
    if let Ok(out) = &result {
        tracing::Span::current().record("output_bytes", out.len());
    }
    result
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

/// JS-facing options shape. `theme` is the kebab-case label exposed by
/// [`FrameTheme::label`] (`"paper"` / `"ink"`); see [`parse_theme`].
#[derive(Debug, Deserialize)]
struct JsOptions {
    jpeg_quality: u8,
    theme: String,
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

#[cfg(test)]
mod tests {
    use super::{parse_theme, FrameTheme};

    #[test]
    fn parse_theme_accepts_known_labels() {
        assert_eq!(parse_theme("paper").unwrap(), FrameTheme::Paper);
        assert_eq!(parse_theme("ink").unwrap(), FrameTheme::Ink);
    }

    #[test]
    fn parse_theme_rejects_unknown() {
        assert_eq!(parse_theme("midnight"), Err("midnight"));
    }
}
