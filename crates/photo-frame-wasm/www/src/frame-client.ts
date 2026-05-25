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
 */

import type { EncodedReply, PreparedReply, WorkerReply, WorkerRequest } from './frame-worker';
import { createRequestIdAllocator, exchange, type MessageTarget } from './lib/worker-channel';

/**
 * Frame theme — pair of border colour and caption text colour.
 * Mirror of `FrameTheme::label()` on the Rust side.
 */
export type FrameTheme = 'paper' | 'ink';

/**
 * Caption arrangement. Mirror of `CaptionLayout::label()` on the Rust
 * side — `edges` is the four-corner liit-style row; `centered` joins
 * each row with `·` and centres it.
 */
export type CaptionLayout = 'edges' | 'centered';

/** Frame-only options consumed by [`render_pixels`] (no quality). */
export type FrameOptionsForPrepare = {
  theme: FrameTheme;
  layout: CaptionLayout;
  show_meta: boolean;
  max_long_edge: number | null;
};

/** Full pipeline options consumed by `frame_batch` (atomic encode). */
export type PipelineOptions = FrameOptionsForPrepare & {
  jpeg_quality: number;
};

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
  frameOptions: FrameOptionsForPrepare,
): Promise<PreparedPixels> => {
  const transferred = bytes.slice();
  const reply = await exchange<WorkerRequest & { requestId: number }, WorkerReply, PreparedReply>(
    target,
    {
      kind: 'prepare',
      requestId,
      bytes: transferred,
      frameOptions,
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
 */
export const preparePixels = (
  bytes: Uint8Array,
  frameOptions: FrameOptionsForPrepare,
): Promise<PreparedPixels> => _preparePixelsOn(getWorker(), allocRequestId(), bytes, frameOptions);

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
 */
export const generateThumbnailBlob = async (
  bytes: Uint8Array,
  options: Omit<FrameOptionsForPrepare, 'max_long_edge'>,
  longEdge: number,
  quality = 70,
): Promise<Blob> => {
  const pixels = await preparePixels(bytes, { ...options, max_long_edge: longEdge });
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
