// Worker request/response correlation, made testable.
//
// The production `frame-client.ts` ferries every WASM call
// through a singleton Worker. Each call gets a monotonically
// increasing `requestId`; the reply listener resolves on a
// matching ID and silently ignores mismatched (= stale) replies.
// A fast slider drag floods the worker with N encode requests;
// only the latest reply matters and the rest evaporate.
//
// This module isolates that correlation logic behind a small
// `MessageTarget` seam so a fake target (e.g. a `MessageChannel`
// or a hand-rolled stub) can stand in during unit tests — the
// production wiring never has to mount a real Worker just to
// pin the request/response contract.

/** The slice of `Worker` that `exchange` actually depends on.
 *  Both production Workers and our test stubs satisfy this.
 *  `postMessage` keeps `transfer` non-optional (matching the
 *  `Worker.postMessage(msg, transfer)` overload) so a production
 *  `Worker` is assignable verbatim — callers pass `[]` for the
 *  no-transfer case rather than relying on the second overload. */
export type MessageTarget = {
  postMessage: (message: unknown, transfer: Transferable[]) => void;
  addEventListener: (type: 'message', listener: (event: MessageEvent<unknown>) => void) => void;
  removeEventListener: (type: 'message', listener: (event: MessageEvent<unknown>) => void) => void;
};

/** Shape every request must satisfy: a `kind` discriminant and a
 *  numeric `requestId` we'll match against the reply. */
export type ExchangeRequest = {
  kind: string;
  requestId: number;
};

/** Shape every reply must satisfy. `requestId` may be `null` for
 *  unsolicited replies (e.g. batch progress events) — those are
 *  always ignored by `exchange`. */
export type ExchangeReply = {
  kind: string;
  requestId?: number | null;
};

/** Allocator for monotonically increasing request IDs. The state
 *  is encapsulated so tests can spin up a fresh allocator without
 *  inheriting the module-global counter. */
export type RequestIdAllocator = () => number;

export const createRequestIdAllocator = (): RequestIdAllocator => {
  let next = 1;
  return (): number => {
    const id = next;
    next += 1;
    return id;
  };
};

/** Issue one request whose reply identifies itself with the same
 *  `requestId`. Resolves on a matching success reply that passes
 *  `isOk`, rejects on a matching `error` reply or a kind mismatch.
 *  Replies for other IDs (= stale or unsolicited) are ignored. */
export const exchange = <
  Req extends ExchangeRequest,
  Reply extends ExchangeReply,
  Ok extends Reply,
>(
  target: MessageTarget,
  request: Req,
  isOk: (reply: Reply) => reply is Ok,
  transfer: Transferable[] = [],
): Promise<Ok> =>
  new Promise<Ok>((resolve, reject) => {
    const handler = (event: MessageEvent<unknown>): void => {
      const reply = event.data as Reply | null;
      // Drop non-conforming messages without leaking the listener.
      if (reply === null || typeof reply !== 'object') return;
      if (!('requestId' in reply) || reply.requestId !== request.requestId) return;
      target.removeEventListener('message', handler);
      if (reply.kind === 'error') {
        const raw = (reply as unknown as { message?: unknown }).message;
        const message =
          raw instanceof Error ? raw.message : typeof raw === 'string' ? raw : 'worker error';
        reject(new Error(message));
        return;
      }
      if (isOk(reply)) {
        resolve(reply);
      } else {
        reject(new Error(`unexpected worker reply kind: ${reply.kind}`));
      }
    };
    target.addEventListener('message', handler);
    target.postMessage(request, transfer);
  });
