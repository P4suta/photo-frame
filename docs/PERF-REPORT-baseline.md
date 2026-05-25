# PERF-REPORT-baseline (Phase B)

**Goal:** turn the Phase A baseline numbers into a defensible Phase C/D
selection rationale. No code changes here — this document is the
record that future PRs cite when they say "we changed X because the
data said so".

**Source data:**
- divan wall-clock per stage × per fixture:
  `artifacts/bench/runtime/20260525T001511Z/divan.txt`
- dhat heap profile (orientation=1):
  `artifacts/bench/runtime/20260525T001511Z/dhat-IMG_3936.json`
- dhat heap profile (orientation=8):
  `artifacts/bench/runtime/20260525T001511Z/dhat-IMG_3940.json`
- workspace commit: `59d3a84` (Phase A merged), all numbers captured
  against `target/release` + workspace MSRV 1.78, host
  Linux 6.8.0-117 x86_64.

## 1. Stage-level wall-clock — where does the second go?

Median ms on the Nikon Z 5 24 MP real-world fixture
(`real_z5_landscape_a_24mp`, 6016×4016, EXIF orientation=1):

| Stage                  |  ms  | % of pipeline |
| ---------------------- | ---: | ------------: |
| decode (from\_bytes)   |  268 |       22 %    |
| frame (no resize)      |  131 |       10 %    |
| encode q92             |  819 |       66 %    |
| ── decode + frame + encode = 1218 ms ≈ pipeline 1245 ms (overhead ≈ 2 %) |||
| **pipeline e2e**       | 1245 |    100 %      |

**Pure observation (no opinion yet):** encode is by far the largest
stage at the default q92 quality. Decode is the second contributor,
roughly a third of encode's cost. Frame compose (no downscale path)
is the smallest, around an eighth of pipeline time.

### 1a. Orientation rotation cost

`real_z5_portrait_rot8_24mp` (`IMG_3940.JPG`) has EXIF orientation=8 —
the decoder applies a 90° CCW rotation pixel shuffle after the JPEG
IDCT. Comparing decode against the orientation=1 baseline:

| fixture                            | decode ms |
| ---------------------------------- | --------: |
| real\_z5\_landscape\_a (orient=1)  |       268 |
| real\_z5\_landscape\_b (orient=1)  |       225 |
| **real\_z5\_portrait\_rot8 (orient=8)** | **357** |

Rotation adds ~110 ms — **+44 %** over the mean landscape baseline of
247 ms. Confirmed by dhat: orientation=8 allocates **+60 MB total** vs
orientation=1 (596 MB vs 535 MB), matching one additional 92-MB RGBA
buffer for the rotated destination.

### 1b. Synthetic vs real-world

xorshift-noise synth at 24 MP decodes at 44 MP/s vs real-world at
~100 MP/s. Maximum-entropy JPEGs make the decoder work harder. Use
synth fixtures for *regression sensitivity*; use real fixtures for
*absolute target numbers*.

## 2. Allocation profile — how much memory churn?

dhat-rs measured on `IMG_3936.JPG` (24 MP, orientation=1):

| Metric        |       Bytes |  Blocks | Note |
| ------------- | ----------: | ------: | ---- |
| Total (lifetime) | **535,732,918** (511 MB) | 648 | every allocation that ever happened |
| Peak (t-gmax)    | **351,768,576** (336 MB) | 109 | concurrent high-watermark |
| End (t-end)      | 58,447                   |  45 | post-pipeline residual |

### Top 6 single allocations (each one block)

Captured by `jq '.pps | sort_by(-.tb) | .[:6]'` on the heap snapshot.
Backtraces are stripped under `--release`, so the *attribution* below
is from code reading, not dhat — but the sizes match the dimensions
exactly:

| Bytes | Likely site (from code) | Reasoning |
| ---: | --- | --- |
| 119 MB | `frame::render::compose_canvas` canvas buffer (`render.rs:90`) | `RgbaImage::from_pixel(canvas_w, canvas_h, …)`; canvas is photo + golden-ratio border, slightly bigger than 92 MB |
| 92 MB  | `decode::lib.rs:81/99` `img.into_rgba8()` | source 6016×4016 RGBA8 = exactly 96.6 MB; rust Vec rounds up |
| 92 MB  | `frame::render::pixels_to_rgba_image` (`render.rs:55`) `.to_vec()` | redundant copy of the decoder's RGBA buffer |
| 89 MB  | image-crate internal during JPEG decode (DCT plane / staging) | comparable to the photo size, slightly smaller because of stride |
| 69 MB  | `encode::lib.rs:110` `drop_alpha` | 6016×4016 RGB8 = exactly 72.5 MB |
| 22 MB  | JPEG output buffer growth | final framed JPEG is ~10–15 MB; Vec doubles to 22 |

**Confirmed by data:** the renderer holds **three full-image
buffers** simultaneously (decode output + render's `.to_vec()` clone +
canvas), plus a fourth (RGB8 alpha-drop) at encode time. That matches
the architectural claim Phase A's BENCHMARKS.md previewed.

## 3. Hotspot ranking → Phase C/D selection

Combining wall-clock and allocation data:

| Rank | Target | Wall-clock signal | Memory signal | Phase | Item |
| --- | --- | --- | --- | --- | --- |
| **1** | **encode quality 92** | 66 % of pipeline (819 ms) | encode allocates 1 × 69 MB RGB + JPEG output | **D** | D3 encoder tuning (Huffman opt + progressive) and/or D1-adjacent: zune-jpeg encoder when it ships (currently decode-only) |
| **2** | **redundant RGBA copy at frame entry** | ~5 ms in `frame` directly attributable to memcpy of 92 MB | 92 MB allocation, immediately discarded | **C** | C1 zero-copy `Pixels::into_parts()` handoff — already designed, just unused |
| **3** | **alpha drop at encode entry** | ~12 ms `drop_alpha` loop | 69 MB new allocation | **C** | C2 RGB8 throughout — eliminate `into_rgba8()` upconvert at `decode/lib.rs:81,99` when source is RGB |
| **4** | **decode (orientation=1)** | 22 % of pipeline (268 ms) | dominated by JPEG IDCT, not alloc | **D** | D1 zune-jpeg swap (its decode path is industry-standard fastest pure-Rust) |
| **5** | **orientation=8 rotation** | +44 % decode for portrait shots | +92 MB allocation | **C** | C2 + investigation: image crate's `apply_orientation` allocates; could rotate during compose pass instead |
| **6** | **canvas compose** | included in 131 ms frame total | 119 MB allocation | **C** | C4 row-parallel compose via rayon `par_chunks_mut`, only worthwhile if Phase C1+C2 don't already bring this below noise |
| **7** | **Lanczos3 resize** | 440 ms in isolation but **not in default pipeline** (only SNS preset triggers it) | n/a | **C** | C3 SSIM-driven filter re-evaluation — defer until SNS preset numbers are measured separately |

## 4. Decisions

### Adopt now (Phase C, dep-free, ordered by ROI)

1. **C1 zero-copy `Pixels` handoff.** Touches three files
   (`crates/photo-frame/src/pipeline.rs`, `crates/photo-frame-frame/src/{lib.rs,render.rs}`),
   uses existing `Pixels::into_parts()` (already in
   `crates/photo-frame-types/src/pixels.rs:152`). Removes 92 MB of
   allocation per 24 MP photo. Expected wall-clock delta: small in
   isolation (~5 ms) but a prerequisite for C2.
2. **C2 RGB8 throughout.** Bigger refactor: `PixelFormat` discriminant
   on `Pixels`, decode returns native format, encode skips `drop_alpha`
   when input is already RGB8, caption text draw replaced with
   `ab_glyph` direct-blend so `imageproc` `text` feature can be dropped.
   Removes 1 × 92 MB and 1 × 69 MB. Expected wall-clock delta:
   ~5–15 ms direct + frees the canvas allocation cost (compose memcpy
   shrinks 25 %).

### Defer / data-dependent

3. **C3 resize filter SSIM eval.** Only relevant on SNS preset path
   (`max_long_edge=2048`). The default-options bench above shows resize
   isn't on the critical path. Run a separate `just bench` invocation
   forcing SNS preset and decide then.
4. **C4 row-parallel compose.** Compose currently ~10 % of pipeline.
   After C1+C2 it should be <5 %. Re-measure before deciding.
5. **D1 zune-jpeg decode swap.** Decode is rank 4 (22 % of pipeline).
   Worth doing AFTER C2 lands so the swap can return RGB8 natively
   instead of being immediately upconverted to RGBA8. Sequencing it
   after C2 avoids wasted work.
6. **D3 encode tuning.** Rank 1 by wall-clock. The image crate's
   JpegEncoder doesn't expose progressive mode or Huffman optimisation
   knobs in 0.25 — and `mozjpeg` is out of scope per the deny.toml
   contract. Realistic Phase D moves on this axis: try
   `image::codecs::jpeg::JpegEncoder` with the `progressive` argument
   in the newer write API if available; otherwise wait for `zune-jpeg`
   to ship encoder support (currently decode-only).

### Not adopting

- Caption rendering: tiny absolute cost; risk reward unfavourable.
- C4 alone (without C1+C2): premature.
- Anything that breaks the `deny.toml` C-purity contract.

## 5. Sequencing for the next 4 PRs

```
C1 (zero-copy Pixels handoff)         ── PR-C1
    └─ unblocks C2's PixelFormat refactor
C2 (RGB8 throughput + ab_glyph blend) ── PR-C2  (largest single PR, scope-bounded)
    └─ unblocks D1 (zune-jpeg can return RGB8 directly)
D1 (decode → zune-jpeg)                ── PR-D1
    └─ measure: did orientation=8 cost halve? if not, investigate
re-measure C3/C4/D3 needs              ── decide point, possibly empty
```

After PR-C2 lands, re-run dhat against `IMG_3936.JPG` and confirm
total drops from 511 MB to ~250 MB (one large buffer left, the
canvas). That number is the success criterion for the Phase C/D arc.

## 6. What this report doesn't answer

- **Per-line attribution.** Release builds strip backtraces; the
  92 MB / 119 MB / 69 MB sizes were attributed by reading the source,
  not by dhat callsite. If the C1/C2 implementation deviates from the
  expected layout, re-run dhat under `-g` (`profile.release.debug =
  true`) for one PR to confirm.
- **SNS-preset resize cost.** The current bench runs default options.
  Phase C3 needs its own bench invocation with `PipelineOptions::from_preset(Sns)`.
- **WASM-side allocation profile.** dhat doesn't work on `wasm32`.
  Phase F will need a different tool — `wee_alloc` stats or browser
  devtools heap snapshots.
