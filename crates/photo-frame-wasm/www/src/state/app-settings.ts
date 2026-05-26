/**
 * Solid-reactive primitive owning every user-facing render setting.
 *
 * Bundles the six independent signals (preset, quality, longEdge, theme,
 * layout, metaPolicy) that the sidebar controls drive, along with their
 * derived helper (`effectiveMaxLongEdge`, `buildSpec`) and the
 * auto-demote effect that snaps the Long-edge picker down when the
 * loaded source can't honour the currently-selected cap.
 *
 * The canonical render shape `PipelineSpec` is the Rust source of truth
 * (`pkg/photo_frame_wasm.d.ts`); `buildSpec()` projects the live signals
 * into that shape. The preset table arrives via `deps.presets` —
 * `loadPresets()` queries `getPresets()` once at app boot so JS doesn't
 * carry a duplicate copy of the Rust `PipelineSpec::PRESETS` data.
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
  FrameStyle,
  FrameTheme,
  MetaPolicy,
  PipelineSpec,
  Preset,
} from '../frame-client';
import {
  LONG_EDGE_OPTIONS,
  type LongEdgeKey,
  longEdgeKeyFor,
  pickAutoDemoteKey,
} from '../lib/long-edge';

/** Default preset label expected to exist in the Rust-side
 *  `PipelineSpec::PRESETS` table. If a future Rust refactor renames
 *  this, the bootstrap `applyPreset` falls back to the first preset
 *  in the array so the UI keeps booting. */
const DEFAULT_PRESET_LABEL = 'standard';

export type AppSettings = {
  state: {
    preset: Accessor<string>;
    quality: Accessor<number>;
    longEdge: Accessor<LongEdgeKey>;
    frameStyle: Accessor<FrameStyle>;
    theme: Accessor<FrameTheme>;
    layout: Accessor<CaptionLayout>;
    metaPolicy: Accessor<MetaPolicy>;
    /** Cached `LONG_EDGE_OPTIONS[longEdge].maxLongEdge`. */
    effectiveMaxLongEdge: Accessor<number | null>;
    /** Live presets table fetched from Rust truth at boot. */
    presets: Accessor<readonly Preset[]>;
    /** Project the current frame settings into the canonical
     *  `PipelineSpec` shape with the supplied `max_long_edge`. */
    buildSpec: (maxLongEdge: number | null) => PipelineSpec;
  };
  actions: {
    /** Atomic bump: preset → matching quality + matching longEdge. */
    applyPreset: (label: string) => void;
    setLongEdge: (k: LongEdgeKey) => void;
    setFrameStyle: (s: FrameStyle) => void;
    setTheme: (t: FrameTheme) => void;
    setLayout: (l: CaptionLayout) => void;
    setMetaPolicy: (m: MetaPolicy) => void;
  };
};

export const createAppSettings = (deps: {
  /** Rust-side preset table, fetched once at app boot via
   *  `loadPresets()`. The array must be non-empty — if the Rust
   *  presets vanished, the bootstrap would have no defaults to
   *  apply. */
  presets: readonly Preset[];
  /** Live source long-edge accessor — drives auto-demote. */
  sourceLongEdge: Accessor<number | null>;
}): AppSettings => {
  if (deps.presets.length === 0) {
    throw new Error('createAppSettings: presets[] must be non-empty');
  }

  // Resolve the default preset row up front so the initial signal
  // values mirror Rust truth rather than a stale JS-side guess.
  const defaultPreset =
    deps.presets.find((p) => p.label === DEFAULT_PRESET_LABEL) ?? deps.presets[0];
  // The `?? deps.presets[0]` above already guarantees this; the
  // non-null assertion here is the type-system tax for that.
  const initial = defaultPreset as Preset;

  const [preset, setPreset] = createSignal<string>(initial.label);
  const [quality, setQuality] = createSignal<number>(initial.spec.jpeg_quality);
  const [longEdge, setLongEdge] = createSignal<LongEdgeKey>(
    longEdgeKeyFor(initial.spec.max_long_edge),
  );
  const [frameStyle, setFrameStyle] = createSignal<FrameStyle>(initial.spec.frame_style);
  const [theme, setTheme] = createSignal<FrameTheme>(initial.spec.theme);
  const [layout, setLayout] = createSignal<CaptionLayout>(initial.spec.layout);
  const [metaPolicy, setMetaPolicy] = createSignal<MetaPolicy>(initial.spec.meta_policy);

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

  const applyPreset = (label: string): void => {
    const p = deps.presets.find((it) => it.label === label);
    if (!p) {
      throw new Error(`applyPreset: no preset named "${label}" in Rust truth`);
    }
    setPreset(label);
    setQuality(p.spec.jpeg_quality);
    setLongEdge(longEdgeKeyFor(p.spec.max_long_edge));
  };

  const buildSpec = (maxLongEdge: number | null): PipelineSpec => ({
    frame_style: frameStyle(),
    theme: theme(),
    layout: layout(),
    meta_policy: metaPolicy(),
    jpeg_quality: quality(),
    max_long_edge: maxLongEdge,
  });

  const presets = (): readonly Preset[] => deps.presets;

  return {
    state: {
      preset,
      quality,
      longEdge,
      frameStyle,
      theme,
      layout,
      metaPolicy,
      effectiveMaxLongEdge,
      presets,
      buildSpec,
    },
    actions: {
      applyPreset,
      setLongEdge,
      setFrameStyle,
      setTheme,
      setLayout,
      setMetaPolicy,
    },
  };
};
