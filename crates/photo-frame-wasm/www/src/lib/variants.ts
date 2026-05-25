// Preview variant cache key — encodes the (theme, layout,
// showMeta) triple as a stable string. The prefetch loop uses
// it to index a Map of pre-rendered previews so toggling any
// of the three doesn't re-pay the WASM round-trip.

import type { CaptionLayout, FrameTheme } from '../frame-client';

export type VariantKey = string;

/** Stable string key for the (theme, layout, showMeta) tuple.
 *  Pipe delimiter — none of the legitimate values contain `|`. */
export const variantKey = (
  theme: FrameTheme,
  layout: CaptionLayout,
  showMeta: boolean,
): VariantKey => `${theme}|${layout}|${showMeta}`;

/** Inverse of `variantKey`. Returns null if the string isn't a
 *  well-formed key (= wrong field count, unknown enum member). */
export const parseVariantKey = (
  key: VariantKey,
): { theme: FrameTheme; layout: CaptionLayout; showMeta: boolean } | null => {
  const parts = key.split('|');
  if (parts.length !== 3) return null;
  const [t, l, s] = parts;
  if (t !== 'paper' && t !== 'ink') return null;
  if (l !== 'edges' && l !== 'centered') return null;
  if (s !== 'true' && s !== 'false') return null;
  return { theme: t, layout: l, showMeta: s === 'true' };
};

/** Every (theme, layout, showMeta) combination — used by the
 *  prefetch loop to enumerate caches to fill in a deterministic
 *  order. 2 themes × 2 layouts × 2 show-meta states = 8 entries. */
export const ALL_VARIANTS: ReadonlyArray<{
  theme: FrameTheme;
  layout: CaptionLayout;
  showMeta: boolean;
}> = (() => {
  const out: { theme: FrameTheme; layout: CaptionLayout; showMeta: boolean }[] = [];
  for (const theme of ['paper', 'ink'] as const) {
    for (const layout of ['edges', 'centered'] as const) {
      for (const showMeta of [true, false] as const) {
        out.push({ theme, layout, showMeta });
      }
    }
  }
  return out;
})();
