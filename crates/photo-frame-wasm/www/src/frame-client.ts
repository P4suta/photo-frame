/**
 * Worker-backed façade for the WASM exports.
 *
 * The UI never calls `render_pixels` / `encode_jpeg` directly — every
 * non-trivial WASM call goes through the singleton worker below, so the
 * main thread never blocks on a 24 MP decode or a quality-slider encode.
 *
 * Three concerns concentrate here:
 *
 *   1. **Worker singleton** — `getWorker()` lazily spawns one Worker for
 *      the page lifetime. Multiple workers would re-initialise the WASM
 *      module and burn ~1.6 MB extra per instance.
 *   2. **Request/response correlation** — every call gets a monotonically
 *      increasing `requestId`; the listener resolves only when the reply
 *      ID matches and rejects stale replies silently. A fast slider drag
 *      can flood the worker with N encode requests; only the latest
 *      reply matters and the rest evaporate.
 *   3. **Buffer transfer** — request payloads ride as transferables so a
 *      cached 100 MB RGBA isn't memcpied per encode. Callers that need
 *      to keep the buffer (e.g. the App's cached pixels) pass a fresh
 *      `slice()` per call.
 */

import type { WorkerReply, WorkerRequest } from './frame-worker';

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

let nextRequestId = 1;
const allocRequestId = (): number => {
  const id = nextRequestId;
  nextRequestId += 1;
  return id;
};

/**
 * Issue one worker request whose reply identifies itself with the same
 * `requestId`. Resolves on a matching success reply, rejects on a
 * matching error reply. Replies for *other* IDs are ignored — letting
 * the slider drag flood the worker harmlessly.
 */
const exchange = <Req extends WorkerRequest & { requestId: number }, Ok extends WorkerReply>(
  request: Req,
  isOk: (reply: WorkerReply) => reply is Ok,
  transfer: Transferable[],
): Promise<Ok> =>
  new Promise<Ok>((resolve, reject) => {
    const worker = getWorker();
    const handler = (event: MessageEvent<WorkerReply>): void => {
      const reply = event.data;
      // Discard replies for unrelated requests (e.g. stale slider ticks).
      if (!('requestId' in reply) || reply.requestId !== request.requestId) return;
      worker.removeEventListener('message', handler);
      if (reply.kind === 'error') {
        reject(new Error(reply.message));
      } else if (isOk(reply)) {
        resolve(reply);
      } else {
        reject(new Error(`unexpected worker reply kind: ${reply.kind}`));
      }
    };
    worker.addEventListener('message', handler);
    worker.postMessage(request, transfer);
  });

// ── public API ──────────────────────────────────────────────────────────

/**
 * Decode `bytes` and render the framed RGBA8 grid in the worker. The
 * returned `rgba` is a fresh buffer (the worker transferred it back) so
 * the caller can keep it around indefinitely.
 */
export const preparePixels = async (
  bytes: Uint8Array,
  frameOptions: FrameOptionsForPrepare,
): Promise<PreparedPixels> => {
  // We send a slice of the input bytes; the caller (DropZone-owned File
  // buffer) keeps the original.
  const transferred = bytes.slice();
  const reply = await exchange(
    {
      kind: 'prepare',
      requestId: allocRequestId(),
      bytes: transferred,
      frameOptions,
    },
    (r): r is import('./frame-worker').PreparedReply => r.kind === 'prepared',
    [transferred.buffer],
  );
  return { rgba: reply.rgba, width: reply.width, height: reply.height };
};

/**
 * Encode an RGBA8 buffer at the given JPEG quality in the worker. The
 * caller passes a fresh `slice()` of its cached RGBA so the cache stays
 * intact across overlapping encode requests.
 */
export const encodeJpeg = async (
  rgba: Uint8Array,
  width: number,
  height: number,
  quality: number,
): Promise<Uint8Array> => {
  const transferred = rgba.slice();
  const reply = await exchange(
    {
      kind: 'encode',
      requestId: allocRequestId(),
      rgba: transferred,
      width,
      height,
      quality,
    },
    (r): r is import('./frame-worker').EncodedReply => r.kind === 'encoded',
    [transferred.buffer],
  );
  return reply.jpeg;
};

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
