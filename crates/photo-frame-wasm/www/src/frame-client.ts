/**
 * Worker-backed façade for the WASM exports.
 *
 * The UI never calls `render_pixels` / `encode_jpeg` directly — every
 * non-trivial WASM call goes through the singleton worker below, so the
 * main thread never blocks on a 24 MP decode or a quality-slider encode.
 *
 * Two concerns concentrate here:
 *
 *   1. **Worker singleton** — `getWorker()` lazily spawns one Worker for
 *      the page lifetime. Multiple workers would re-initialise the WASM
 *      module and burn ~1.6 MB extra per instance.
 *   2. **Buffer transfer** — request payloads ride as transferables so a
 *      cached 100 MB RGBA isn't memcpied per encode. Callers that need
 *      to keep the buffer (e.g. the App's cached pixels) pass a fresh
 *      `slice()` per call.
 *
 * The request/response correlation lives in `lib/worker-channel.ts`
 * behind a `MessageTarget` seam so it can be exercised against a fake
 * target without spinning up a real Worker.
 *
 * Every type the UI speaks in for render configuration — `PipelineSpec`,
 * `FrameTheme`, `CaptionLayout`, `MetaPolicy`, `Preset` — is the
 * `tsify`-generated mirror of the Rust truth, re-exported from
 * `pkg/`. There is no hand-rolled JS-side shape for the boundary
 * contract; a drift in the Rust struct surfaces as a `bun run
 * typecheck` error here.
 */

import type {
  CaptionLayout,
  FrameStyle,
  FrameTheme,
  MetaPolicy,
  PipelineSpec,
  Preset,
} from '../pkg/photo_frame_wasm';
import type { EncodedReply, PreparedReply, WorkerReply, WorkerRequest } from './frame-worker';
import { createRequestIdAllocator, exchange, type MessageTarget } from './lib/worker-channel';

export type { CaptionLayout, FrameStyle, FrameTheme, MetaPolicy, PipelineSpec, Preset };

export type PreparedPixels = {
  rgba: Uint8Array;
  width: number;
  height: number;
};

// ── worker singleton ────────────────────────────────────────────────────

let workerInstance: Worker | null = null;
const getWorker = (): Worker => {
  workerInstance ??= new Worker(new URL('./frame-worker.ts', import.meta.url), {
    type: 'module',
  });
  return workerInstance;
};

/** Terminate the shared worker (test/teardown only). */
export const disposeWorker = (): void => {
  workerInstance?.terminate();
  workerInstance = null;
};

// ── request correlation ─────────────────────────────────────────────────

const allocRequestId = createRequestIdAllocator();

const isPreparedReply = (reply: WorkerReply): reply is PreparedReply => reply.kind === 'prepared';
const isEncodedReply = (reply: WorkerReply): reply is EncodedReply => reply.kind === 'encoded';

// ── public API ──────────────────────────────────────────────────────────

/**
 * `_preparePixelsOn` / `_encodeJpegOn` are the channel-target-injected
 * variants of the public functions below. They exist so unit tests can
 * drive the request/response shape against a fake `MessageTarget`
 * without spinning up a real Worker. Production code always goes
 * through `preparePixels` / `encodeJpeg`, which simply close over
 * `getWorker()` and the module-global request-id allocator.
 */
export const _preparePixelsOn = async (
  target: MessageTarget,
  requestId: number,
  bytes: Uint8Array,
  spec: PipelineSpec,
): Promise<PreparedPixels> => {
  const transferred = bytes.slice();
  const reply = await exchange<WorkerRequest & { requestId: number }, WorkerReply, PreparedReply>(
    target,
    {
      kind: 'prepare',
      requestId,
      bytes: transferred,
      spec,
    },
    isPreparedReply,
    [transferred.buffer],
  );
  return { rgba: reply.rgba, width: reply.width, height: reply.height };
};

export const _encodeJpegOn = async (
  target: MessageTarget,
  requestId: number,
  rgba: Uint8Array,
  width: number,
  height: number,
  quality: number,
): Promise<Uint8Array> => {
  const transferred = rgba.slice();
  const reply = await exchange<WorkerRequest & { requestId: number }, WorkerReply, EncodedReply>(
    target,
    {
      kind: 'encode',
      requestId,
      rgba: transferred,
      width,
      height,
      quality,
    },
    isEncodedReply,
    [transferred.buffer],
  );
  return reply.jpeg;
};

/**
 * Decode `bytes` and render the framed RGBA8 grid in the worker. The
 * returned `rgba` is a fresh buffer (the worker transferred it back) so
 * the caller can keep it around indefinitely.
 *
 * `spec.jpeg_quality` is ignored by the render path — encode lives in
 * `encodeJpeg`. We pass the full spec anyway so the call site doesn't
 * have to project a partial subset.
 */
export const preparePixels = (bytes: Uint8Array, spec: PipelineSpec): Promise<PreparedPixels> =>
  _preparePixelsOn(getWorker(), allocRequestId(), bytes, spec);

/**
 * Encode an RGBA8 buffer at the given JPEG quality in the worker. The
 * caller passes a fresh `slice()` of its cached RGBA so the cache stays
 * intact across overlapping encode requests.
 */
export const encodeJpeg = (
  rgba: Uint8Array,
  width: number,
  height: number,
  quality: number,
): Promise<Uint8Array> =>
  _encodeJpegOn(getWorker(), allocRequestId(), rgba, width, height, quality);

/**
 * Render a low-resolution framed JPEG of `bytes` at the caller's
 * `longEdge` cap, suitable for a batch-card thumbnail. Combines the
 * existing `preparePixels` (render + downscale) and `encodeJpeg`
 * (encode at modest quality) into a single Promise returning a
 * `Blob` the caller can hand to `URL.createObjectURL()`.
 *
 * The WASM-side photograph cache (`cached_or_decode` in `lib.rs`)
 * makes the thumbnail and the eventual full-resolution pass share
 * one decode — generating thumbnails for an N-file batch costs N
 * decodes plus 2N frame stages, not 2N decodes.
 *
 * `baseSpec` carries theme / layout / meta_policy from the user's
 * current settings; we override `max_long_edge` to clamp the thumb
 * and `jpeg_quality` to the (lower) thumb quality.
 */
export const generateThumbnailBlob = async (
  bytes: Uint8Array,
  baseSpec: PipelineSpec,
  longEdge: number,
  quality = 70,
): Promise<Blob> => {
  const thumbSpec: PipelineSpec = { ...baseSpec, max_long_edge: longEdge };
  const pixels = await preparePixels(bytes, thumbSpec);
  const jpeg = await encodeJpeg(pixels.rgba, pixels.width, pixels.height, quality);
  const buffer = new ArrayBuffer(jpeg.byteLength);
  new Uint8Array(buffer).set(jpeg);
  return new Blob([buffer], { type: 'image/jpeg' });
};

/**
 * Run a batch through the worker's `frame_batch` entry. Re-exported for
 * `App.tsx` to consume; the worker's per-item progress events arrive on
 * the same channel and the caller listens for them directly to update
 * the UI.
 */
export type { BatchItem, BatchResult, WorkerReply, WorkerRequest } from './frame-worker';
export { getWorker };

// ── boot-time preset fetch ──────────────────────────────────────────────
//
// The canonical preset table lives in Rust (`PipelineSpec::PRESETS`).
// `loadPresets()` initialises the main-thread WASM instance once and
// returns the typed `Preset[]` so the UI can build its preset segmented
// from the same source of truth that the renderer consumes. The result
// is memoised — subsequent calls hit the cache, including across
// remounts during HMR.
//
// Lives in the main thread (not the worker) because the call site is
// the UI bootstrap (`main.tsx`) and there is no benefit to round-
// tripping a 200-byte payload through `postMessage` when the WASM
// load is already cached by the browser.
//
// The WASM module is loaded via dynamic `import()` so the bundle keeps
// `pkg/photo_frame_wasm.js` in its own chunk. A static import here
// would force Vite to inline the chunk into the main entry — which it
// is also dynamically imported from the rayon worker helpers, and
// mixing the two import modes triggers the `INEFFECTIVE_DYNAMIC_IMPORT`
// build warning. Dynamic on both sides keeps the chunk split clean.

let presetsCache: readonly Preset[] | null = null;
let presetsPromise: Promise<readonly Preset[]> | null = null;

export const loadPresets = async (): Promise<readonly Preset[]> => {
  if (presetsCache) return presetsCache;
  presetsPromise ??= (async () => {
    const wasm = await import('../pkg/photo_frame_wasm');
    await wasm.default();
    presetsCache = wasm.getPresets();
    return presetsCache;
  })();
  return presetsPromise;
};
