import { createRoot, createSignal } from 'solid-js';
import { describe, expect, test, vi } from 'vitest';
import type { DroppedFile } from '../DropZone';
import type {
  CaptionLayout,
  FrameStyle,
  FrameTheme,
  MetaPolicy,
  PipelineSpec,
  PreparedPixels,
} from '../frame-client';
import type { LongEdgeKey } from '../lib/long-edge';
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
  metaPolicy: () => MetaPolicy,
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

describe('createPreviewSession', () => {
  test('starts with null pixels and not busy', () => {
    createRoot((dispose) => {
      const [source] = createSignal<DroppedFile | null>(null);
      const [theme] = createSignal<FrameTheme>('paper');
      const [layout] = createSignal<CaptionLayout>('edges');
      const [metaPolicy] = createSignal<MetaPolicy>('auto');
      const preview = createPreviewSession({
        source,
        settings: fakeSettings(theme, layout, metaPolicy),
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
    vi.mocked(preparePixels).mockImplementation((_bytes, _spec) => {
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
        const [metaPolicy] = createSignal<MetaPolicy>('auto');
        const preview = createPreviewSession({
          source,
          settings: fakeSettings(theme, layout, metaPolicy),
          setStatus: () => undefined,
        });

        setSource(file('a.jpg'));
        await Promise.resolve();

        expect(preview.state.pixels()).toBeNull();

        resolveFirst?.(px(99));
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
        const [metaPolicy] = createSignal<MetaPolicy>('auto');
        const preview = createPreviewSession({
          source,
          settings: fakeSettings(theme, layout, metaPolicy),
          setStatus: () => undefined,
        });

        setSource(file('a.jpg'));
        await Promise.resolve();

        preview.dispose();

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
        const [metaPolicy] = createSignal<MetaPolicy>('auto');
        createPreviewSession({
          source,
          settings: fakeSettings(theme, layout, metaPolicy),
          setStatus: (s) => statuses.push(s),
        });

        setSource(file('a.jpg'));
        await Promise.resolve();
        await Promise.resolve();
        await Promise.resolve();

        expect(statuses).toContain('Framing preview…');
        expect(statuses).toContain('');
        dispose();
        finish();
      });
    });
  });

  test('prepare receives a PipelineSpec carrying the current frame settings', async () => {
    vi.mocked(preparePixels).mockReset();
    vi.mocked(preparePixels).mockResolvedValue(px(2));

    await new Promise<void>((finish) => {
      createRoot(async (dispose) => {
        const [source, setSource] = createSignal<DroppedFile | null>(null);
        const [theme] = createSignal<FrameTheme>('ink');
        const [layout] = createSignal<CaptionLayout>('centered');
        const [metaPolicy] = createSignal<MetaPolicy>('never');
        const [frameStyle] = createSignal<FrameStyle>('polaroid');
        createPreviewSession({
          source,
          settings: fakeSettings(theme, layout, metaPolicy, frameStyle),
          setStatus: () => undefined,
        });

        setSource(file('a.jpg'));
        await Promise.resolve();
        await Promise.resolve();

        const calls = vi.mocked(preparePixels).mock.calls;
        expect(calls.length).toBeGreaterThan(0);
        const firstSpec = calls[0]?.[1];
        expect(firstSpec).toMatchObject({
          frame_style: 'polaroid',
          theme: 'ink',
          layout: 'centered',
          meta_policy: 'never',
        });
        dispose();
        finish();
      });
    });
  });
});
