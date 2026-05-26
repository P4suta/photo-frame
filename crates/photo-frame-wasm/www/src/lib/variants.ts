// Preview variant cache key — encodes the (frameStyle, theme,
// layout, metaPolicy) tuple as a stable string. The prefetch loop
// uses it to index a Map of pre-rendered previews so toggling any
// of the four doesn't re-pay the WASM round-trip.
//
// All four axes are independent (FrameStyle picks the silhouette,
// CaptionLayout picks how text is arranged inside whichever caption
// region the silhouette provides), so the cardinality is the full
// 2 × 2 × 2 × 2 = 16 combinations.

import type { CaptionLayout, FrameStyle, FrameTheme, MetaPolicy } from '../frame-client';

export type VariantKey = string;

/** Stable string key for the variant tuple.
 *  Pipe delimiter — none of the legitimate values contain `|`. */
export const variantKey = (
  frameStyle: FrameStyle,
  theme: FrameTheme,
  layout: CaptionLayout,
  metaPolicy: MetaPolicy,
): VariantKey => `${frameStyle}|${theme}|${layout}|${metaPolicy}`;

/** Inverse of `variantKey`. Returns null if the string isn't a
 *  well-formed key (= wrong field count, unknown enum member). */
export const parseVariantKey = (
  key: VariantKey,
): {
  frameStyle: FrameStyle;
  theme: FrameTheme;
  layout: CaptionLayout;
  metaPolicy: MetaPolicy;
} | null => {
  const parts = key.split('|');
  if (parts.length !== 4) return null;
  const [s, t, l, m] = parts;
  if (s !== 'standard' && s !== 'polaroid') return null;
  if (t !== 'paper' && t !== 'ink') return null;
  if (l !== 'edges' && l !== 'centered') return null;
  if (m !== 'auto' && m !== 'never') return null;
  return { frameStyle: s, theme: t, layout: l, metaPolicy: m };
};

/** Every distinct (frameStyle, theme, layout, metaPolicy) combination
 *  the renderer produces. 2 (frameStyles) × 2 (themes) × 2 (layouts)
 *  × 2 (metaPolicies) = 16 entries. */
export const ALL_VARIANTS: ReadonlyArray<{
  frameStyle: FrameStyle;
  theme: FrameTheme;
  layout: CaptionLayout;
  metaPolicy: MetaPolicy;
}> = (() => {
  const out: {
    frameStyle: FrameStyle;
    theme: FrameTheme;
    layout: CaptionLayout;
    metaPolicy: MetaPolicy;
  }[] = [];
  for (const frameStyle of ['standard', 'polaroid'] as const) {
    for (const theme of ['paper', 'ink'] as const) {
      for (const layout of ['edges', 'centered'] as const) {
        for (const metaPolicy of ['auto', 'never'] as const) {
          out.push({ frameStyle, theme, layout, metaPolicy });
        }
      }
    }
  }
  return out;
})();
