// Long-edge picker options + the helpers that snap user / preset
// values to one of those keys.
//
// `LONG_EDGE_OPTIONS` is the UI Long-edge segmented's source of
// truth (Full / 4K / FHD / HD). The Rust-side `PipelineSpec`'s
// presets carry their own numeric `max_long_edge` caps that flow
// across the WASM boundary as `Preset[]`; `longEdgeKeyFor` projects
// such a cap into the closest UI key so the picker shows a
// meaningful selection after a preset click.

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

/** Project a numeric `maxLongEdge` (typically from a Rust
 *  `PipelineSpec`'s preset) onto the closest UI `LongEdgeKey`
 *  whose cap is `≤` the target. Returns `'full'` for `null`
 *  inputs and for any target smaller than every numeric option
 *  (since `full`'s cap of `null` is always a valid superset).
 *
 *  The "closest cap ≤ target" semantic lets Rust evolve preset
 *  values (e.g. `SNS::max_long_edge` raised from 1920 → 2048)
 *  without breaking the picker: a 2048-cap preset selects FHD
 *  (1920), still within the preset's promise. */
export const longEdgeKeyFor = (maxLongEdge: number | null): LongEdgeKey => {
  if (maxLongEdge === null) return 'full';
  let best: LongEdgeKey = 'full';
  let bestCap = -1;
  for (const k of Object.keys(LONG_EDGE_OPTIONS) as LongEdgeKey[]) {
    const v = LONG_EDGE_OPTIONS[k].maxLongEdge;
    if (v !== null && v <= maxLongEdge && v > bestCap) {
      best = k;
      bestCap = v;
    }
  }
  return best;
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
 *  larger than the source can deliver, snap to the largest valid
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

/** UI display name for a canonical Rust preset label. The
 *  segmented control shows these strings; the `label` field on
 *  the Rust-side `Preset` is the kebab-case canonical identifier.
 *  Unknown labels (a new Rust preset that hasn't landed here yet)
 *  fall back to the raw label rather than throwing, so a Rust
 *  addition surfaces as a visible-but-unstyled control rather
 *  than a runtime error. */
const PRESET_DISPLAY_NAMES: Record<string, string> = {
  sns: 'SNS',
  standard: 'Standard',
  maximum: 'Maximum',
};

export const presetDisplayName = (label: string): string => PRESET_DISPLAY_NAMES[label] ?? label;
