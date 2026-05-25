// Caption picker — collapses the previous (`Layout` segmented +
// `Metadata` checkbox) pair into a single 3-state choice. The
// internal `layout` + `showMeta` signals stay the source of
// truth so the WASM render options struct is unchanged; this
// module owns the bidirectional mapping that lets a single
// segmented drive both.

import type { CaptionLayout } from '../frame-client';

/** UI-facing union: either no caption at all, or one of the
 *  two layout arrangements. */
export type CaptionMode = 'off' | CaptionLayout;

export const CAPTION_MODES = [
  { value: 'off' as const, label: 'Off', description: 'No metadata caption' },
  { value: 'edges' as const, label: 'Edges', description: 'Four-corner liit-style layout' },
  {
    value: 'centered' as const,
    label: 'Centered',
    description: 'Both rows centred under the photo',
  },
] satisfies ReadonlyArray<{ value: CaptionMode; label: string; description: string }>;

/** Project (layout, showMeta) → the segmented control's
 *  display value. `showMeta: false` always reads as `'off'`
 *  regardless of layout. */
export const toCaptionMode = ({
  layout,
  showMeta,
}: {
  layout: CaptionLayout;
  showMeta: boolean;
}): CaptionMode => (showMeta ? layout : 'off');

/** Inverse projection: a CaptionMode pick → the underlying
 *  (layout, showMeta) update. When the user picks `'off'` we
 *  *keep* the previous layout so flipping back to
 *  `'edges'`/`'centered'` later restores their last pick
 *  without losing context. */
export const fromCaptionMode = (
  mode: CaptionMode,
  prevLayout: CaptionLayout,
): { layout: CaptionLayout; showMeta: boolean } => {
  if (mode === 'off') return { layout: prevLayout, showMeta: false };
  return { layout: mode, showMeta: true };
};
