# Baseline performance report

The Phase B goal is to turn Phase A measurements into Phase C and D decisions.
Future pull requests can cite the evidence and distinguish it from estimates.

Divan records stage and fixture wall-clock measurements in the following file.

```text
artifacts/bench/runtime/20260525T001511Z/divan.txt
```

The dhat heap profiles cover orientation values 1 and 8, respectively.

```text
artifacts/bench/runtime/20260525T001511Z/dhat-IMG_3936.json
artifacts/bench/runtime/20260525T001511Z/dhat-IMG_3940.json
```

The workspace commit was `59d3a84`, after Phase A merged.
Measurements used `target/release`.
The supported Rust version floor was 1.78.
The host ran Linux 6.8.0-117 on `x86_64`.

## 1. Stage wall-clock measurements

The fixture is the Nikon Z 5 landscape image with 24 megapixels.
Its identifier is `real_z5_landscape_a_24mp`.
Its dimensions are 6016 by 4016 pixels.
Its Exchangeable Image File Format orientation is 1.
The following times are medians.

Decode from `from_bytes` took 268 milliseconds, or 22 percent of the pipeline.
Frame composition without resizing took 131 milliseconds, or 10 percent.
Encoding at quality 92 took 819 milliseconds, or 66 percent.
Decode, frame, and encode together took 1218 milliseconds.
The full pipeline took 1245 milliseconds, or 100 percent.
The difference represents about 2 percent overhead.

Encoding is the largest stage at the default quality of 92.
Decoding costs about a third of encoding.
Composition without downscaling costs around an eighth of pipeline time.

### Orientation rotation cost

The `real_z5_portrait_rot8_24mp` fixture uses `IMG_3940.JPG`.
Its Exchangeable Image File Format orientation is 8.
The decoder applies a 90-degree counterclockwise pixel-shuffle rotation.
The rotation follows the `JPEG` inverse discrete cosine transform.

The `real_z5_landscape_a` fixture with orientation 1 decoded in 268 milliseconds.
The `real_z5_landscape_b` fixture with orientation 1 decoded in 225 milliseconds.
The `real_z5_portrait_rot8` fixture with orientation 8 took 357 milliseconds.

Rotation adds about 110 milliseconds over the mean landscape time of 247.
That represents a 44 percent increase.
The dhat data records about 60 megabytes more total allocation for orientation 8.
The totals are 596 megabytes for orientation 8 and 535 for orientation 1.
The original report associates the increase with an extra rotated destination.
That destination is a 92-megabyte `RGBA` buffer.

### Synthetic and real-world fixtures

The 24-megapixel xorshift-noise fixture decodes at 44 megapixels per second.
Real-world fixtures decode at about 100 megapixels per second.
The report describes synthetic `JPEG` entropy as reaching its upper bound.
It attributes extra decoder work to the entropy.
Use synthetic fixtures for regression sensitivity.
Use real fixtures for absolute target measurements.

## 2. Allocation profile

The dhat-rs profile covers `IMG_3936.JPG` with 24 megapixels and orientation 1.

Total lifetime allocation was 535,732,918 bytes across 648 blocks.
The report expresses that total as 511 megabytes.
The total includes every allocation during the run.
Peak allocation at `t-gmax` was 351,768,576 bytes across 109 blocks.
The report expresses that concurrent high-water mark as 336 megabytes.
End allocation at `t-end` was 58,447 bytes across 45 blocks.
The end value represents memory remaining after the pipeline.

### Six largest single allocations

Each selected allocation occupies one block.
The report selected them from the heap snapshot with the following command.

```sh
jq '.pps | sort_by(-.tb) | .[:6]'
```

Release builds strip backtraces under `--release`.
The following attribution comes from source reading rather than dhat callsites.
The report compares allocation sizes with image dimensions.

The 119-megabyte allocation likely belongs to `frame::render::compose_canvas`.
The canvas buffer appears at `render.rs:90`.
The allocation uses `RgbaImage::from_pixel(canvas_w, canvas_h, …)`.
The photo and golden-ratio border make the canvas larger than 92 megabytes.

The first 92-megabyte allocation likely comes from `img.into_rgba8()`.
The cited locations are `decode::lib.rs:81/99`.
A 6016-by-4016 image in `RGBA8` requires exactly 96.6 megabytes.
The report attributes the allocation size to Rust vector rounding.

The second 92-megabyte allocation likely copies the decoder's `RGBA` buffer.
The cited function is `frame::render::pixels_to_rgba_image`.
The copy uses `.to_vec()` at `render.rs:55`.

The 89-megabyte allocation likely belongs to image-crate during `JPEG` decode.
Possible uses include a discrete cosine transform plane or staging.
The allocation resembles the photo size, with a stride-related difference.

The 69-megabyte allocation likely comes from `drop_alpha`.
The cited location is `encode::lib.rs:110`.
A 6016-by-4016 image in `RGB8` requires exactly 72.5 megabytes.

The 22-megabyte allocation likely reflects `JPEG` output buffer growth.
The final framed `JPEG` measures about 10 to 15 megabytes.
The `Vec` doubles its capacity to 22 megabytes.

The data supports three full-image buffers held at once by the renderer.
They're the decode output, the render's `.to_vec()` clone, and the canvas.
Encoding adds a fourth buffer for the `RGB8` alpha-drop conversion.
The pattern matches the architectural claim in Phase A's `BENCHMARKS.md`.

## 3. Hotspot ranking

The ranking combines wall-clock and allocation data for Phase C and D.

### Rank 1 encoder quality 92

Encoding uses 66 percent of pipeline time, or 819 milliseconds.
It allocates one 69-megabyte `RGB` buffer plus `JPEG` output.
Phase D includes D3 encoder tuning for Huffman optimization and progressive output.
A future `zune-jpeg` encoder could offer another option.
The existing `zune-jpeg` implementation only decodes.

### Rank 2 redundant copy at frame entry

Copying 92 megabytes accounts for about 5 milliseconds in `frame`.
The operation allocates and discards another 92 megabytes.
Phase C item C1 uses the existing zero-copy `Pixels::into_parts()` handoff.
The earlier design hadn't yet used that method.

### Rank 3 alpha drop at encode entry

The `drop_alpha` loop takes about 12 milliseconds and allocates 69 megabytes.
Phase C item C2 would remove an unnecessary `into_rgba8()` conversion.
The conversion occurs at `decode/lib.rs:81,99` when the source is `RGB`.

### Rank 4 decode with orientation 1

Decode takes 268 milliseconds, or 22 percent of pipeline time.
The `JPEG` inverse discrete cosine transform dominates decode time.
Phase D item D1 considers replacing the decoder with `zune-jpeg`.
The baseline proposal describes that decoder as a fast pure-Rust option.

### Rank 5 orientation 8 rotation

Portrait decoding costs 44 percent more and allocates 92 megabytes more.
Phase C item C2 considers rotating pixels during composition.
The alternative could replace the image crate's allocating `apply_orientation`.

### Rank 6 canvas composition

Composition forms part of the 131-millisecond frame stage.
It allocates 119 megabytes.
Phase C item C4 considers row-parallel composition through Rayon.
The proposed method is `par_chunks_mut`.
Consider C4 only if composition remains measurable after C1 and C2.

### Rank 7 resize with Lanczos3

Resizing takes 440 milliseconds in isolation.
Only the `sns` preset triggers resizing, outside the default pipeline.
The memory signal is unavailable.
Phase C item C3 evaluates filters using the structural similarity index.
Defer that evaluation until separate measurements cover the `sns` preset.

## 4. Decisions

### Adopt changes without new dependencies

Order the Phase C changes by their expected benefit.
C1 introduces a zero-copy `Pixels` handoff in the following source files.

```text
crates/photo-frame/src/pipeline.rs
crates/photo-frame-frame/src/{lib.rs,render.rs}
```

The existing method is `Pixels::into_parts()`.
Its source location is `crates/photo-frame-types/src/pixels.rs:152`.
C1 removes 92 megabytes of allocation per 24-megapixel photo.
The expected wall-clock improvement is about 5 milliseconds in isolation.
C1 also enables C2.

C2 uses `RGB8` throughout the pipeline.
The refactor adds a `PixelFormat` tag to `Pixels`.
Decode returns the native format.
Encoding skips `drop_alpha` when the input is already `RGB8`.
Direct `ab_glyph` blending replaces caption text drawing.
That replacement removes the need for the `imageproc` `text` feature.
C2 removes one 92-megabyte allocation and one 69-megabyte allocation.
The expected direct wall-clock improvement is about 5 to 15 milliseconds.
The change frees the canvas allocation cost.
It also reduces composition memory copying by 25 percent.

### Defer decisions pending measurements

C3 evaluates resize filters using the structural similarity index.
The relevant `sns` path sets `max_long_edge=2048`.
The default-options benchmark places resizing outside the critical path.
Run a separate `just bench` invocation that forces `sns` before deciding.

C4 considers row-parallel composition.
Composition uses about 10 percent of pipeline time.
The expectation is less than 5 percent after C1 and C2.
Measure again before deciding.

D1 considers replacing the decoder with `zune-jpeg`.
Decode ranks fourth at 22 percent of pipeline time.
After C2, the replacement could return `RGB8` natively.
That order avoids an immediate conversion to `RGBA8`.

D3 considers encoder tuning, the largest wall-clock cost.
The image crate's `JpegEncoder` lacks progressive mode controls in version 0.25.
It also lacks Huffman optimization controls.
The `deny.toml` contract excludes `mozjpeg`.
One Phase D option depends on a newer write interface.
Try `image::codecs::jpeg::JpegEncoder` with `progressive` only if supported.
Another option depends on future `zune-jpeg` encoder support.
The existing implementation only decodes.

### Changes outside the proposal

Caption rendering has a small absolute cost and an unfavorable risk-benefit ratio.
C4 alone is premature before C1 and C2.
Preserve the `deny.toml` contract that excludes C dependencies.

## 5. Proposed pull-request sequence

```text
C1 (zero-copy Pixels handoff)         ── PR-C1
    └─ unblocks C2's PixelFormat refactor
C2 (RGB8 throughput + ab_glyph blend) ── PR-C2
    └─ unblocks D1 (zune-jpeg can return RGB8 directly)
D1 (decode → zune-jpeg)                ── PR-D1
    └─ measure: did orientation=8 cost halve? if not, investigate
re-measure C3/C4/D3 needs              ── decide point, possibly empty
```

`PR-C2` is the largest individual pull request, with a bounded scope.
After `PR-C2`, run dhat again with `IMG_3936.JPG`.
The success criterion is a drop from 511 to about 250 megabytes.
One large buffer, the canvas, should remain.
The criterion covers the Phase C and D sequence.

## 6. Questions outside the report

### Attribution to source lines

Release builds strip backtraces.
Source reading attributes the 92-, 119-, and 69-megabyte allocations.
The attribution doesn't come from dhat callsites.
If C1 or C2 changes the expected layout, run dhat again with `-g`.
Use `profile.release.debug = true` for one pull request to confirm attribution.

### Preset-specific resize cost

The benchmark uses default options.
Phase C3 needs a separate run with `PipelineOptions::from_preset(Sns)`.

### Allocation profile for `wasm32`

dhat doesn't work on `wasm32`.
Phase F needs another tool.
Options include `wee_alloc` statistics or browser DevTools heap snapshots.
