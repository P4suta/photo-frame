//! Span-timing flamegraph capture, gated by the `trace` cargo feature.
//!
//! The pipeline already emits a `tracing::span` for every stage
//! (`pipeline.rs:42`, decode `lib.rs`, `frame/render.rs:13`,
//! `encode/lib.rs`). Until this module landed those spans had no
//! consumer — they were observable via any `tracing-subscriber` but
//! nobody actually rendered them into a flamegraph.
//!
//! Workflow:
//!
//! ```ignore
//! let _flame = photo_frame::trace::flame_guard("/tmp/trace.folded")?;
//! // run the pipeline
//! drop(_flame); // forces the layer to flush before exit
//! ```
//!
//! Then turn the `.folded` file into an SVG with
//! `inferno-flamegraph < trace.folded > trace.svg`
//! (the `cargo:inferno` tool listed in `mise.toml`).

use std::path::Path;

use tracing_flame::{FlameLayer, FlushGuard};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Registry;

/// Install a `tracing-subscriber` that writes span enter/exit timings
/// into `path`. The returned guard must be held for the duration of
/// the workload; dropping it flushes any buffered events.
///
/// # Errors
/// Bubbles up the underlying file-open error from `tracing-flame`.
pub fn flame_guard<P: AsRef<Path>>(
    path: P,
) -> Result<FlushGuard<std::io::BufWriter<std::fs::File>>, tracing_flame::Error> {
    let (layer, guard) = FlameLayer::with_file(path)?;
    Registry::default().with(layer).init();
    Ok(guard)
}
