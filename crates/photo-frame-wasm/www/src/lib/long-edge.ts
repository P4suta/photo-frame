// Resolution / preset lookup tables — pulled out of `App.tsx`
// so the table values and the helpers that consume them can be
// unit-tested without spinning up the SolidJS reactive scope.

/** Quality presets — mirror of `photo_frame::QualityPreset` on
 *  the Rust side. Each preset rolls up two settings the user
 *  doesn't have to think about individually: JPEG quality and
 *  the source long-edge cap. The Rust side is the source of
 *  truth; this duplication keeps preset clicks UI-responsive
 *  (no WASM round-trip just to expand the preset). */
export const PRESETS = {
  sns: { label: 'SNS', quality: 78, maxLongEdge: 1920 as number | null },
  standard: { label: 'Standard', quality: 92, maxLongEdge: null as number | null },
  maximum: { label: 'Maximum', quality: 98, maxLongEdge: null as number | null },
} as const satisfies Record<string, { label: string; quality: number; maxLongEdge: number | null }>;

export type PresetKey = keyof typeof PRESETS;

/** Long-edge size choices the user picks from in the
 *  Resolution segmented. `Full` (= null cap) is the source-
 *  size-unchanged path; the rest cap at a recognisable
 *  display-size target. */
export const LONG_EDGE_OPTIONS = {
  full: { label: 'Full', maxLongEdge: null as number | null },
  '4k': { label: '4K', maxLongEdge: 3840 as number | null },
  fhd: { label: 'FHD', maxLongEdge: 1920 as number | null },
  hd: { label: 'HD', maxLongEdge: 1280 as number | null },
} as const satisfies Record<string, { label: string; maxLongEdge: number | null }>;

export type LongEdgeKey = keyof typeof LONG_EDGE_OPTIONS;

/** Map a preset's numeric `maxLongEdge` onto the closest
 *  `LongEdgeKey`. Equality matching only — the preset table is
 *  intentionally aligned with the LONG_EDGE_OPTIONS values, so
 *  a strict match is correct. Falls back to `'full'` for any
 *  cap that doesn't appear in the options table (including
 *  `null`, which `LONG_EDGE_OPTIONS.full.maxLongEdge` matches
 *  exactly anyway). */
export const longEdgeKeyFor = (maxLongEdge: number | null): LongEdgeKey => {
  for (const [key, info] of Object.entries(LONG_EDGE_OPTIONS)) {
    if (info.maxLongEdge === maxLongEdge) return key as LongEdgeKey;
  }
  return 'full';
};

/** Derive the *effective* source long edge from the current
 *  session. In single mode that's the loaded image's measured
 *  long edge; in batch mode it's the smallest across the set
 *  (so a cap promise never exceeds what the weakest source
 *  can deliver). Returns null when the session is empty so the
 *  Resolution picker can leave all options enabled. */
export const sourceLongEdgeOf = (
  single: { longEdge: number } | null,
  batch: ReadonlyArray<{ longEdge: number }> | null,
): number | null => {
  if (single) return single.longEdge;
  if (batch && batch.length > 0) {
    return Math.min(...batch.map((f) => f.longEdge));
  }
  return null;
};

/** Pick the appropriate Long-edge key given a measured source
 *  size and the currently-selected key. If the selected cap is
 *  larger than the source can deliver, snap to the largest
 *  numeric cap that *does* fit; if even HD is larger than the
 *  source, fall back to `'full'` (always valid — its "cap" is
 *  null = source-size). When `sourceLongEdge` is null (= no
 *  measured source yet) the current selection is kept as-is. */
export const pickAutoDemoteKey = (
  sourceLongEdge: number | null,
  current: LongEdgeKey,
): LongEdgeKey => {
  if (sourceLongEdge === null) return current;
  const cap = LONG_EDGE_OPTIONS[current].maxLongEdge;
  if (cap === null || cap <= sourceLongEdge) return current;
  let best: LongEdgeKey = 'full';
  let bestCap = -1;
  for (const k of Object.keys(LONG_EDGE_OPTIONS) as LongEdgeKey[]) {
    const v = LONG_EDGE_OPTIONS[k].maxLongEdge;
    if (v !== null && v <= sourceLongEdge && v > bestCap) {
      best = k;
      bestCap = v;
    }
  }
  return best;
};
