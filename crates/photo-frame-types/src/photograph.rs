use crate::pixels::Pixels;
use crate::provenance::Provenance;

/// A photograph as the pipeline understands it: pixels plus the structured
/// metadata that came with them.
///
/// `Photograph` is the canonical intermediate vocabulary — the output of
/// decode, the input of frame. By the time a `Photograph` exists, the
/// pixels are already upright (orientation applied) and the metadata is
/// already parsed into structured primitives.
///
/// Like [`Pixels`], `Photograph` does **not** implement [`Clone`]. Sharing
/// a decoded photograph (the WASM cache's one job) goes through
/// `Arc<Photograph>` instead — `Arc::clone` is a refcount bump, while a
/// `derive(Clone)` would silently copy the ~100 MB pixel grid every time.
#[derive(Debug, PartialEq)]
pub struct Photograph {
    /// Decoded RGBA8 pixels, already oriented upright (EXIF orientation
    /// applied at decode time).
    pub pixels: Pixels,
    /// Structured metadata parsed from the source image (camera, lens,
    /// exposure, capture timestamp). All fields are `Option`s; an empty
    /// [`Provenance`] is valid and just means the source carried no
    /// usable metadata.
    pub provenance: Provenance,
}

impl Photograph {
    /// Compose from parts. The most common construction site is decode,
    /// which builds both halves and snaps them together here.
    #[must_use]
    pub const fn new(pixels: Pixels, provenance: Provenance) -> Self {
        Self { pixels, provenance }
    }

    /// `(width, height)` in pixels — shorthand for `self.pixels.dimensions()`.
    #[must_use]
    pub const fn dimensions(&self) -> (u32, u32) {
        self.pixels.dimensions()
    }
}

#[cfg(test)]
mod tests {
    use super::Photograph;
    use crate::pixels::Pixels;
    use crate::provenance::Provenance;

    #[test]
    fn new_round_trips_parts() {
        // `Pixels` doesn't implement `Clone` (see its docstring), so the
        // equality assertion builds an independent copy from the same
        // RGBA8 payload — the comparison is on value, not identity.
        let pixels = Pixels::from_rgba8(1, 1, vec![0, 0, 0, 0xFF]).unwrap();
        let expected = Pixels::from_rgba8(1, 1, vec![0, 0, 0, 0xFF]).unwrap();
        let photo = Photograph::new(pixels, Provenance::default());
        assert_eq!(photo.pixels, expected);
        assert!(photo.provenance.is_empty());
        assert_eq!(photo.dimensions(), (1, 1));
    }
}
