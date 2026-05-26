//! Stage marker and progress event for the decode → frame → encode pipeline.

use serde::{Deserialize, Serialize};
use tsify::Tsify;

/// Marks which pipeline stage has just finished.
///
/// Surfaced through the pipeline's progress callback so long-running
/// front-ends (the WASM batch path, the CLI's indicatif bar) can drive
/// an item-internal progress bar.
#[derive(Tsify, Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    /// `photo_frame_decode::from_bytes` returned. The pixel buffer and
    /// EXIF provenance now exist in memory.
    Decode,
    /// `photo_frame_frame::render` returned. The framed RGBA8 canvas
    /// now exists; encoding has not started yet.
    Frame,
    /// `photo_frame_encode::jpeg` returned. The JPEG bytes are ready to
    /// hand back to the caller.
    Encode,
}

impl Stage {
    /// Cumulative percent of total pipeline work after this stage
    /// completes. Derived from `BENCHMARKS.md`: decode dominates with
    /// ~33% of wall time, the framing pass adds the next ~36%
    /// (cumulative 69%), and encode rounds out to 100%.
    ///
    /// Front-ends fill a per-item progress bar by reading these values
    /// directly so each does not need to keep its own table in sync.
    #[must_use]
    pub const fn percent_complete(self) -> u8 {
        match self {
            Self::Decode => 33,
            Self::Frame => 69,
            Self::Encode => 100,
        }
    }
}

/// A single progress event emitted when a pipeline stage completes.
///
/// Carries everything a front-end needs for a per-item progress bar:
/// which item (`index` / `total` / `key`), which stage just finished
/// (`stage`), and the cumulative percent that maps to (`percent`,
/// pre-computed from [`Stage::percent_complete`] so JS / TS consumers
/// don't need to know the weighting table).
#[derive(Tsify, Clone, Debug, PartialEq, Eq, Serialize)]
#[tsify(into_wasm_abi)]
pub struct StageEvent {
    /// Stage that just completed.
    pub stage: Stage,
    /// Cumulative percent (`0..=100`) of work done on this item.
    pub percent: u8,
    /// Zero-based index of the item inside its batch.
    pub index: usize,
    /// Total number of items in the batch.
    pub total: u32,
    /// Stable per-item key supplied by the caller (typically the
    /// upload's filename).
    pub key: String,
}

impl StageEvent {
    /// Build a [`StageEvent`] with `percent` derived from `stage` so
    /// the two fields cannot disagree. This is the canonical way to
    /// construct one — `percent` is a public field for ergonomics on
    /// the JS side, but direct struct-initialization with a mismatched
    /// `percent` is caller-side responsibility.
    #[must_use]
    pub const fn new(stage: Stage, index: usize, total: u32, key: String) -> Self {
        Self {
            stage,
            percent: stage.percent_complete(),
            index,
            total,
            key,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Stage, StageEvent};

    #[test]
    fn percent_complete_is_monotone_per_stage() {
        assert!(Stage::Decode.percent_complete() < Stage::Frame.percent_complete());
        assert!(Stage::Frame.percent_complete() < Stage::Encode.percent_complete());
        assert_eq!(Stage::Encode.percent_complete(), 100);
    }

    #[test]
    fn new_derives_percent_from_stage() {
        let e = StageEvent::new(Stage::Frame, 2, 10, "photo.jpg".into());
        assert_eq!(e.percent, Stage::Frame.percent_complete());
        assert_eq!(e.index, 2);
        assert_eq!(e.total, 10);
        assert_eq!(e.key, "photo.jpg");
    }

    #[test]
    fn stage_serializes_as_lowercase_string() {
        // The WASM bridge depends on the lowercase encoding so JS-side
        // consumers can keep the existing "decode" | "frame" | "encode"
        // discriminant. Pin it here so an accidental rename never
        // crosses the FFI silently.
        assert_eq!(serde_json::to_string(&Stage::Decode).unwrap(), "\"decode\"");
        assert_eq!(serde_json::to_string(&Stage::Frame).unwrap(), "\"frame\"");
        assert_eq!(serde_json::to_string(&Stage::Encode).unwrap(), "\"encode\"");
    }
}
