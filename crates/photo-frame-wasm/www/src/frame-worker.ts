/**
 * Web Worker host for the WASM batch entry point.
 *
 * Lives off the main thread so a 100-image drop doesn't freeze the UI
 * while wasm-bindgen does its decode → frame → encode loop. The Worker
 * speaks a tiny ad-hoc protocol with `App.tsx`:
 *
 *   request:  { kind: 'batch'; items: BatchItem[]; options: FrameOptions }
 *   reply 1+: { kind: 'progress'; index; total; key }
 *   reply:    { kind: 'done'; results: BatchResult[] }
 *   reply:    { kind: 'error'; message: string }
 *
 * Because the Worker re-initialises the WASM module on its own thread,
 * the main-thread `frame()` function from `./frame-client.ts` is
 * unaffected — we still use it for the single-image preview path so
 * preview latency stays at one round-trip.
 */

import init, { frame_batch } from '../pkg/photo_frame_wasm.js';
import type { FrameOptions } from './frame-client';

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

export type WorkerRequest = {
  kind: 'batch';
  items: BatchItem[];
  options: FrameOptions;
};

export type WorkerProgress = {
  kind: 'progress';
  index: number;
  total: number;
  key: string;
};
export type WorkerDone = { kind: 'done'; results: BatchResult[] };
export type WorkerError = { kind: 'error'; message: string };
export type WorkerReply = WorkerProgress | WorkerDone | WorkerError;

let wasmReady: Promise<void> | null = null;
const ensureReady = (): Promise<void> => {
  wasmReady ??= init().then(() => undefined);
  return wasmReady;
};

self.addEventListener('message', async (event: MessageEvent<WorkerRequest>) => {
  const req = event.data;
  if (req.kind !== 'batch') return;

  try {
    await ensureReady();
    // Per-item progress is approximated: WASM returns the whole batch
    // at once, so we report the start of each item *before* the call
    // and the synthesised "done" event after. For per-item progress
    // during the WASM call we'd need to slice the batch into N=1
    // calls, which makes the WASM ↔ JS hop cost dominate.
    //
    // This works fine for our use case: the per-image wall time is
    // mainly the decode/encode CPU, which the user already sees
    // through the post-call results list.
    for (let i = 0; i < req.items.length; i++) {
      const item = req.items[i];
      if (!item) continue;
      const progress: WorkerProgress = {
        kind: 'progress',
        index: i,
        total: req.items.length,
        key: item.key,
      };
      postMessage(progress);
    }
    const raw = frame_batch(req.items, req.options) as unknown as BatchResult[];
    const done: WorkerDone = { kind: 'done', results: raw };
    postMessage(done);
  } catch (error) {
    const failure: WorkerError = {
      kind: 'error',
      message: error instanceof Error ? error.message : String(error),
    };
    postMessage(failure);
  }
});
