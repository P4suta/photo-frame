//! Representative input bytes for the photo-frame pipeline benches.
//!
//! Two families, both loaded lazily once per process and shared across
//! every bench iteration:
//!
//! - **Real-world** Nikon Z 5 JPEGs at `samples/bench/IMG_*.JPG`. The
//!   workspace `.gitignore` keeps image binaries out of the repo (the
//!   project's policy: real photos are personal, large, and rarely
//!   the right shape for someone else's testing). Loaded via
//!   `env!("CARGO_MANIFEST_DIR")` so the path works regardless of the
//!   caller's CWD; **a missing file is not an error** — the loader
//!   skips it and prints a one-line warning the first time, so CI
//!   and clean clones get a synth-only run instead of a panic.
//! - **Synthetic noise** JPEGs at the megapixel counts a typical
//!   smartphone / mid-range camera / full-frame mirrorless produces.
//!   A deterministic xorshift fills RGB so the encoded file has
//!   realistic JPEG entropy — solid-colour fixtures collapse every
//!   DCT coefficient to zero and benchmark unrealistically fast.
//!
//! Each fixture is a small accessor fn backed by a private
//! [`OnceLock`]. The flat [`all`] / [`small`] helpers materialise
//! every fixture in deterministic order (real ones first if present)
//! so divan's `args = …` parameterisation has a
//! `Vec<&'static Fixture>` to iterate over. `OnceLock` keeps us on
//! workspace MSRV 1.78 — `LazyLock` would be cleaner but is only
//! stable since 1.80.

use std::fmt;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::OnceLock;

use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageEncoder, ImageReader, RgbImage};

/// A single benchmarkable input: name, dimensions, and the raw bytes
/// the decoder will be handed. The struct is the unit of bench
/// parameterisation — divan uses [`std::fmt::Display`] for the per-row label.
#[derive(Debug)]
pub struct Fixture {
    /// Stable identifier used as the bench row label and the
    /// `BENCHMARKS.md` row key. Snake-case, includes the megapixel
    /// count so the row sorts intuitively.
    pub name: &'static str,
    /// Image width in pixels (used for the megapixel-throughput
    /// derivation).
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Encoded JPEG bytes that the pipeline takes as input.
    pub bytes: Vec<u8>,
}

impl Fixture {
    /// Pixel count, as the unit divan multiplies by sample throughput to
    /// report MP/s. `u64` because `u32::MAX * u32::MAX` overflows `u32`.
    #[must_use]
    pub const fn pixel_count(&self) -> u64 {
        (self.width as u64) * (self.height as u64)
    }
}

impl fmt::Display for Fixture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

/// Try to read a JPEG out of the workspace's `samples/bench/`
/// directory. Returns `None` if the file is missing — the workspace
/// `.gitignore` excludes image binaries so this is the expected
/// state on CI and clean clones. Logs to stderr the first time so
/// the absence is visible in bench output without flooding logs.
fn try_load_real(name: &'static str, file: &str) -> Option<Fixture> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../samples/bench");
    path.push(file);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "bench-fixture: skipping `{name}` — {} not readable ({e})",
                path.display()
            );
            return None;
        },
    };
    let (width, height) = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()?;
    Some(Fixture {
        name,
        width,
        height,
        bytes,
    })
}

/// Deterministic xorshift32 — fills RGB so the encoded JPEG has
/// realistic high-frequency content. Same input always yields the
/// same output, so bench runs are reproducible.
fn synth_noise_rgb(width: u32, height: u32) -> Vec<u8> {
    let mut state: u32 = 0x1234_5678;
    let pixel_count = (width as usize) * (height as usize);
    let mut buf = Vec::with_capacity(pixel_count * 3);
    for _ in 0..(pixel_count * 3) {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        buf.push(state.to_le_bytes()[0]);
    }
    buf
}

/// Synthesise a JPEG of the given dimensions filled with deterministic
/// noise, encoded at the supplied quality.
fn synth_noise_jpeg(name: &'static str, width: u32, height: u32, quality: u8) -> Fixture {
    let rgb = synth_noise_rgb(width, height);
    let img = RgbImage::from_raw(width, height, rgb)
        .expect("synth buffer length matches w*h*3 by construction");
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, quality)
        .write_image(&img, width, height, ExtendedColorType::Rgb8)
        .expect("jpeg encode of well-formed noise image");
    Fixture {
        name,
        width,
        height,
        bytes: out,
    }
}

// ─── Real-world fixtures (Nikon Z 5, 6016×4016 native) ──────────────────
//
// IMG_3936 and IMG_3939 are landscape native (EXIF orientation=1).
// IMG_3940 is portrait via EXIF orientation=8 (90° CCW), so the
// decode path exercises the rotation pixel-shuffle step on a
// real-world buffer rather than a synthetic one. All three return
// `None` if the local samples/bench/ directory doesn't have them —
// see the module-level note on the gitignore policy.

/// Z 5 landscape (orientation=1). `None` when missing from `samples/bench/`.
#[must_use]
pub fn real_z5_landscape_a() -> Option<&'static Fixture> {
    static CELL: OnceLock<Option<Fixture>> = OnceLock::new();
    CELL.get_or_init(|| try_load_real("real_z5_landscape_a_24mp", "IMG_3936.JPG"))
        .as_ref()
}

/// Z 5 landscape (orientation=1), second sample. `None` when missing.
#[must_use]
pub fn real_z5_landscape_b() -> Option<&'static Fixture> {
    static CELL: OnceLock<Option<Fixture>> = OnceLock::new();
    CELL.get_or_init(|| try_load_real("real_z5_landscape_b_24mp", "IMG_3939.JPG"))
        .as_ref()
}

/// Z 5 portrait via EXIF orientation=8 (90° CCW) — exercises the
/// rotation pixel-shuffle path. `None` when missing.
#[must_use]
pub fn real_z5_portrait_rotated() -> Option<&'static Fixture> {
    static CELL: OnceLock<Option<Fixture>> = OnceLock::new();
    CELL.get_or_init(|| try_load_real("real_z5_portrait_rot8_24mp", "IMG_3940.JPG"))
        .as_ref()
}

// ─── Synthetic noise fixtures ──────────────────────────────────────────
//
// Always available — no filesystem dependency. Quality 85 is the
// most common JPEG quality across consumer cameras and matches the
// entropy profile real users feed the pipeline.

/// 2400×1600 (3.84 MP) — smartphone-class output.
#[must_use]
pub fn synth_4mp() -> &'static Fixture {
    static CELL: OnceLock<Fixture> = OnceLock::new();
    CELL.get_or_init(|| synth_noise_jpeg("synth_noise_4mp_2400x1600", 2400, 1600, 85))
}

/// 4240×2832 (12 MP) — mid-range mirrorless. Falls between the
/// smartphone and full-frame extremes so we can read the scaling curve.
#[must_use]
pub fn synth_12mp() -> &'static Fixture {
    static CELL: OnceLock<Fixture> = OnceLock::new();
    CELL.get_or_init(|| synth_noise_jpeg("synth_noise_12mp_4240x2832", 4240, 2832, 85))
}

/// 6016×4016 (24 MP) — matches the Z 5 native sensor. Phase B can
/// compare synthetic vs real-world at the same MP count to isolate
/// "JPEG entropy realism" from "decoder behaviour at scale".
#[must_use]
pub fn synth_24mp() -> &'static Fixture {
    static CELL: OnceLock<Fixture> = OnceLock::new();
    CELL.get_or_init(|| synth_noise_jpeg("synth_noise_24mp_6016x4016", 6016, 4016, 85))
}

/// 10000×100 — extreme aspect ratio. Exercises row-stride edge cases
/// in the resize and compose paths that square-ish fixtures miss.
#[must_use]
pub fn synth_panorama() -> &'static Fixture {
    static CELL: OnceLock<Fixture> = OnceLock::new();
    CELL.get_or_init(|| synth_noise_jpeg("synth_noise_panorama_10000x100", 10000, 100, 85))
}

/// Every available fixture, in deterministic order.
///
/// Real-world fixtures come first when present, synthetic ones after.
/// Order is fixed so historical bench rows in `BENCHMARKS.md` remain
/// comparable across phases — when a real fixture is absent its
/// column is simply empty for that run.
#[must_use]
pub fn all() -> Vec<&'static Fixture> {
    let reals = [
        real_z5_landscape_a(),
        real_z5_landscape_b(),
        real_z5_portrait_rotated(),
    ];
    let synths = [synth_4mp(), synth_12mp(), synth_24mp(), synth_panorama()];
    reals.into_iter().flatten().chain(synths).collect()
}

/// Subset for very-expensive benches (e.g. iai-callgrind under
/// Valgrind, where a 24 MP fixture would take minutes per stage).
/// All synth so it works on CI without the real fixtures present.
#[must_use]
pub fn small() -> Vec<&'static Fixture> {
    vec![synth_4mp(), synth_panorama()]
}
