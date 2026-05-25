import { createRoot, createSignal } from 'solid-js';
import { describe, expect, test } from 'vitest';
import { PRESETS } from '../lib/long-edge';
import { createAppSettings } from './app-settings';

describe('createAppSettings', () => {
  test('starts with sensible defaults (standard preset)', () => {
    createRoot((dispose) => {
      const [src] = createSignal<number | null>(null);
      const s = createAppSettings({ sourceLongEdge: src });
      expect(s.state.preset()).toBe('standard');
      expect(s.state.quality()).toBe(PRESETS.standard.quality);
      expect(s.state.longEdge()).toBe('full');
      expect(s.state.theme()).toBe('paper');
      expect(s.state.layout()).toBe('edges');
      expect(s.state.showMeta()).toBe(true);
      expect(s.state.effectiveMaxLongEdge()).toBeNull();
      dispose();
    });
  });

  test('applyPreset bumps preset + quality + longEdge atomically', () => {
    createRoot((dispose) => {
      const [src] = createSignal<number | null>(null);
      const s = createAppSettings({ sourceLongEdge: src });
      s.actions.applyPreset('sns');
      expect(s.state.preset()).toBe('sns');
      expect(s.state.quality()).toBe(PRESETS.sns.quality);
      expect(s.state.longEdge()).toBe('fhd');
      expect(s.state.effectiveMaxLongEdge()).toBe(1920);
      dispose();
    });
  });

  test('auto-demote snaps Long-edge down when source is smaller than the cap', async () => {
    await new Promise<void>((resolve) => {
      createRoot(async (dispose) => {
        const [src, setSrc] = createSignal<number | null>(null);
        const s = createAppSettings({ sourceLongEdge: src });
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
        const s = createAppSettings({ sourceLongEdge: src });
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
        const s = createAppSettings({ sourceLongEdge: src });
        s.actions.setLongEdge('fhd');
        setSrc(8000);
        await Promise.resolve();
        expect(s.state.longEdge()).toBe('fhd');
        dispose();
        resolve();
      });
    });
  });

  test('buildFrameOptions reflects current settings + caller-supplied cap', () => {
    createRoot((dispose) => {
      const [src] = createSignal<number | null>(null);
      const s = createAppSettings({ sourceLongEdge: src });
      s.actions.setTheme('ink');
      s.actions.setLayout('centered');
      s.actions.setShowMeta(false);
      expect(s.state.buildFrameOptions(1920)).toEqual({
        theme: 'ink',
        layout: 'centered',
        show_meta: false,
        max_long_edge: 1920,
      });
      expect(s.state.buildFrameOptions(null)).toEqual({
        theme: 'ink',
        layout: 'centered',
        show_meta: false,
        max_long_edge: null,
      });
      dispose();
    });
  });

  test('buildPipelineOptions composes buildFrameOptions + current quality', () => {
    createRoot((dispose) => {
      const [src] = createSignal<number | null>(null);
      const s = createAppSettings({ sourceLongEdge: src });
      s.actions.applyPreset('maximum');
      expect(s.state.buildPipelineOptions(null)).toEqual({
        theme: 'paper',
        layout: 'edges',
        show_meta: true,
        max_long_edge: null,
        jpeg_quality: PRESETS.maximum.quality,
      });
      dispose();
    });
  });

  test('individual setters update the matching accessor', () => {
    createRoot((dispose) => {
      const [src] = createSignal<number | null>(null);
      const s = createAppSettings({ sourceLongEdge: src });
      s.actions.setTheme('ink');
      s.actions.setLayout('centered');
      s.actions.setShowMeta(false);
      s.actions.setLongEdge('hd');
      expect(s.state.theme()).toBe('ink');
      expect(s.state.layout()).toBe('centered');
      expect(s.state.showMeta()).toBe(false);
      expect(s.state.longEdge()).toBe('hd');
      expect(s.state.effectiveMaxLongEdge()).toBe(1280);
      dispose();
    });
  });
});
