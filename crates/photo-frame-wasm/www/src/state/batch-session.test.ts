import { createRoot, createSignal } from 'solid-js';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import type { DroppedFile } from '../DropZone';
import type {
  BatchResult,
  CaptionLayout,
  FrameStyle,
  FrameTheme,
  MetaPolicy,
  PipelineSpec,
  WorkerReply,
  WorkerRequest,
} from '../frame-client';
import type { LongEdgeKey } from '../lib/long-edge';
import type { MessageTarget } from '../lib/worker-channel';
import type { AppSettings } from './app-settings';

vi.mock('../frame-client', async () => {
  const actual = await vi.importActual<typeof import('../frame-client')>('../frame-client');
  return {
    ...actual,
    generateThumbnailBlob: vi.fn().mockResolvedValue(new Blob()),
  };
});

import { generateThumbnailBlob } from '../frame-client';
import { createBatchSession } from './batch-session';

const file = (name: string, longEdge = 1000): DroppedFile => ({
  name,
  data: new Uint8Array([1, 2, 3]),
  longEdge,
});

const fakeSettings = (
  theme: () => FrameTheme = () => 'paper',
  layout: () => CaptionLayout = () => 'edges',
  metaPolicy: () => MetaPolicy = () => 'auto',
  frameStyle: () => FrameStyle = () => 'standard',
): AppSettings['state'] => ({
  preset: () => 'standard',
  quality: () => 92,
  longEdge: (() => 'full') as () => LongEdgeKey,
  frameStyle,
  theme,
  layout,
  metaPolicy,
  effectiveMaxLongEdge: () => null,
  presets: () => [],
  buildSpec: (maxLongEdge): PipelineSpec => ({
    frame_style: frameStyle(),
    theme: theme(),
    layout: layout(),
    meta_policy: metaPolicy(),
    jpeg_quality: 92,
    max_long_edge: maxLongEdge,
  }),
});

// In-memory MessageTarget that records postMessage calls and lets
// tests fire synthetic worker replies to the most-recently attached
// listener.
const makeFakeWorker = (): MessageTarget & {
  posts: WorkerRequest[];
  reply: (msg: WorkerReply) => void;
  listenerCount: () => number;
} => {
  const posts: WorkerRequest[] = [];
  const listeners: Array<(event: MessageEvent<unknown>) => void> = [];
  return {
    posts,
    postMessage: (message) => {
      posts.push(message as WorkerRequest);
    },
    addEventListener: (_type, listener) => {
      listeners.push(listener);
    },
    removeEventListener: (_type, listener) => {
      const i = listeners.indexOf(listener);
      if (i >= 0) listeners.splice(i, 1);
    },
    reply: (msg) => {
      const event = { data: msg } as MessageEvent<unknown>;
      for (const l of [...listeners]) l(event);
    },
    listenerCount: () => listeners.length,
  };
};

beforeEach(() => {
  vi.mocked(generateThumbnailBlob).mockClear();
  vi.mocked(generateThumbnailBlob).mockResolvedValue(new Blob());
  vi.useFakeTimers();
});

describe('createBatchSession', () => {
  test('starts empty', () => {
    createRoot((dispose) => {
      const [files] = createSignal<DroppedFile[] | null>(null);
      const session = createBatchSession({
        files,
        settings: fakeSettings(),
        setStatus: () => undefined,
        workerTarget: makeFakeWorker(),
      });
      expect(session.state.rows()).toEqual([]);
      expect(session.state.doneCount()).toBe(0);
      dispose();
    });
  });

  test('seeds queued rows when files arrive', async () => {
    await new Promise<void>((finish) => {
      createRoot(async (dispose) => {
        const [files, setFiles] = createSignal<DroppedFile[] | null>(null);
        const session = createBatchSession({
          files,
          settings: fakeSettings(),
          setStatus: () => undefined,
          workerTarget: makeFakeWorker(),
        });
        setFiles([file('a.jpg'), file('b.jpg')]);
        // Solid's createEffect fires on the microtask queue.
        await Promise.resolve();
        const rs = session.state.rows();
        expect(rs).toHaveLength(2);
        expect(rs[0]?.key).toBe('a.jpg');
        expect(rs[0]?.status).toBe('queued');
        // Each row gets a unique stable identifier the View
        // Transitions API can use to scope per-row crossfades.
        expect(rs[0]?.transitionName).toBe('gallery-thumb-0');
        expect(rs[1]?.transitionName).toBe('gallery-thumb-1');
        dispose();
        finish();
      });
    });
  });

  test('thumbnail pass posts batch request as a tail-call', async () => {
    const worker = makeFakeWorker();
    await new Promise<void>((finish) => {
      createRoot(async (dispose) => {
        const [files, setFiles] = createSignal<DroppedFile[] | null>(null);
        createBatchSession({
          files,
          settings: fakeSettings(),
          setStatus: () => undefined,
          workerTarget: worker,
        });
        setFiles([file('a.jpg')]);
        await Promise.resolve();
        // Advance past the thumbnail debounce window so the regen
        // fires.
        await vi.advanceTimersByTimeAsync(400);
        // Microtask flush so the await chain inside the regen
        // resolves all the way to onProcessBatch.
        await Promise.resolve();
        await Promise.resolve();
        expect(worker.posts).toHaveLength(1);
        expect(worker.posts[0]?.kind).toBe('batch');
        dispose();
        finish();
      });
    });
  });

  test('progress reply marks the matching row as processing', async () => {
    const worker = makeFakeWorker();
    await new Promise<void>((finish) => {
      createRoot(async (dispose) => {
        const [files, setFiles] = createSignal<DroppedFile[] | null>(null);
        const session = createBatchSession({
          files,
          settings: fakeSettings(),
          setStatus: () => undefined,
          workerTarget: worker,
        });
        setFiles([file('a.jpg'), file('b.jpg')]);
        await Promise.resolve();
        await vi.advanceTimersByTimeAsync(400);
        await Promise.resolve();
        await Promise.resolve();
        worker.reply({ kind: 'progress', key: 'b.jpg', index: 0, total: 2, percent: 50 });
        expect(session.state.rows().find((r) => r.key === 'b.jpg')?.status).toBe('processing');
        expect(session.state.rows().find((r) => r.key === 'a.jpg')?.status).toBe('queued');
        dispose();
        finish();
      });
    });
  });

  test('done reply fills resultUrl and doneCount', async () => {
    const worker = makeFakeWorker();
    await new Promise<void>((finish) => {
      createRoot(async (dispose) => {
        const [files, setFiles] = createSignal<DroppedFile[] | null>(null);
        const session = createBatchSession({
          files,
          settings: fakeSettings(),
          setStatus: () => undefined,
          workerTarget: worker,
        });
        setFiles([file('a.jpg')]);
        await Promise.resolve();
        await vi.advanceTimersByTimeAsync(400);
        await Promise.resolve();
        await Promise.resolve();
        const results: BatchResult[] = [
          { key: 'a.jpg', ok: true, result: new Uint8Array([0xff, 0xd8, 0xff]), elapsed_ms: 42 },
        ];
        worker.reply({ kind: 'done', results });
        const row = session.state.rows().find((r) => r.key === 'a.jpg');
        expect(row?.status).toBe('done');
        expect(row?.resultUrl).toBeTruthy();
        expect(row?.message).toBe('42 ms');
        expect(session.state.doneCount()).toBe(1);
        dispose();
        finish();
      });
    });
  });

  test('dispose detaches the worker listener and clears rows', async () => {
    const worker = makeFakeWorker();
    await new Promise<void>((finish) => {
      createRoot(async (dispose) => {
        const [files, setFiles] = createSignal<DroppedFile[] | null>(null);
        const session = createBatchSession({
          files,
          settings: fakeSettings(),
          setStatus: () => undefined,
          workerTarget: worker,
        });
        setFiles([file('a.jpg')]);
        await Promise.resolve();
        await vi.advanceTimersByTimeAsync(400);
        await Promise.resolve();
        await Promise.resolve();
        expect(worker.listenerCount()).toBeGreaterThan(0);
        session.dispose();
        expect(worker.listenerCount()).toBe(0);
        expect(session.state.rows()).toEqual([]);
        dispose();
        finish();
      });
    });
  });

  test('classification stays synchronous when startViewTransition defers its callback', async () => {
    // Regression for a production-only bug: jsdom's environment
    // has no `document.startViewTransition`, so the wrapper
    // falls back to a synchronous call. In real Chromium the
    // callback is deferred to the next rendering tick. If the
    // classification (= which rows need a worker re-run) lived
    // inside that deferred callback, the for-loop would race
    // ahead with an empty work list and *no* thumbnails would be
    // generated — producing the "progress bar shows but the
    // thumbnail never lands" symptom the user reported.
    //
    // Stub `document.startViewTransition` to *never* invoke its
    // callback, then assert the worker calls happen anyway.
    const deferred: Array<() => void> = [];
    vi.stubGlobal('document', {
      startViewTransition: (cb: () => void) => {
        deferred.push(cb);
        return {};
      },
    });
    try {
      await new Promise<void>((finish) => {
        createRoot(async (dispose) => {
          const [files, setFiles] = createSignal<DroppedFile[] | null>(null);
          createBatchSession({
            files,
            settings: fakeSettings(),
            setStatus: () => undefined,
            workerTarget: makeFakeWorker(),
          });
          const callsBefore = vi.mocked(generateThumbnailBlob).mock.calls.length;
          setFiles([file('a.jpg'), file('b.jpg')]);
          await Promise.resolve();
          await vi.advanceTimersByTimeAsync(400);
          await Promise.resolve();
          await Promise.resolve();
          // Both rows must have entered the worker round-trip
          // even though the deferred View Transition callbacks
          // are still pending invocation.
          expect(vi.mocked(generateThumbnailBlob).mock.calls.length).toBe(callsBefore + 2);
          dispose();
          finish();
        });
      });
    } finally {
      vi.unstubAllGlobals();
    }
  });

  test('settings-change regenerate keeps the previous thumb visible until the new one lands', async () => {
    // Stale-while-revalidate pin: a settings flip must not blank
    // the gallery while the new thumbnails are in flight. The
    // row's `thumb.url` must point at the previous URL right up
    // until the new blob resolves; only then does the rotate land
    // (previous URL drops into `prevThumb`, new URL becomes
    // `thumb`) in a single tick.
    const revokeSpy = vi.spyOn(URL, 'revokeObjectURL');
    await new Promise<void>((finish) => {
      createRoot(async (dispose) => {
        const [files, setFiles] = createSignal<DroppedFile[] | null>(null);
        const [theme, setTheme] = createSignal<FrameTheme>('paper');
        const session = createBatchSession({
          files,
          settings: fakeSettings(theme),
          setStatus: () => undefined,
          workerTarget: makeFakeWorker(),
        });

        // Initial pass — vanilla resolved mock populates thumbnails.
        setFiles([file('a.jpg')]);
        await Promise.resolve();
        await vi.advanceTimersByTimeAsync(400);
        await Promise.resolve();
        await Promise.resolve();
        const firstUrl = session.state.rows().find((r) => r.key === 'a.jpg')?.thumb?.url;
        expect(firstUrl).toBeTruthy();

        // Second pass — hold the new blob promise so we can observe
        // the in-flight interval. The default value throws on
        // accidental early invocation; the test relies on the
        // await chain below to have run the mock impl by the time
        // we call it explicitly.
        let resolveSecond: (blob: Blob) => void = () => {
          throw new Error('resolveSecond invoked before the mock impl bound it');
        };
        vi.mocked(generateThumbnailBlob).mockImplementationOnce(
          () =>
            new Promise<Blob>((resolve) => {
              resolveSecond = resolve;
            }),
        );
        const revokesBeforeFlip = revokeSpy.mock.calls.length;
        setTheme('ink');
        await vi.advanceTimersByTimeAsync(400);
        await Promise.resolve();

        // While the new thumb is in flight the previous URL must
        // still be on the row, and it must not have been revoked.
        expect(session.state.rows().find((r) => r.key === 'a.jpg')?.thumb?.url).toBe(firstUrl);
        expect(revokeSpy.mock.calls.length).toBe(revokesBeforeFlip);

        // Resolving the new thumb rotates: the depth-2 layers shift
        // (new → thumb, old → prevThumb). Nothing is revoked yet
        // because the previous slot was empty before this rotation.
        resolveSecond(new Blob());
        await Promise.resolve();
        await Promise.resolve();
        const after = session.state.rows().find((r) => r.key === 'a.jpg');
        expect(after?.thumb?.url).toBeTruthy();
        expect(after?.thumb?.url).not.toBe(firstUrl);
        expect(after?.prevThumb?.url).toBe(firstUrl);
        // The first rotation evicts nothing — the previous slot
        // started empty, so no revoke fires here.
        expect(revokeSpy.mock.calls.length).toBe(revokesBeforeFlip);

        dispose();
        finish();
      });
    });
    revokeSpy.mockRestore();
  });

  test('toggle-back to a cached spec swaps the depth-2 layers without a worker call', async () => {
    // The whole point of the per-row LRU: a setting flip and back
    // (A → B → A) should restore the original thumbnail instantly
    // (no `generateThumbnailBlob` call for the back-flip) since
    // we kept it alive in `prevThumb`.
    await new Promise<void>((finish) => {
      createRoot(async (dispose) => {
        const [files, setFiles] = createSignal<DroppedFile[] | null>(null);
        const [theme, setTheme] = createSignal<FrameTheme>('paper');
        const session = createBatchSession({
          files,
          settings: fakeSettings(theme),
          setStatus: () => undefined,
          workerTarget: makeFakeWorker(),
        });

        // A: initial pass.
        setFiles([file('a.jpg')]);
        await Promise.resolve();
        await vi.advanceTimersByTimeAsync(400);
        await Promise.resolve();
        await Promise.resolve();
        const urlA = session.state.rows().find((r) => r.key === 'a.jpg')?.thumb?.url;
        expect(urlA).toBeTruthy();

        // B: flip the theme. Generates a new thumb under the new key.
        setTheme('ink');
        await vi.advanceTimersByTimeAsync(400);
        await Promise.resolve();
        await Promise.resolve();
        const urlB = session.state.rows().find((r) => r.key === 'a.jpg')?.thumb?.url;
        expect(urlB).toBeTruthy();
        expect(urlB).not.toBe(urlA);
        expect(session.state.rows().find((r) => r.key === 'a.jpg')?.prevThumb?.url).toBe(urlA);

        // A again: flip back. The first-pass `setRows` should swap
        // `thumb` ↔ `prevThumb` (no worker call), so the row's
        // visible URL is urlA again without invoking the mock.
        const callsBeforeBackFlip = vi.mocked(generateThumbnailBlob).mock.calls.length;
        setTheme('paper');
        await vi.advanceTimersByTimeAsync(400);
        await Promise.resolve();
        await Promise.resolve();
        const after = session.state.rows().find((r) => r.key === 'a.jpg');
        expect(after?.thumb?.url).toBe(urlA);
        expect(after?.prevThumb?.url).toBe(urlB);
        // No new worker call for the cached spec.
        expect(vi.mocked(generateThumbnailBlob).mock.calls.length).toBe(callsBeforeBackFlip);

        dispose();
        finish();
      });
    });
  });

  test('a third spec evicts and revokes the depth-2 oldest layer', async () => {
    // Cache depth is 2 — going A → B → C must revoke A's URL when
    // C lands (A drops out of `prevThumb`).
    const revokeSpy = vi.spyOn(URL, 'revokeObjectURL');
    await new Promise<void>((finish) => {
      createRoot(async (dispose) => {
        const [files, setFiles] = createSignal<DroppedFile[] | null>(null);
        const [theme, setTheme] = createSignal<FrameTheme>('paper');
        const [layout, setLayout] = createSignal<CaptionLayout>('edges');
        const session = createBatchSession({
          files,
          settings: fakeSettings(theme, layout),
          setStatus: () => undefined,
          workerTarget: makeFakeWorker(),
        });

        setFiles([file('a.jpg')]);
        await Promise.resolve();
        await vi.advanceTimersByTimeAsync(400);
        await Promise.resolve();
        await Promise.resolve();
        const urlA = session.state.rows().find((r) => r.key === 'a.jpg')?.thumb?.url;
        expect(urlA).toBeTruthy();

        setTheme('ink');
        await vi.advanceTimersByTimeAsync(400);
        await Promise.resolve();
        await Promise.resolve();

        const revokesBeforeC = revokeSpy.mock.calls.length;
        setLayout('centered');
        await vi.advanceTimersByTimeAsync(400);
        await Promise.resolve();
        await Promise.resolve();

        // C lands → A (the oldest) was revoked.
        expect(revokeSpy.mock.calls.length).toBeGreaterThan(revokesBeforeC);
        expect(revokeSpy).toHaveBeenCalledWith(urlA);

        dispose();
        finish();
      });
    });
    revokeSpy.mockRestore();
  });

  test('replacing the files signal revokes the previous batch URLs', async () => {
    // Drop → new drop is a session boundary. The previous batch's
    // thumbnail layers + resultUrl blobs are unreachable once the
    // rows are replaced, so the renderEffect must revoke them
    // before seeding the new rows — otherwise a drop-and-redrop
    // loop accumulates dead entries in the blob registry.
    const revokeSpy = vi.spyOn(URL, 'revokeObjectURL');
    await new Promise<void>((finish) => {
      createRoot(async (dispose) => {
        const [files, setFiles] = createSignal<DroppedFile[] | null>(null);
        const worker = makeFakeWorker();
        const session = createBatchSession({
          files,
          settings: fakeSettings(),
          setStatus: () => undefined,
          workerTarget: worker,
        });
        setFiles([file('a.jpg')]);
        await Promise.resolve();
        await vi.advanceTimersByTimeAsync(400);
        await Promise.resolve();
        await Promise.resolve();
        worker.reply({
          kind: 'done',
          results: [{ key: 'a.jpg', ok: true, result: new Uint8Array([1, 2, 3]), elapsed_ms: 10 }],
        });
        const row = session.state.rows().find((r) => r.key === 'a.jpg');
        const thumbUrl = row?.thumb?.url;
        const resultUrl = row?.resultUrl;
        expect(thumbUrl).toBeTruthy();
        expect(resultUrl).toBeTruthy();

        const revokesBeforeRedrop = revokeSpy.mock.calls.length;
        setFiles([file('b.jpg')]);
        await Promise.resolve();

        expect(revokeSpy.mock.calls.length).toBeGreaterThan(revokesBeforeRedrop);
        expect(revokeSpy).toHaveBeenCalledWith(thumbUrl);
        expect(revokeSpy).toHaveBeenCalledWith(resultUrl);

        dispose();
        finish();
      });
    });
    revokeSpy.mockRestore();
  });

  test('re-run done reply revokes the prior resultUrl before overwriting', async () => {
    // A second batch run on the same files (settings flip → debounce
    // → regen → tail-call onProcessBatch → another 'done') must not
    // leak the first run's resultUrl. The revoke happens inside
    // applyBatchResults so successive runs do not accumulate dead
    // entries in the blob registry.
    const revokeSpy = vi.spyOn(URL, 'revokeObjectURL');
    await new Promise<void>((finish) => {
      createRoot(async (dispose) => {
        const [files, setFiles] = createSignal<DroppedFile[] | null>(null);
        const [theme, setTheme] = createSignal<FrameTheme>('paper');
        const worker = makeFakeWorker();
        const session = createBatchSession({
          files,
          settings: fakeSettings(theme),
          setStatus: () => undefined,
          workerTarget: worker,
        });

        // First run.
        setFiles([file('a.jpg')]);
        await Promise.resolve();
        await vi.advanceTimersByTimeAsync(400);
        await Promise.resolve();
        await Promise.resolve();
        worker.reply({
          kind: 'done',
          results: [{ key: 'a.jpg', ok: true, result: new Uint8Array([1, 2, 3]), elapsed_ms: 10 }],
        });
        const firstResultUrl = session.state.rows().find((r) => r.key === 'a.jpg')?.resultUrl;
        expect(firstResultUrl).toBeTruthy();

        // Second run via a settings flip.
        const revokesBefore2and = revokeSpy.mock.calls.length;
        setTheme('ink');
        await vi.advanceTimersByTimeAsync(400);
        await Promise.resolve();
        await Promise.resolve();
        worker.reply({
          kind: 'done',
          results: [{ key: 'a.jpg', ok: true, result: new Uint8Array([4, 5, 6]), elapsed_ms: 20 }],
        });
        const secondResultUrl = session.state.rows().find((r) => r.key === 'a.jpg')?.resultUrl;
        expect(secondResultUrl).toBeTruthy();
        expect(secondResultUrl).not.toBe(firstResultUrl);
        expect(revokeSpy.mock.calls.length).toBeGreaterThan(revokesBefore2and);
        expect(revokeSpy).toHaveBeenCalledWith(firstResultUrl);

        dispose();
        finish();
      });
    });
    revokeSpy.mockRestore();
  });

  test('dispose revokes resultUrl blobs', async () => {
    const worker = makeFakeWorker();
    const revokeSpy = vi.spyOn(URL, 'revokeObjectURL');
    await new Promise<void>((finish) => {
      createRoot(async (dispose) => {
        const [files, setFiles] = createSignal<DroppedFile[] | null>(null);
        const session = createBatchSession({
          files,
          settings: fakeSettings(),
          setStatus: () => undefined,
          workerTarget: worker,
        });
        setFiles([file('a.jpg')]);
        await Promise.resolve();
        await vi.advanceTimersByTimeAsync(400);
        await Promise.resolve();
        await Promise.resolve();
        worker.reply({
          kind: 'done',
          results: [{ key: 'a.jpg', ok: true, result: new Uint8Array([1, 2, 3]), elapsed_ms: 10 }],
        });
        const before = revokeSpy.mock.calls.length;
        session.dispose();
        // dispose revoked at least the one resultUrl from the row.
        expect(revokeSpy.mock.calls.length).toBeGreaterThan(before);
        dispose();
        finish();
      });
    });
    revokeSpy.mockRestore();
  });
});
