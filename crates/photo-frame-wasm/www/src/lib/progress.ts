/**
 * Mapping from Rust pipeline stage completion → cumulative progress
 * percent for a single batch item.
 *
 * The weights come from `BENCHMARKS.md` (post-D3): decode ≈ 33%,
 * frame ≈ 3%, encode ≈ 64% of wall-clock for a 24 MP image. The
 * cumulative percent at each stage boundary is rounded to whole
 * numbers so the progress bar reads cleanly:
 *
 *  | stage completed | cumulative % |
 *  | --------------- | ------------ |
 *  | (none yet)      | 0            |
 *  | decode          | 33           |
 *  | frame           | 36           |
 *  | encode          | 100          |
 *
 * Adjust when `BENCHMARKS.md` numbers drift materially — the bar's
 * visual smoothness depends on the weights matching real timings.
 */
export type Stage = 'decode' | 'frame' | 'encode';

const STAGE_PERCENT: Record<Stage, number> = {
  decode: 33,
  frame: 36,
  encode: 100,
};

/**
 * Cumulative percent (0..100) after the given stage completes.
 * Returns 0 for any unknown stage label (defensive — the Rust side
 * is supposed to send one of the three literals above).
 */
export const stageToPercent = (stage: string): number => {
  return Object.hasOwn(STAGE_PERCENT, stage) ? STAGE_PERCENT[stage as Stage] : 0;
};
