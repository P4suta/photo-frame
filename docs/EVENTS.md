# Observability events — specification

Every `tracing::{info,warn,error,debug,trace}!` invocation in the
workspace emits a structured event with a stable `target` field. This
document is the **contract**: every event listed below is observable
under all production deployments (CLI with `--log-format json`,
the WASM bundle's tracing callback, future telemetry pipelines).

If you add an event, add a row here and follow the naming convention.
If you remove or rename one, treat it as a breaking change to the
observability contract.

## Naming convention

```
<crate>.<stage>.<concept>
```

- `crate`  — short name (`decode`, `frame`, `encode`, `pipeline`, `cli`, `wasm`)
- `stage`  — internal pipeline phase (`exif`, `orientation`, `format`, `io`, …)
- `concept` — what happened (`applied`, `absent`, `exhausted`, `truncated`, …)

Examples: `decode.exif.datetime.exhausted`, `decode.orientation.applied`,
`wasm.frame.options_invalid`.

## Severity guidance

- `error` — the operation failed and the caller will see an `Err`. Always paired with the `Err(...)` site.
- `warn`  — the operation continues, but a fallback or recovery happened that operators may want to know about.
- `info`  — high-level pipeline transitions: start/finish, configuration choices.
- `debug` — fallback paths, format detection details, branch decisions.
- `trace` — fine-grained traces (per-tag presence, per-row processing).

## Spans

Top-level entry points open INFO spans whose names are stable and
whose fields are filled in over the span's lifetime via
`Span::current().record(...)`. Consumers can rely on:

| Span name        | Crate    | Fields populated by close                                                       |
| ---------------- | -------- | ------------------------------------------------------------------------------- |
| `decode`         | decode   | `input_bytes`, `format`, `width`, `height`, `exif_present`                      |
| `heif_decode`    | decode   | `bytes`, `width`, `height`, `stride`                                            |
| `frame_render`  | frame    | `photo_width`, `photo_height`, `canvas_width`, `canvas_height`, `caption_visible` |
| `encode_jpeg`   | encode   | `width`, `height`, `quality`, `output_bytes`                                    |
| `pipeline`      | facade   | `input_bytes`, `output_bytes`                                                    |
| `batch_one`     | facade   | `input_bytes`, `elapsed_ms`                                                      |
| `run`           | cli      | `inputs`, `preset`, `jobs`, `strict`                                              |
| `process_one`   | cli      | `inputs`, `input`, `output`                                                      |
| `wasm_frame`    | wasm     | `input_bytes`, `output_bytes`                                                    |
| `wasm_frame_batch` | wasm  | `total`, `succeeded`, `failed`                                                   |

## Event catalogue

| target | level | crate | fields | when emitted |
| ------ | ----- | ----- | ------ | ------------ |
| `decode.format.unsupported`           | debug | decode | `detected` (image::ImageFormat) | `image::guess_format` recognised a format whose decoder we don't compile in (AVIF, HDR, …); treated as Unknown |
| `decode.orientation.absent`           | trace | decode | —                            | EXIF Orientation tag not present at all |
| `decode.orientation.unspecified`      | trace | decode | —                            | EXIF Orientation tag = 0 (caller said "unspecified") |
| `decode.orientation.overflow`         | warn  | decode | `raw` (u32)                  | EXIF Orientation value doesn't fit in u8 — corrupt EXIF |
| `decode.orientation.unknown`          | warn  | decode | `raw` (u8 → enum miss)       | EXIF Orientation value in 1..255 but not in 1..=8 |
| `decode.orientation.applied`          | debug | decode | `raw`, `applied`             | Orientation successfully resolved + applied |
| `decode.exif.parsed`                  | implicit (Parsed variant in `ExifReadOutcome`) | decode | — | EXIF segment parsed; downstream extractors run |
| (anonymous warn in exif.rs)           | warn  | decode | `error`                      | EXIF segment present but failed to parse |
| `decode.exif.camera.absent`           | debug | decode | —                            | Neither Make nor Model EXIF tag present |
| `decode.exif.lens.absent`             | debug | decode | —                            | Neither LensMake nor LensModel EXIF tag present |
| `decode.exif.exposure.absent`         | debug | decode | —                            | No exposure facts present (focal/aperture/shutter/iso all missing) |
| `decode.exif.datetime.exhausted`      | debug | decode | `candidates`                 | All three datetime candidate tags (Original/Digitized/DateTime) missing or unparsable |
| `decode.heif.stride_padding`          | debug | decode (feature=heif) | `stride`, `packed_row` | libheif plane has alignment padding; row-by-row repack happens |
| `decode.heif.truncated`               | warn  | decode (feature=heif) | `expected_rows`, `got_rows` | libheif plane data shorter than declared stride*height |
| `wasm.options_invalid`                | error | wasm   | `error`                      | serde-wasm-bindgen failed to deserialise the JS-side options object |
| `wasm.theme_invalid`                  | error | wasm   | `theme`                      | JS-side theme string didn't match `paper` or `ink` |
| `wasm.layout_invalid`                 | error | wasm   | `layout`                     | JS-side layout string didn't match `edges` or `centered` |
| `wasm.frame_batch.done`               | info  | wasm   | `total`, `succeeded`, `failed` | Batch finished (one event per `frame_batch` call) |
| `cli.batch.started`                   | info  | cli    | `inputs`, `jobs`, `strict`   | Batch run kicked off |
| `cli.batch.item_done`                 | info  | cli    | `input`, `output`, `elapsed_ms` | A single input finished framing and was written to disk |
| `cli.batch.summary`                   | info  | cli    | `processed`, `ok`, `failed`, `total_ms`, `avg_ms`, `speedup`, `jobs` | Continue-on-failure run summary (mirrors the stderr text) |
| (anonymous error in lib.rs)           | error | wasm   | `chain`                      | Pipeline failed; the full source chain rendered as one string before JsError construction |

Anonymous events (no `target:` override) inherit the file path as
target, e.g. `photo_frame_decode::exif`. They are listed for
completeness but should be migrated to the dotted-name convention
when next touched.

## Error events (paired with `Err(...)` sites)

Every error variant the workspace returns from a public `fn` has its
`Err(...)` construction co-located with a `tracing::error!` event
carrying:

  - `error.code`     — the miette `#[diagnostic(code = ...)]`
  - `error.category` — `Categorize::category().label()`
  - `error.source`   — the wrapped error's `Display` if any

Currently this is enforced by code review (and the docs/EVENTS.md
update gate); a future Phase could automate it via a custom clippy
lint or `tracing-error::SpanTrace`.

## Consumer reference

### CLI

  - `photo-frame --log-format pretty`  (default)
  - `photo-frame --log-format json`    (one JSON event per line on stderr)
  - `photo-frame --log-format compact` (single-line, no ANSI)
  - `PHOTO_FRAME_LOG="decode=debug,wasm=info" photo-frame ...` — per-target filtering

### WASM

  The bundle installs `tracing-wasm` on init; events surface in
  the browser console with the structured fields preserved. A future
  Phase will add a `window.addEventListener('photo-frame-event', …)`
  hook that re-emits each event as a `CustomEvent` so a UI panel can
  tail the workspace without parsing the console.

## See also

  - `docs/DEVELOPMENT.md` (Phase 6) — how to read these in dev
  - `crates/photo-frame-types/src/category.rs` — `Category` enum + `Categorize` trait
  - `crates/photo-frame-cli/src/main.rs` — the structured-panic hook + log-format wiring
