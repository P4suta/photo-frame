import { createRoot, createSignal } from 'solid-js';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import type { DroppedFile } from '../DropZone';
import type {
  BatchResult,
  CaptionLayout,
  FrameTheme,
  WorkerReply,
  WorkerRequest,
} from '../frame-client';
import type { LongEdgeKey, PresetKey } from '../lib/long-edge';
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
  showMeta: () => boolean = () => true,
): AppSettings['state'] => ({
  preset: (() => 'standard') as () => PresetKey,
  quality: () => 92,
  longEdge: (() => 'full') as () => LongEdgeKey,
  theme,
  layout,
  showMeta,
  effectiveMaxLongEdge: () => null,
  buildFrameOptions: () => ({
    theme: theme(),
    layout: layout(),
    show_meta: showMeta(),
    max_long_edge: null,
  }),
  buildPipelineOptions: () => ({
    theme: theme(),
    layout: layout(),
    show_meta: showMeta(),
    max_long_edge: null,
    jpeg_quality: 92,
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
        setStatus: () => {},
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
          setStatus: () => {},
          workerTarget: makeFakeWorker(),
        });
        setFiles([file('a.jpg'), file('b.jpg')]);
        // Solid's createEffect fires on the microtask queue.
        await Promise.resolve();
        const rs = session.state.rows();
        expect(rs).toHaveLength(2);
        expect(rs[0]?.key).toBe('a.jpg');
        expect(rs[0]?.status).toBe('queued');
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
          setStatus: () => {},
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
          setStatus: () => {},
          workerTarget: worker,
        });
        setFiles([file('a.jpg'), file('b.jpg')]);
        await Promise.resolve();
        await vi.advanceTimersByTimeAsync(400);
        await Promise.resolve();
        await Promise.resolve();
        worker.reply({ kind: 'progress', key: 'b.jpg', index: 0, total: 2 });
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
          setStatus: () => {},
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
          setStatus: () => {},
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

  test('dispose revokes resultUrl blobs', async () => {
    const worker = makeFakeWorker();
    const revokeSpy = vi.spyOn(URL, 'revokeObjectURL');
    await new Promise<void>((finish) => {
      createRoot(async (dispose) => {
        const [files, setFiles] = createSignal<DroppedFile[] | null>(null);
        const session = createBatchSession({
          files,
          settings: fakeSettings(),
          setStatus: () => {},
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
