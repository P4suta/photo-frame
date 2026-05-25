// Contain-fit a source rectangle inside a target stage. Pulled
// out of `App.tsx`'s preview frame math so the cases that
// keep regressing (portrait vs landscape, zero-stage, tiny
// stage) get fixed once and pinned by tests.

export type Size = { width: number; height: number };

export type ContainFitInput = {
  /** Source long-axis pixel count. */
  srcW: number;
  /** Source short-axis pixel count. */
  srcH: number;
  /** Available stage width in CSS pixels. */
  stageW: number;
  /** Available stage height in CSS pixels. */
  stageH: number;
};

/** Returns the largest `{width, height}` that fits inside
 *  `stageW × stageH` while preserving `srcW / srcH`'s aspect.
 *  Returns `null` when the stage hasn't been measured yet
 *  (= caller hasn't laid out the parent yet) so the caller
 *  knows to hide the wrapper instead of painting it at zero.
 *
 *  Output is `Math.floor`'d so the wrapper never exceeds the
 *  stage on either axis (a fractional pixel rounding up was
 *  the previous portrait-overflow bug). */
export const containFit = ({ srcW, srcH, stageW, stageH }: ContainFitInput): Size | null => {
  if (srcW <= 0 || srcH <= 0 || stageW <= 0 || stageH <= 0) return null;
  const srcAspect = srcW / srcH;
  // Width-driven fit: take the full stage width and derive
  // height. If that height fits inside the stage, the width
  // axis is the binding constraint; otherwise the height axis
  // is. Either way, the dimension that fits last wins.
  const hIfWidthFull = stageW / srcAspect;
  const fitW = hIfWidthFull <= stageH ? stageW : stageH * srcAspect;
  const fitH = fitW / srcAspect;
  return { width: Math.floor(fitW), height: Math.floor(fitH) };
};
