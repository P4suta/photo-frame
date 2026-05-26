import { createRoot, createSignal } from 'solid-js';
import { describe, expect, test } from 'vitest';
import type { Preset } from '../frame-client';
import { createAppSettings } from './app-settings';

// Fixed preset table mirroring the Rust `PipelineSpec::PRESETS` shape.
// Tests inject this instead of waiting for `loadPresets()` so the
// reactive primitive can be exercised without spinning up WASM. Drift
// against Rust truth is caught by the round-trip pin test in
// `photo-frame-types/src/spec/pipeline.rs`.
const PRESETS: readonly Preset[] = [
  {
    label: 'sns',
    spec: {
      frame_style: 'standard',
      theme: 'paper',
      layout: 'edges',
      meta_policy: 'auto',
      jpeg_quality: 78,
      max_long_edge: 2048,
    },
  },
  {
    label: 'standard',
    spec: {
      frame_style: 'standard',
      theme: 'paper',
      layout: 'edges',
      meta_policy: 'auto',
      jpeg_quality: 92,
      max_long_edge: null,
    },
  },
  {
    label: 'maximum',
    spec: {
      frame_style: 'standard',
      theme: 'paper',
      layout: 'edges',
      meta_policy: 'auto',
      jpeg_quality: 98,
      max_long_edge: null,
    },
  },
];

const findPreset = (label: string): Preset => {
  const p = PRESETS.find((it) => it.label === label);
  if (!p) throw new Error(`test fixture: no preset "${label}"`);
  return p;
};

describe('createAppSettings', () => {
  test('starts with sensible defaults (standard preset)', () => {
    createRoot((dispose) => {
      const [src] = createSignal<number | null>(null);
      const s = createAppSettings({ presets: PRESETS, sourceLongEdge: src });
      expect(s.state.preset()).toBe('standard');
      expect(s.state.quality()).toBe(findPreset('standard').spec.jpeg_quality);
      expect(s.state.longEdge()).toBe('full');
      expect(s.state.theme()).toBe('paper');
      expect(s.state.layout()).toBe('edges');
      expect(s.state.metaPolicy()).toBe('auto');
      expect(s.state.effectiveMaxLongEdge()).toBeNull();
      dispose();
    });
  });

  test('applyPreset bumps preset + quality + longEdge atomically', () => {
    createRoot((dispose) => {
      const [src] = createSignal<number | null>(null);
      const s = createAppSettings({ presets: PRESETS, sourceLongEdge: src });
      s.actions.applyPreset('sns');
      expect(s.state.preset()).toBe('sns');
      expect(s.state.quality()).toBe(findPreset('sns').spec.jpeg_quality);
      // SNS's `max_long_edge` is 2048; the picker snaps to FHD (1920 — the
      // closest cap ≤ 2048) per `longEdgeKeyFor` semantics. The effective
      // cap that flows to WASM is 1920, still within the preset's promise.
      expect(s.state.longEdge()).toBe('fhd');
      expect(s.state.effectiveMaxLongEdge()).toBe(1920);
      dispose();
    });
  });

  test('auto-demote snaps Long-edge down when source is smaller than the cap', async () => {
    await new Promise<void>((resolve) => {
      createRoot(async (dispose) => {
        const [src, setSrc] = createSignal<number | null>(null);
        const s = createAppSettings({ presets: PRESETS, sourceLongEdge: src });
        s.actions.setLongEdge('4k'); // 3840 cap
        expect(s.state.longEdge()).toBe('4k');
        // Source smaller than HD cap (1280) falls all the way back to Full.
        setSrc(900);
        // Solid's createEffect fires on the microtask queue — flush it.
        await Promise.resolve();
        expect(s.state.longEdge()).toBe('full');
        dispose();
        resolve();
      });
    });
  });

  test('auto-demote chooses largest cap ≤ source long-edge', async () => {
    await new Promise<void>((resolve) => {
      createRoot(async (dispose) => {
        const [src, setSrc] = createSignal<number | null>(null);
        const s = createAppSettings({ presets: PRESETS, sourceLongEdge: src });
        s.actions.setLongEdge('4k');
        // 2000 px: FHD (1920) is the largest cap that fits.
        setSrc(2000);
        await Promise.resolve();
        expect(s.state.longEdge()).toBe('fhd');
        dispose();
        resolve();
      });
    });
  });

  test('auto-demote leaves selection alone when source ≥ cap', async () => {
    await new Promise<void>((resolve) => {
      createRoot(async (dispose) => {
        const [src, setSrc] = createSignal<number | null>(null);
        const s = createAppSettings({ presets: PRESETS, sourceLongEdge: src });
        s.actions.setLongEdge('fhd');
        setSrc(8000);
        await Promise.resolve();
        expect(s.state.longEdge()).toBe('fhd');
        dispose();
        resolve();
      });
    });
  });

  test('buildSpec reflects current settings + caller-supplied cap', () => {
    createRoot((dispose) => {
      const [src] = createSignal<number | null>(null);
      const s = createAppSettings({ presets: PRESETS, sourceLongEdge: src });
      s.actions.setTheme('ink');
      s.actions.setLayout('centered');
      s.actions.setMetaPolicy('never');
      expect(s.state.buildSpec(1920)).toEqual({
        frame_style: 'standard',
        theme: 'ink',
        layout: 'centered',
        meta_policy: 'never',
        jpeg_quality: findPreset('standard').spec.jpeg_quality,
        max_long_edge: 1920,
      });
      expect(s.state.buildSpec(null)).toEqual({
        frame_style: 'standard',
        theme: 'ink',
        layout: 'centered',
        meta_policy: 'never',
        jpeg_quality: findPreset('standard').spec.jpeg_quality,
        max_long_edge: null,
      });
      dispose();
    });
  });

  test('buildSpec picks up the current quality after applyPreset', () => {
    createRoot((dispose) => {
      const [src] = createSignal<number | null>(null);
      const s = createAppSettings({ presets: PRESETS, sourceLongEdge: src });
      s.actions.applyPreset('maximum');
      expect(s.state.buildSpec(null)).toEqual({
        frame_style: 'standard',
        theme: 'paper',
        layout: 'edges',
        meta_policy: 'auto',
        jpeg_quality: findPreset('maximum').spec.jpeg_quality,
        max_long_edge: null,
      });
      dispose();
    });
  });

  test('setFrameStyle flips the silhouette and threads through buildSpec', () => {
    createRoot((dispose) => {
      const [src] = createSignal<number | null>(null);
      const s = createAppSettings({ presets: PRESETS, sourceLongEdge: src });
      expect(s.state.frameStyle()).toBe('standard');
      s.actions.setFrameStyle('polaroid');
      expect(s.state.frameStyle()).toBe('polaroid');
      expect(s.state.buildSpec(null).frame_style).toBe('polaroid');
      dispose();
    });
  });

  test('individual setters update the matching accessor', () => {
    createRoot((dispose) => {
      const [src] = createSignal<number | null>(null);
      const s = createAppSettings({ presets: PRESETS, sourceLongEdge: src });
      s.actions.setTheme('ink');
      s.actions.setLayout('centered');
      s.actions.setMetaPolicy('never');
      s.actions.setLongEdge('hd');
      expect(s.state.theme()).toBe('ink');
      expect(s.state.layout()).toBe('centered');
      expect(s.state.metaPolicy()).toBe('never');
      expect(s.state.longEdge()).toBe('hd');
      expect(s.state.effectiveMaxLongEdge()).toBe(1280);
      dispose();
    });
  });

  test('throws when the preset table is empty', () => {
    createRoot((dispose) => {
      const [src] = createSignal<number | null>(null);
      expect(() => createAppSettings({ presets: [], sourceLongEdge: src })).toThrow(
        /presets\[\] must be non-empty/,
      );
      dispose();
    });
  });

  test('applyPreset throws for an unknown label', () => {
    createRoot((dispose) => {
      const [src] = createSignal<number | null>(null);
      const s = createAppSettings({ presets: PRESETS, sourceLongEdge: src });
      expect(() => s.actions.applyPreset('archival')).toThrow(/no preset named "archival"/);
      dispose();
    });
  });
});
