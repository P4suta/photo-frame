// WASM façade — hides wasm-bindgen plumbing from the UI layer.
//
// Wrapping the imperative `frame(bytes, opts)` call in a typed,
// promise-returning function gives the UI code a single seam to mock or
// instrument (e.g. for tracing), and makes the JS↔WASM byte ferrying
// explicit and TS-clean.

import init, { frame } from '../pkg/photo_frame_wasm.js';

export type FrameOptions = {
  jpeg_quality: number;
  bg_r: number;
  bg_g: number;
  bg_b: number;
  show_meta: boolean;
  max_long_edge: number | null;
};

let initialized: Promise<void> | null = null;

/** Idempotently initialise the WASM module. */
export const ensureWasm = (): Promise<void> => {
  initialized ??= init().then(() => undefined);
  return initialized;
};

/** Frame `bytes` and return a JPEG `Blob`. */
export const frameImage = async (bytes: Uint8Array, opts: FrameOptions): Promise<Blob> => {
  await ensureWasm();
  const out = frame(bytes, opts);
  // TS 5.x's Blob refuses Uint8Array<ArrayBufferLike> directly; copy into a
  // fresh ArrayBuffer so the type is unambiguously ArrayBuffer.
  const buffer = new ArrayBuffer(out.byteLength);
  new Uint8Array(buffer).set(out);
  return new Blob([buffer], { type: 'image/jpeg' });
};
