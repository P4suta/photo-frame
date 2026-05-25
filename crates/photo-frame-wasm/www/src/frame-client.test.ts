import { describe, expect, test, vi } from 'vitest';
import { _encodeJpegOn, _preparePixelsOn, type FrameOptionsForPrepare } from './frame-client';
import type { MessageTarget } from './lib/worker-channel';

// ── fake MessageTarget ─────────────────────────────────────────
//
// Mirrors the shape used in `worker-channel.test.ts` so the
// frame-client functions can be driven against a deterministic
// stand-in: every `postMessage` is recorded, replies arrive when
// the test calls `respond()`.

type Sent = { message: unknown; transfer: Transferable[] };

const createFakeTarget = (): MessageTarget & {
  sent: Sent[];
  respond: (data: unknown) => void;
} => {
  const sent: Sent[] = [];
  const listeners = new Set<(event: MessageEvent<unknown>) => void>();
  return {
    postMessage(message, transfer = []) {
      sent.push({ message, transfer });
    },
    addEventListener(_type, listener) {
      listeners.add(listener);
    },
    removeEventListener(_type, listener) {
      listeners.delete(listener);
    },
    sent,
    respond(data) {
      for (const listener of [...listeners]) {
        listener({ data } as MessageEvent<unknown>);
      }
    },
  };
};

describe('_preparePixelsOn', () => {
  test('posts a prepare request with the framed options and a sliced byte buffer', () => {
    const target = createFakeTarget();
    const bytes = new Uint8Array([1, 2, 3, 4]);
    const options: FrameOptionsForPrepare = {
      theme: 'paper',
      layout: 'edges',
      show_meta: true,
      max_long_edge: 1920,
    };
    void _preparePixelsOn(target, 42, bytes, options);
    expect(target.sent).toHaveLength(1);
    const sent = target.sent[0]?.message as {
      kind: string;
      requestId: number;
      bytes: Uint8Array;
      frameOptions: FrameOptionsForPrepare;
    };
    expect(sent.kind).toBe('prepare');
    expect(sent.requestId).toBe(42);
    expect(sent.frameOptions).toEqual(options);
    expect(Array.from(sent.bytes)).toEqual([1, 2, 3, 4]);
    // The byte buffer must be a fresh slice — never the caller's,
    // otherwise the transferable would detach a buffer the caller
    // still owns. Identity check confirms.
    expect(sent.bytes).not.toBe(bytes);
    expect(target.sent[0]?.transfer[0]).toBe(sent.bytes.buffer);
  });

  test('resolves with the rgba/width/height payload from a matching prepared reply', async () => {
    const target = createFakeTarget();
    const promise = _preparePixelsOn(target, 1, new Uint8Array([0]), {
      theme: 'ink',
      layout: 'centered',
      show_meta: false,
      max_long_edge: null,
    });
    const rgba = new Uint8Array([255, 0, 0, 255]);
    target.respond({ kind: 'prepared', requestId: 1, rgba, width: 1, height: 1 });
    await expect(promise).resolves.toEqual({ rgba, width: 1, height: 1 });
  });

  test('rejects with the worker error message on a matching error reply', async () => {
    const target = createFakeTarget();
    const promise = _preparePixelsOn(target, 7, new Uint8Array([0]), {
      theme: 'paper',
      layout: 'edges',
      show_meta: true,
      max_long_edge: null,
    });
    target.respond({ kind: 'error', requestId: 7, message: 'decode failed' });
    await expect(promise).rejects.toThrow('decode failed');
  });
});

describe('_encodeJpegOn', () => {
  test('posts an encode request with width/height/quality and the rgba slice', () => {
    const target = createFakeTarget();
    const rgba = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
    void _encodeJpegOn(target, 99, rgba, 2, 1, 90);
    expect(target.sent).toHaveLength(1);
    const sent = target.sent[0]?.message as {
      kind: string;
      requestId: number;
      rgba: Uint8Array;
      width: number;
      height: number;
      quality: number;
    };
    expect(sent.kind).toBe('encode');
    expect(sent.requestId).toBe(99);
    expect(sent.width).toBe(2);
    expect(sent.height).toBe(1);
    expect(sent.quality).toBe(90);
    expect(Array.from(sent.rgba)).toEqual([1, 2, 3, 4, 5, 6, 7, 8]);
    // Caller's rgba must stay intact across the call.
    expect(sent.rgba).not.toBe(rgba);
  });

  test('resolves with the jpeg buffer from a matching encoded reply', async () => {
    const target = createFakeTarget();
    const promise = _encodeJpegOn(target, 1, new Uint8Array([0, 0, 0, 255]), 1, 1, 80);
    const jpeg = new Uint8Array([0xff, 0xd8, 0xff, 0xd9]);
    target.respond({ kind: 'encoded', requestId: 1, jpeg });
    await expect(promise).resolves.toEqual(jpeg);
  });
});

describe('exchange-level wiring (frame-client)', () => {
  test('a stale reply for a previous requestId does not resolve a fresh request', async () => {
    // Phase G2 — a slider-drag race that the requestId fence
    // exists to prevent. Pin it here so any refactor that
    // drops the fence is caught by tests, not by users.
    const target = createFakeTarget();
    const promise = _encodeJpegOn(target, 100, new Uint8Array([0]), 1, 1, 80);
    target.respond({ kind: 'encoded', requestId: 99, jpeg: new Uint8Array([0xde, 0xad]) });
    const settled = vi.fn();
    promise.then(settled, settled);
    await Promise.resolve();
    await Promise.resolve();
    expect(settled).not.toHaveBeenCalled();
    target.respond({ kind: 'encoded', requestId: 100, jpeg: new Uint8Array([0xbe, 0xef]) });
    await expect(promise).resolves.toEqual(new Uint8Array([0xbe, 0xef]));
  });
});
