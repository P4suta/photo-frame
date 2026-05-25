import { describe, expect, test } from 'vitest';
import {
  createRequestIdAllocator,
  type ExchangeReply,
  type ExchangeRequest,
  exchange,
  type MessageTarget,
} from './worker-channel';

// ── fake MessageTarget ─────────────────────────────────────────
//
// A tiny in-memory stand-in for `Worker` that records every
// `postMessage` (so request shape can be asserted) and lets the
// test drive a reply via `respond()`. Mirrors the production
// `Worker` interface closely enough that `exchange` doesn't
// notice it's not the real thing.

type SentMessage = { message: unknown; transfer: Transferable[] };

const createFakeTarget = (): MessageTarget & {
  sent: SentMessage[];
  respond: (data: unknown) => void;
  listenerCount: () => number;
} => {
  const sent: SentMessage[] = [];
  const listeners = new Set<(event: MessageEvent<unknown>) => void>();
  return {
    postMessage(message: unknown, transfer: Transferable[] = []) {
      sent.push({ message, transfer });
    },
    addEventListener(_type, listener) {
      listeners.add(listener);
    },
    removeEventListener(_type, listener) {
      listeners.delete(listener);
    },
    sent,
    listenerCount: () => listeners.size,
    respond(data: unknown) {
      // Snapshot at dispatch time so a handler removing itself
      // during iteration doesn't trip the live set.
      for (const listener of [...listeners]) {
        listener({ data } as MessageEvent<unknown>);
      }
    },
  };
};

// ── reply shapes ───────────────────────────────────────────────

type DemoRequest = ExchangeRequest & {
  kind: 'demo';
  payload: number;
};

type DemoOk = ExchangeReply & {
  kind: 'ok';
  requestId: number;
  result: number;
};

type DemoError = ExchangeReply & {
  kind: 'error';
  requestId: number;
  message: string;
};

const isDemoOk = (reply: ExchangeReply): reply is DemoOk => reply.kind === 'ok';

// ── tests ──────────────────────────────────────────────────────

describe('exchange', () => {
  test('forwards the request payload + transferables verbatim', () => {
    const target = createFakeTarget();
    const buffer = new ArrayBuffer(8);
    const request: DemoRequest = { kind: 'demo', requestId: 1, payload: 42 };
    void exchange<DemoRequest, ExchangeReply, DemoOk>(target, request, isDemoOk, [buffer]);
    expect(target.sent).toHaveLength(1);
    expect(target.sent[0]?.message).toEqual(request);
    expect(target.sent[0]?.transfer).toEqual([buffer]);
  });

  test('resolves on a matching success reply', async () => {
    const target = createFakeTarget();
    const promise = exchange<DemoRequest, ExchangeReply, DemoOk>(
      target,
      { kind: 'demo', requestId: 7, payload: 0 },
      isDemoOk,
    );
    target.respond({ kind: 'ok', requestId: 7, result: 123 } satisfies DemoOk);
    await expect(promise).resolves.toEqual({ kind: 'ok', requestId: 7, result: 123 });
  });

  test('rejects on a matching error reply with the error message', async () => {
    const target = createFakeTarget();
    const promise = exchange<DemoRequest, ExchangeReply, DemoOk>(
      target,
      { kind: 'demo', requestId: 3, payload: 0 },
      isDemoOk,
    );
    target.respond({ kind: 'error', requestId: 3, message: 'decode failed' } satisfies DemoError);
    await expect(promise).rejects.toThrow('decode failed');
  });

  test('rejects when the matching reply has an unexpected kind', async () => {
    const target = createFakeTarget();
    const promise = exchange<DemoRequest, ExchangeReply, DemoOk>(
      target,
      { kind: 'demo', requestId: 5, payload: 0 },
      isDemoOk,
    );
    // Same requestId but a kind isOk doesn't accept and that
    // isn't 'error' — exchange must reject so a protocol drift
    // surfaces loudly instead of hanging the promise.
    target.respond({ kind: 'progress', requestId: 5 });
    await expect(promise).rejects.toThrow(/unexpected worker reply kind: progress/);
  });

  test('ignores replies with mismatched requestId (stale slider tick)', async () => {
    const target = createFakeTarget();
    const promise = exchange<DemoRequest, ExchangeReply, DemoOk>(
      target,
      { kind: 'demo', requestId: 10, payload: 0 },
      isDemoOk,
    );
    // A stale reply for the previous request — exchange must
    // ignore it (handler stays attached, promise pending).
    target.respond({ kind: 'ok', requestId: 9, result: 999 } satisfies DemoOk);
    expect(target.listenerCount()).toBe(1);
    target.respond({ kind: 'ok', requestId: 10, result: 1 } satisfies DemoOk);
    await expect(promise).resolves.toMatchObject({ result: 1 });
    expect(target.listenerCount()).toBe(0);
  });

  test('ignores unsolicited replies with null requestId (batch progress)', async () => {
    const target = createFakeTarget();
    const promise = exchange<DemoRequest, ExchangeReply, DemoOk>(
      target,
      { kind: 'demo', requestId: 4, payload: 0 },
      isDemoOk,
    );
    // Batch-mode progress events ride with `requestId: null` —
    // exchange must let them pass through without resolving or
    // rejecting the outstanding request.
    target.respond({ kind: 'progress', requestId: null });
    expect(target.listenerCount()).toBe(1);
    target.respond({ kind: 'ok', requestId: 4, result: 5 } satisfies DemoOk);
    await expect(promise).resolves.toMatchObject({ result: 5 });
  });

  test('drops non-object messages without crashing', () => {
    const target = createFakeTarget();
    void exchange<DemoRequest, ExchangeReply, DemoOk>(
      target,
      { kind: 'demo', requestId: 1, payload: 0 },
      isDemoOk,
    );
    expect(() => target.respond(null)).not.toThrow();
    expect(() => target.respond('not an object')).not.toThrow();
    expect(() => target.respond(42)).not.toThrow();
    expect(target.listenerCount()).toBe(1);
  });

  test('removes its listener after a settled exchange (no leak)', async () => {
    const target = createFakeTarget();
    const promise = exchange<DemoRequest, ExchangeReply, DemoOk>(
      target,
      { kind: 'demo', requestId: 1, payload: 0 },
      isDemoOk,
    );
    expect(target.listenerCount()).toBe(1);
    target.respond({ kind: 'ok', requestId: 1, result: 0 } satisfies DemoOk);
    await promise;
    expect(target.listenerCount()).toBe(0);
  });

  test('three concurrent requests each settle on their own reply', async () => {
    const target = createFakeTarget();
    const a = exchange<DemoRequest, ExchangeReply, DemoOk>(
      target,
      { kind: 'demo', requestId: 1, payload: 0 },
      isDemoOk,
    );
    const b = exchange<DemoRequest, ExchangeReply, DemoOk>(
      target,
      { kind: 'demo', requestId: 2, payload: 0 },
      isDemoOk,
    );
    const c = exchange<DemoRequest, ExchangeReply, DemoOk>(
      target,
      { kind: 'demo', requestId: 3, payload: 0 },
      isDemoOk,
    );
    expect(target.listenerCount()).toBe(3);
    // Reply out of order — each request's listener must pick its own.
    target.respond({ kind: 'ok', requestId: 2, result: 200 } satisfies DemoOk);
    target.respond({ kind: 'ok', requestId: 3, result: 300 } satisfies DemoOk);
    target.respond({ kind: 'ok', requestId: 1, result: 100 } satisfies DemoOk);
    await expect(a).resolves.toMatchObject({ result: 100 });
    await expect(b).resolves.toMatchObject({ result: 200 });
    await expect(c).resolves.toMatchObject({ result: 300 });
    expect(target.listenerCount()).toBe(0);
  });
});

describe('createRequestIdAllocator', () => {
  test('returns 1, 2, 3, … monotonically per allocator', () => {
    const alloc = createRequestIdAllocator();
    expect(alloc()).toBe(1);
    expect(alloc()).toBe(2);
    expect(alloc()).toBe(3);
  });

  test('two allocators have independent counters', () => {
    // The seam exists so tests can spin up a fresh counter
    // without the module-global one leaking between cases.
    const a = createRequestIdAllocator();
    const b = createRequestIdAllocator();
    a();
    a();
    expect(b()).toBe(1);
    expect(a()).toBe(3);
  });
});
