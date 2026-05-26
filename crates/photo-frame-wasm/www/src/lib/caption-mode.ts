// Caption picker — collapses the previous (`Layout` segmented +
// `Metadata` checkbox) pair into a single 3-state choice. The
// internal `layout` + `metaPolicy` signals stay the source of
// truth so the WASM render options struct is unchanged; this
// module owns the bidirectional mapping that lets a single
// segmented drive both.
//
// Frame silhouette (Standard vs Polaroid) is a separate concern —
// it lives in `FrameStyle` and has its own picker. When Polaroid
// is selected the caption arrangement choice is ignored by the
// renderer (Polaroid always centres its caption), but Caption Off
// still suppresses the text.

import type { CaptionLayout, MetaPolicy } from '../frame-client';

/** UI-facing union: either no caption at all, or one of the
 *  layout arrangements. */
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

/** Project (layout, metaPolicy) → the segmented control's
 *  display value. `metaPolicy: 'never'` always reads as `'off'`
 *  regardless of layout. */
export const toCaptionMode = ({
  layout,
  metaPolicy,
}: {
  layout: CaptionLayout;
  metaPolicy: MetaPolicy;
}): CaptionMode => (metaPolicy === 'auto' ? layout : 'off');

/** Inverse projection: a CaptionMode pick → the underlying
 *  (layout, metaPolicy) update. When the user picks `'off'` we
 *  *keep* the previous layout so flipping back to a layout mode
 *  later restores their last pick without losing context. */
export const fromCaptionMode = (
  mode: CaptionMode,
  prevLayout: CaptionLayout,
): { layout: CaptionLayout; metaPolicy: MetaPolicy } => {
  if (mode === 'off') return { layout: prevLayout, metaPolicy: 'never' };
  return { layout: mode, metaPolicy: 'auto' };
};
