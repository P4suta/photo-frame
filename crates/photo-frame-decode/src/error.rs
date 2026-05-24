use photo_frame_types::PixelError;
use thiserror::Error;

/// Every reason `from_bytes` can fail. Carries enough source-chain detail
/// for a CLI or WASM caller to render a useful message without re-parsing
/// the input.
#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("input is empty (0 bytes)")]
    EmptyInput,

    #[error("could not determine image format from input bytes")]
    UnknownFormat,

    // Kept in both feature configurations so downstream `match` arms compile
    // identically on host and wasm32. The variant carries no data.
    #[error("input is HEIC/HEIF but the `heif` feature is not enabled")]
    HeifFeatureDisabled,

    #[error("failed to decode image")]
    Decode(#[source] image::ImageError),

    #[cfg(feature = "heif")]
    #[error("failed to decode HEIC image via libheif")]
    HeifDecode(#[source] libheif_rs::HeifError),

    #[error("decoded pixels failed canonical-form validation")]
    InvalidPixels(#[from] PixelError),
}
