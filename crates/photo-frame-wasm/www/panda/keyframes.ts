// Animation keyframes. Defined as a Panda theme primitive (typed
// at codegen) so consumers reference them by name through
// `animation: 'name 1s ease'` — same vocabulary as tokens.
//
// Resist adding keyframes that don't serve a specific UI need; an
// idle palette of unused animations is the kind of dead code that
// quietly accretes.
export const keyframes = {
  // Pulse — gentle opacity breathing used by:
  //   - batch gallery thumbnail placeholders while their preview
  //     renders
  //   - batch gallery cards while their full-resolution pass is
  //     in flight
  // Loops indefinitely; consumers stop the animation by removing
  // the class once the underlying work finishes.
  'gallery-pulse': {
    '0%, 100%': { opacity: '0.45' },
    '50%': { opacity: '1' },
  },
} as const;
