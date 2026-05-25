import { createRoot, createSignal } from 'solid-js';
import { describe, expect, test, vi } from 'vitest';
import type { DroppedFile } from '../DropZone';
import type { CaptionLayout, FrameTheme, PreparedPixels } from '../frame-client';
import type { LongEdgeKey, PresetKey } from '../lib/long-edge';
import type { AppSettings } from './app-settings';
import { createPreviewSession } from './preview-session';

// Mock the frame-client worker calls so the preview-session can be
// tested without spinning up a Worker / WASM. Each test that
// exercises the prepare flight overrides `preparePixels` to a
// deterministic in-memory response.
vi.mock('../frame-client', async () => {
  const actual = await vi.importActual<typeof import('../frame-client')>('../frame-client');
  return {
    ...actual,
    preparePixels: vi.fn(),
    encodeJpeg: vi.fn(),
  };
});

import { preparePixels } from '../frame-client';

const px = (label: number): PreparedPixels => ({
  rgba: new Uint8Array([label, 0, 0, 255]),
  width: 2,
  height: 2,
});

const file = (name: string, longEdge = 1000): DroppedFile => ({
  name,
  data: new Uint8Array([1, 2, 3]),
  longEdge,
});

const fakeSettings = (
  theme: () => FrameTheme,
  layout: () => CaptionLayout,
  showMeta: () => boolean,
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

describe('createPreviewSession', () => {
  test('starts with null pixels and not busy', () => {
    createRoot((dispose) => {
      const [source] = createSignal<DroppedFile | null>(null);
      const [theme] = createSignal<FrameTheme>('paper');
      const [layout] = createSignal<CaptionLayout>('edges');
      const [showMeta] = createSignal<boolean>(true);
      const preview = createPreviewSession({
        source,
        settings: fakeSettings(theme, layout, showMeta),
        setStatus: () => undefined,
      });
      expect(preview.state.pixels()).toBeNull();
      expect(preview.state.busy()).toBe(false);
      expect(preview.state.frameSize()).toBeNull();
      dispose();
    });
  });

  test('source change triggers a prepare; pixels() lands after the worker resolves', async () => {
    vi.mocked(preparePixels).mockReset();
    let resolveFirst: ((p: PreparedPixels) => void) | null = null;
    vi.mocked(preparePixels).mockImplementation((_bytes, _opts) => {
      // Capture the first call so we can observe before / after
      // resolution; subsequent prefetch calls resolve immediately.
      if (resolveFirst === null) {
        return new Promise<PreparedPixels>((r) => {
          resolveFirst = r;
        });
      }
      return Promise.resolve(px(0));
    });

    await new Promise<void>((finish) => {
      createRoot(async (dispose) => {
        const [source, setSource] = createSignal<DroppedFile | null>(null);
        const [theme] = createSignal<FrameTheme>('paper');
        const [layout] = createSignal<CaptionLayout>('edges');
        const [showMeta] = createSignal<boolean>(true);
        const preview = createPreviewSession({
          source,
          settings: fakeSettings(theme, layout, showMeta),
          setStatus: () => undefined,
        });

        setSource(file('a.jpg'));
        await Promise.resolve(); // flush scope effect

        // Before the first prepare resolves, pixels is still null.
        expect(preview.state.pixels()).toBeNull();

        // Resolve the in-flight prepare.
        resolveFirst?.(px(99));
        // Drain microtasks so the await in runPreparePromise + the
        // setVariants signal write propagate.
        await Promise.resolve();
        await Promise.resolve();

        expect(preview.state.pixels()?.rgba[0]).toBe(99);
        dispose();
        finish();
      });
    });
  });

  test('dispose bumps the gate so a late prepare reply does not write into the cache', async () => {
    vi.mocked(preparePixels).mockReset();
    let resolveFirst: ((p: PreparedPixels) => void) | null = null;
    vi.mocked(preparePixels).mockImplementation(() => {
      if (resolveFirst === null) {
        return new Promise<PreparedPixels>((r) => {
          resolveFirst = r;
        });
      }
      return Promise.resolve(px(0));
    });

    await new Promise<void>((finish) => {
      createRoot(async (dispose) => {
        const [source, setSource] = createSignal<DroppedFile | null>(null);
        const [theme] = createSignal<FrameTheme>('paper');
        const [layout] = createSignal<CaptionLayout>('edges');
        const [showMeta] = createSignal<boolean>(true);
        const preview = createPreviewSession({
          source,
          settings: fakeSettings(theme, layout, showMeta),
          setStatus: () => undefined,
        });

        setSource(file('a.jpg'));
        await Promise.resolve();

        // Reset before the in-flight prepare lands.
        preview.dispose();

        // Now resolve — should be ignored by the gate check inside
        // runPreparePromise.
        resolveFirst?.(px(7));
        await Promise.resolve();
        await Promise.resolve();

        expect(preview.state.pixels()).toBeNull();
        dispose();
        finish();
      });
    });
  });

  test('setStatus is invoked during prepare + on completion', async () => {
    vi.mocked(preparePixels).mockReset();
    vi.mocked(preparePixels).mockResolvedValue(px(1));

    const statuses: string[] = [];
    await new Promise<void>((finish) => {
      createRoot(async (dispose) => {
        const [source, setSource] = createSignal<DroppedFile | null>(null);
        const [theme] = createSignal<FrameTheme>('paper');
        const [layout] = createSignal<CaptionLayout>('edges');
        const [showMeta] = createSignal<boolean>(true);
        createPreviewSession({
          source,
          settings: fakeSettings(theme, layout, showMeta),
          setStatus: (s) => statuses.push(s),
        });

        setSource(file('a.jpg'));
        await Promise.resolve();
        await Promise.resolve();
        await Promise.resolve();

        // Should have at least seen 'Framing preview…' and then
        // an empty-string clear when the first prepare lands.
        expect(statuses).toContain('Framing preview…');
        expect(statuses).toContain('');
        dispose();
        finish();
      });
    });
  });
});
