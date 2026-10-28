# Observability event specification

Every workspace tracing invocation emits a structured event.
The event has a stable `target` field.
The invocation can use `tracing::{info,warn,error,debug,trace}!`.
The listed events form the observability contract for production deployments.
Consumers include the command-line interface with `--log-format json`.
They also include the WebAssembly tracing callback and future telemetry pipelines.

Add new events to the catalogue and follow the naming convention.
Removing or renaming an event breaks the observability contract.

## Naming convention

```text
<crate>.<stage>.<concept>
```

`crate` names a component.
Examples include `decode`, `frame`, `encode`, `pipeline`, `cli`, and `wasm`.
`stage` names an internal pipeline phase.
Examples include `exif`, `orientation`, `format`, and `io`.
`concept` describes the occurrence.
Examples include `applied`, `absent`, `exhausted`, and `truncated`.

Example identifiers include `decode.exif.datetime.exhausted`.
Other examples are `decode.orientation.applied` and `wasm.frame.options_invalid`.

## Severity guidance

Use `error` when an operation fails and the caller receives an `Err`.
Pair every error event with its `Err(...)` site.
Use `warn` when an operation continues after fallback or recovery.
Warnings describe conditions that operators may want to know about.
Use `info` for pipeline transitions and configuration choices.
Use `debug` for fallback paths, format detection, and branch decisions.
Use `trace` for details such as tag presence and row processing.

## Spans

Top-level entry points open `INFO` spans with stable names.
They record fields during each span through `Span::current().record(...)`.
Consumers can rely on the following names and fields at span closure.

The decode crate opens the `decode` span.
It records `input_bytes`, `format`, `width`, `height`, and `exif_present`.
The same crate opens `heif_decode`.
That span records `bytes`, `width`, `height`, and `stride`.

The frame crate opens `frame_render`.
It records `photo_width`, `photo_height`, and `canvas_width`.
Its remaining fields are `canvas_height` and `caption_visible`.

The encode crate opens `encode_jpeg`.
It records `width`, `height`, `quality`, and `output_bytes`.

The pipeline façade opens `pipeline` and `batch_one`.
The `pipeline` span records `input_bytes` and `output_bytes`.
The `batch_one` span records `input_bytes` and `elapsed_ms`.

The command-line interface opens `run` and `process_one`.
The `run` span records `inputs`, `preset`, `jobs`, and `strict`.
The `process_one` span records `inputs`, `input`, and `output`.

The WebAssembly crate opens `wasm_frame` and `wasm_frame_batch`.
The `wasm_frame` span records `input_bytes` and `output_bytes`.
The `wasm_frame_batch` span records `total`, `succeeded`, and `failed`.

## Event catalogue

### Decode events

The debug-level `decode.format.unsupported` event records `detected`.
Its type is `image::ImageFormat`.
It occurs when `image::guess_format` recognizes an unavailable decoder format.
Examples include `AVIF` and `HDR`.
The decoder treats the format as `Unknown`.

The trace-level `decode.orientation.absent` event reports a missing tag.
The tag is the Exchangeable Image File Format `Orientation` tag.
The trace-level `decode.orientation.unspecified` event reports tag value 0.

The warn-level `decode.orientation.overflow` event records `raw` as a `u32`.
The orientation value doesn't fit in `u8`, indicating corrupt metadata.
The warn-level `decode.orientation.unknown` event also records `raw`.
It follows a `u8` enum miss for a value in `1..255` but outside `1..=8`.
The debug-level `decode.orientation.applied` event records `raw` and `applied`.
It follows resolving and applying the orientation.

The `decode.exif.parsed` identifier represents the implicit `Parsed` variant.
The variant belongs to `ExifReadOutcome`.
Downstream extractors run after parsing.
An anonymous warn-level event in `exif.rs` records `error`.
It occurs when an Exchangeable Image File Format segment fails to parse.
The segment must be present for that event to occur.

The debug-level `decode.exif.camera.absent` event reports missing camera tags.
Neither `Make` nor `Model` is present.
The debug-level `decode.exif.lens.absent` event reports missing lens tags.
Neither `LensMake` nor `LensModel` is present.
The debug-level `decode.exif.exposure.absent` event reports missing exposure data.
Focal length, aperture, shutter speed, and photographic sensitivity are missing.
The sensitivity uses the `ISO` scale.
The debug-level `decode.exif.datetime.exhausted` event records `candidates`.
The `Original`, `Digitized`, and `DateTime` tags are all missing or unparsable.

The `heif` feature enables debug-level `decode.heif.stride_padding` events.
They record `stride` and `packed_row`.
They occur when libheif plane alignment requires repacking each row.
The feature also enables warn-level `decode.heif.truncated` events.
They record `expected_rows` and `got_rows`.
Plane data is shorter than the declared stride multiplied by height.

### Events from `photo-frame-wasm`

The error-level `wasm.options_invalid` event records `error`.
`serde-wasm-bindgen` can't deserialize the JavaScript options object.
The error-level `wasm.theme_invalid` event records `theme`.
The JavaScript theme value is neither `paper` nor `ink`.
The error-level `wasm.layout_invalid` event records `layout`.
The JavaScript layout value is neither `edges` nor `centered`.
The info-level `wasm.frame_batch.done` event occurs once per `frame_batch` call.
It records `total`, `succeeded`, and `failed`.
An anonymous error-level event in `lib.rs` records `chain` on pipeline failure.
It renders the full source chain as one string before constructing `JsError`.

### Command-line events

The info-level `cli.batch.started` event occurs when a batch run starts.
It records `inputs`, `jobs`, and `strict`.
The info-level `cli.batch.item_done` event follows each saved framed image.
It records `input`, `output`, and `elapsed_ms`.
The info-level `cli.batch.summary` event describes a continue-on-failure run.
It records `processed`, `ok`, `failed`, `total_ms`, and `avg_ms`.
Its remaining fields are `speedup` and `jobs`.
The fields match the standard error summary.

Anonymous events without a `target:` override inherit the path as target.
One example is `photo_frame_decode::exif`.
The catalogue includes anonymous events for completeness.
Migrate them to the dotted naming convention when changing their source.

## Error events paired with `Err(...)` sites

Every error variant from a public workspace `fn` has a paired event.
The `Err(...)` construction shares its site with a `tracing::error!` event.
The event carries `error.code` from miette's `#[diagnostic(code = ...)]`.
It also carries `error.category` from `Categorize::category().label()`.
When present, `error.source` carries the wrapped error's `Display` value.

Code review and the `docs/EVENTS.md` update gate enforce the pairing.
A future phase could automate the check with a custom Clippy lint.
Another option is `tracing-error::SpanTrace`.

## Consumer reference

### Command-line interface

The default is `photo-frame --log-format pretty`.
Use `photo-frame --log-format json` for one structured event per line.
The command writes events to standard error.
Use `photo-frame --log-format compact` for output on a single line.
Compact output omits American National Standards Institute escape sequences.
Filter events by target with the following command.

```sh
PHOTO_FRAME_LOG="decode=debug,wasm=info" photo-frame ...
```

### Browser consumer

The bundle installs `tracing-wasm` during initialization.
Events appear in the browser console with their structured fields preserved.
A future phase plans a `window.addEventListener('photo-frame-event', …)` hook.
The hook would emit each event again as a `CustomEvent`.
A user-interface panel could follow events without parsing the console.

## See also

[The development guide](DEVELOPMENT.md) covers reading events in Phase 6.
The [domain types](../crates/photo-frame-types/src/category.rs) define categories.
The types are the `Category` enum and `Categorize` trait.
The [entry point](../crates/photo-frame-cli/src/main.rs) wires log formats.
It also installs the structured-panic hook.
