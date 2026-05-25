// Generation gate — the pattern `App.tsx` uses three times to
// fence async work against superseding requests. When the user
// drops a new image while an old `preparePixels` promise is
// still in flight, the gate's number bumps; the stale completion
// reads `isCurrent(gen) === false` and exits without touching
// signals. Without this fence, a slow decode could overwrite
// the freshly-dropped image's preview with the old one.
//
// Pulled out so the three call sites share one tested
// implementation instead of three copy-pasted `let gen = 0`
// blocks that each need their own invalidation rule.

export type GenerationGate = {
  /** Bump the gate; returns the new generation number for the
   *  caller to remember and check via `isCurrent`. */
  bump: () => number;
  /** Does `gen` still match the current generation? Returns
   *  `false` once `bump` has been called since `gen` was
   *  issued — the caller's completion handler should bail out. */
  isCurrent: (gen: number) => boolean;
  /** Read-only access to the current generation. */
  current: () => number;
};

export const createGenerationGate = (): GenerationGate => {
  let gen = 0;
  return {
    bump: (): number => {
      gen += 1;
      return gen;
    },
    isCurrent: (g: number): boolean => g === gen,
    current: (): number => gen,
  };
};
