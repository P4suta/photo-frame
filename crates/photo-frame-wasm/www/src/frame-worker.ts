/**
 * Web Worker host for every expensive WASM call.
 *
 * One worker serves three modes — `prepare`, `encode`, `batch` — so the
 * WASM module is initialised only once per page load. Single-image
 * preview lives here for the same reason large batches do: `render_pixels`
 * on a 24 MP photo takes 300–700 ms, and the main thread shouldn't pay
 * that latency.
 *
 * Protocol (every reply carries the same `requestId` the request did, so
 * the main side can ignore stale replies after a fast slider drag):
 *
 *   request:  { kind: 'prepare', requestId, bytes, frameOptions }
 *   reply:    { kind: 'prepared', requestId, rgba, width, height }
 *
 *   request:  { kind: 'encode',  requestId, rgba, width, height, quality }
 *   reply:    { kind: 'encoded', requestId, jpeg }
 *
 *   request:  { kind: 'batch',   items, options }   (no requestId — atomic)
 *   reply 1+: { kind: 'progress', index, total, key }
 *   reply:    { kind: 'done',     results }
 *
 *   reply:    { kind: 'error',   requestId | null, message }
 *
 * Byte buffers (`bytes`, `rgba`, `jpeg`) ride as transferables on both
 * the request and the reply — the main thread sends a slice it owns,
 * and gets a fresh buffer back. Slider-drag spam therefore never memcpies
 * the cached RGBA.
 */

import init, {
  encode_jpeg,
  frame_batch,
  initThreadPool,
  render_pixels,
  type Stage,
  type StageEvent,
} from '../pkg/photo_frame_wasm.js';
import type { FrameOptionsForPrepare, PipelineOptions } from './frame-client';

export type BatchItem = {
  key: string;
  bytes: Uint8Array;
};

export type BatchOkResult = {
  key: string;
  ok: true;
  result: Uint8Array;
  elapsed_ms: number;
};
export type BatchErrResult = {
  key: string;
  ok: false;
  result: string; // human-readable error chain
  elapsed_ms: number;
};
export type BatchResult = BatchOkResult | BatchErrResult;

export type PrepareRequest = {
  kind: 'prepare';
  requestId: number;
  bytes: Uint8Array;
  frameOptions: FrameOptionsForPrepare;
};
export type EncodeRequest = {
  kind: 'encode';
  requestId: number;
  rgba: Uint8Array;
  width: number;
  height: number;
  quality: number;
};
export type BatchRequest = {
  kind: 'batch';
  items: BatchItem[];
  options: PipelineOptions;
};
export type WorkerRequest = PrepareRequest | EncodeRequest | BatchRequest;

export type PreparedReply = {
  kind: 'prepared';
  requestId: number;
  rgba: Uint8Array;
  width: number;
  height: number;
};
export type EncodedReply = {
  kind: 'encoded';
  requestId: number;
  jpeg: Uint8Array;
};
export type WorkerProgress = {
  kind: 'progress';
  index: number;
  total: number;
  key: string;
  /** Last pipeline stage that finished for this item; absent on the
   * initial "item started" event. */
  stage?: Stage;
  /** Cumulative percent for the current item, 0..100. Rust computes
   * this from {@link Stage.percent_complete} and rides it across the
   * boundary inside `StageEvent`, so the JS side never carries a
   * parallel lookup table. */
  percent: number;
};
export type WorkerDone = { kind: 'done'; results: BatchResult[] };
export type WorkerError = {
  kind: 'error';
  requestId: number | null;
  message: string;
};
export type WorkerReply = PreparedReply | EncodedReply | WorkerProgress | WorkerDone | WorkerError;

let wasmReady: Promise<void> | null = null;
const ensureReady = (): Promise<void> => {
  wasmReady ??= (async () => {
    await init();
    // Phase F2 — bring the rayon worker pool online if the host
    // page has SharedArrayBuffer support (set by COOP/COEP headers
    // in dev, by coi-serviceworker on GitHub Pages). When absent we
    // skip silently and rayon-aware code paths (compose_canvas
    // par_chunks_mut, fast_image_resize internals) fall back to
    // single-thread — the WASM module still runs, just without
    // intra-image parallelism. navigator.hardwareConcurrency is
    // the standard heuristic for pool size; clamp at 8 so we don't
    // spawn more workers than the typical browser cap on Web
    // Workers per origin.
    if (typeof SharedArrayBuffer !== 'undefined') {
      const cores = Math.min(8, Math.max(2, navigator.hardwareConcurrency ?? 4));
      try {
        await initThreadPool(cores);
      } catch (err) {
        // Browser refused to spawn workers despite SAB being present
        // (uncommon — usually CSP). Fall back to single-thread.
        // biome-ignore lint/suspicious/noConsole: worker boot diagnostic
        console.warn('photo-frame-wasm: initThreadPool failed, falling back', err);
      }
    }
  })();
  return wasmReady;
};

const post = (reply: WorkerReply, transfer: Transferable[] = []): void => {
  // The worker-scope postMessage is typed as Window.postMessage in lib.dom;
  // the cast keeps the transfer-list overload reachable without pulling in
  // DOM lib types we don't need.
  (postMessage as (msg: WorkerReply, transfer?: Transferable[]) => void)(reply, transfer);
};

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

self.addEventListener('message', async (event: MessageEvent<WorkerRequest>) => {
  const req = event.data;

  try {
    await ensureReady();
  } catch (error) {
    const requestId = 'requestId' in req ? req.requestId : null;
    post({ kind: 'error', requestId, message: errorMessage(error) });
    return;
  }

  switch (req.kind) {
    case 'prepare': {
      try {
        const out = render_pixels(req.bytes, req.frameOptions) as {
          rgba: Uint8Array;
          width: number;
          height: number;
        };
        post(
          {
            kind: 'prepared',
            requestId: req.requestId,
            rgba: out.rgba,
            width: out.width,
            height: out.height,
          },
          [out.rgba.buffer],
        );
      } catch (error) {
        post({ kind: 'error', requestId: req.requestId, message: errorMessage(error) });
      }
      return;
    }

    case 'encode': {
      try {
        const jpeg = encode_jpeg(req.rgba, req.width, req.height, req.quality);
        post({ kind: 'encoded', requestId: req.requestId, jpeg }, [jpeg.buffer]);
      } catch (error) {
        post({ kind: 'error', requestId: req.requestId, message: errorMessage(error) });
      }
      return;
    }

    case 'batch': {
      try {
        // The Rust `frame_batch` fires this callback once per
        // pipeline stage (decode → frame → encode) for every item.
        // The event arrives as the `StageEvent` discriminated union
        // generated from the Rust struct — no casts, no fallback
        // lookups, percent already computed on the Rust side.
        const onStage = (event: StageEvent) => {
          post({
            kind: 'progress',
            index: event.index,
            total: event.total,
            key: event.key,
            stage: event.stage,
            percent: event.percent,
          });
        };
        // Each item also emits a "started" event up-front (percent
        // 0, no stage yet) so the row visibly switches to
        // `processing` even before decode completes.
        for (let i = 0; i < req.items.length; i++) {
          const item = req.items[i];
          if (!item) continue;
          post({
            kind: 'progress',
            index: i,
            total: req.items.length,
            key: item.key,
            percent: 0,
          });
        }
        const raw = frame_batch(req.items, req.options, onStage) as unknown as BatchResult[];
        post({ kind: 'done', results: raw });
      } catch (error) {
        post({ kind: 'error', requestId: null, message: errorMessage(error) });
      }
      return;
    }
  }
});
