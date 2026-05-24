use miette::Diagnostic;
use photo_frame_types::{Categorize, Category, PixelError};
use thiserror::Error;

/// Every reason `from_bytes` can fail. Carries enough source-chain detail
/// for a CLI or WASM caller to render a useful message without re-parsing
/// the input.
///
/// Each variant has a stable `code(photo_frame::decode::<name>)`
/// diagnostic identifier — the CLI and any future telemetry pipeline
/// can dedupe / group on these without parsing the human-readable
/// message.
#[derive(Debug, Error, Diagnostic)]
pub enum DecodeError {
    #[error("input is empty (0 bytes)")]
    #[diagnostic(
        code(photo_frame::decode::empty_input),
        help("Pass a real image file. `--help` shows accepted formats.")
    )]
    EmptyInput,

    #[error("could not determine image format from input bytes")]
    #[diagnostic(
        code(photo_frame::decode::unknown_format),
        help(
            "Supported formats: JPEG, PNG, TIFF, BMP, WebP. \
             HEIC requires `--features heif` on the photo-frame-cli build."
        )
    )]
    UnknownFormat,

    // Kept in both feature configurations so downstream `match` arms compile
    // identically on host and wasm32. The variant carries no data.
    #[error("input is HEIC/HEIF but the `heif` feature is not enabled")]
    #[diagnostic(
        code(photo_frame::decode::heif_disabled),
        help(
            "Reinstall the CLI with the HEIC backend: \
             `cargo install --path crates/photo-frame-cli --features heif`. \
             Requires libheif >= 1.18 on the host (libheif-dev on Debian)."
        )
    )]
    HeifFeatureDisabled,

    #[error("failed to decode image")]
    #[diagnostic(
        code(photo_frame::decode::decode_error),
        help(
            "The bytes were a recognised format but the decoder couldn't \
             parse them. Likely causes: truncated download, wrong file \
             extension, or a format-specific corruption. The wrapped error \
             below names the format-specific reason."
        )
    )]
    Decode(#[source] image::ImageError),

    #[cfg(feature = "heif")]
    #[error("failed to decode HEIC image via libheif")]
    #[diagnostic(
        code(photo_frame::decode::heif_decode_error),
        help(
            "libheif rejected the input. Check the file with \
             `heif-info` or `heif-dec` to see whether libheif itself can \
             open it."
        )
    )]
    HeifDecode(#[source] libheif_rs::HeifError),

    #[error("decoded pixels failed canonical-form validation")]
    #[diagnostic(
        code(photo_frame::decode::invalid_pixels),
        help(
            "The format decoder returned a buffer whose length / dimensions \
             don't agree. This is a bug in the decode path, not the input."
        )
    )]
    InvalidPixels(#[from] PixelError),
}

impl Categorize for DecodeError {
    fn category(&self) -> Category {
        match self {
            Self::EmptyInput | Self::UnknownFormat | Self::HeifFeatureDisabled => Category::Input,
            Self::Decode(_) => Category::Decode,
            #[cfg(feature = "heif")]
            Self::HeifDecode(_) => Category::Decode,
            Self::InvalidPixels(_) => Category::Internal,
        }
    }
}
