/**
 * Solid-reactive primitive owning every user-facing render setting.
 *
 * Bundles the six independent signals (preset, quality, longEdge, theme,
 * layout, showMeta) that the sidebar controls drive, along with their
 * derived helpers (`effectiveMaxLongEdge`, `buildFrameOptions`,
 * `buildPipelineOptions`) and the auto-demote effect that snaps the
 * Long-edge picker down when the loaded source can't honour the
 * currently-selected cap.
 *
 * Consumers (preview/batch sessions, SidebarControls) read through the
 * `.state` accessor surface and mutate through `.actions` — the raw
 * `createSignal` setters never leak out, so a UI cannot bypass
 * `applyPreset` (which has to bump three settings atomically) by
 * setting `quality` directly.
 */

import { type Accessor, createEffect, createMemo, createSignal } from 'solid-js';
import type {
  CaptionLayout,
  FrameOptionsForPrepare,
  FrameTheme,
  PipelineOptions,
} from '../frame-client';
import {
  LONG_EDGE_OPTIONS,
  type LongEdgeKey,
  longEdgeKeyFor,
  pickAutoDemoteKey,
  PRESETS,
  type PresetKey,
} from '../lib/long-edge';

export type AppSettings = {
  state: {
    preset: Accessor<PresetKey>;
    quality: Accessor<number>;
    longEdge: Accessor<LongEdgeKey>;
    theme: Accessor<FrameTheme>;
    layout: Accessor<CaptionLayout>;
    showMeta: Accessor<boolean>;
    /** Cached `LONG_EDGE_OPTIONS[longEdge].maxLongEdge`. */
    effectiveMaxLongEdge: Accessor<number | null>;
    /** Project the current frame settings into the worker's
     *  `FrameOptionsForPrepare` shape with the supplied cap. */
    buildFrameOptions: (maxLongEdge: number | null) => FrameOptionsForPrepare;
    /** Same as `buildFrameOptions` plus the JPEG quality —
     *  matches the worker's `PipelineOptions` shape. */
    buildPipelineOptions: (maxLongEdge: number | null) => PipelineOptions;
  };
  actions: {
    /** Atomic bump: preset → matching quality + matching longEdge. */
    applyPreset: (k: PresetKey) => void;
    setLongEdge: (k: LongEdgeKey) => void;
    setTheme: (t: FrameTheme) => void;
    setLayout: (l: CaptionLayout) => void;
    setShowMeta: (v: boolean) => void;
  };
};

export const createAppSettings = (deps: {
  /** Live source long-edge accessor — drives auto-demote. */
  sourceLongEdge: Accessor<number | null>;
}): AppSettings => {
  const [preset, setPreset] = createSignal<PresetKey>('standard');
  const [quality, setQuality] = createSignal<number>(PRESETS.standard.quality);
  const [longEdge, setLongEdge] = createSignal<LongEdgeKey>('full');
  const [theme, setTheme] = createSignal<FrameTheme>('paper');
  const [layout, setLayout] = createSignal<CaptionLayout>('edges');
  const [showMeta, setShowMeta] = createSignal(true);

  const effectiveMaxLongEdge = createMemo<number | null>(
    () => LONG_EDGE_OPTIONS[longEdge()].maxLongEdge,
  );

  // Auto-demote: if a Long-edge option larger than the source is
  // currently selected, snap to the largest valid option.
  // Without this, the user could "select 4K" and quietly get a
  // Full-resolution output (WASM's max_long_edge is a ceiling,
  // not a target), which reads as a bug.
  createEffect(() => {
    const next = pickAutoDemoteKey(deps.sourceLongEdge(), longEdge());
    if (next !== longEdge()) setLongEdge(next);
  });

  const applyPreset = (k: PresetKey): void => {
    setPreset(k);
    const p = PRESETS[k];
    setQuality(p.quality);
    setLongEdge(longEdgeKeyFor(p.maxLongEdge));
  };

  const buildFrameOptions = (maxLongEdge: number | null): FrameOptionsForPrepare => ({
    theme: theme(),
    layout: layout(),
    show_meta: showMeta(),
    max_long_edge: maxLongEdge,
  });

  const buildPipelineOptions = (maxLongEdge: number | null): PipelineOptions => ({
    ...buildFrameOptions(maxLongEdge),
    jpeg_quality: quality(),
  });

  return {
    state: {
      preset,
      quality,
      longEdge,
      theme,
      layout,
      showMeta,
      effectiveMaxLongEdge,
      buildFrameOptions,
      buildPipelineOptions,
    },
    actions: {
      applyPreset,
      setLongEdge,
      setTheme,
      setLayout,
      setShowMeta,
    },
  };
};
