// Logarithmic golden spiral — pure geometry kept separate from the
// SolidJS component (`GoldenSpiral.tsx`) so the canvas renderer
// stays a thin wrapper and the spiral / rectangle math can be
// pinned against a small set of analytical invariants in unit
// tests.
//
// The spiral is the *true* logarithmic spiral that threads the
// φ-nested rectangles' corners — `r(θ) = ρ₀·exp(k·θ)` with
// `k = ln(φ) / (π/2)`. Each quarter turn (θ += π/2) multiplies
// the radius by φ; one full turn multiplies it by φ⁴. That
// per-turn φ⁴ factor is what the rendering loop exploits to make
// the spiral feel like it grows from the pole forever without a
// visible seam (wrapping θ by 2π and dividing the canvas scale
// by φ⁴ lands on a self-similar frame).

export const PHI = 1.618_033_988_749_895;

// Logarithmic-spiral growth constant — radius is multiplied by
// exp(k · Δθ); choosing k = ln(φ)/(π/2) gives the canonical golden
// spiral where r doubles by φ every quarter turn.
export const K = Math.log(PHI) / (Math.PI / 2);

const TWO_PI = Math.PI * 2;

export type Vec2 = { readonly x: number; readonly y: number };

/** A nested-rectangle quad in (TL, TR, BR, BL) corner order. */
export type Rectangle = readonly [Vec2, Vec2, Vec2, Vec2];

const add = (a: Vec2, b: Vec2): Vec2 => ({ x: a.x + b.x, y: a.y + b.y });
const sub = (a: Vec2, b: Vec2): Vec2 => ({ x: a.x - b.x, y: a.y - b.y });
const mul = (v: Vec2, s: number): Vec2 => ({ x: v.x * s, y: v.y * s });
// 90° rotations of the basis vectors that span each rectangle.
// `rot90` is CCW, `rotN90` is CW — the inward step uses CCW so
// each successive rectangle rotates the same way as the spiral
// itself; the outward step uses CW because we're walking the
// recursion backwards.
const rot90 = (v: Vec2): Vec2 => ({ x: -v.y, y: v.x });
const rotN90 = (v: Vec2): Vec2 => ({ x: v.y, y: -v.x });

// The pole is the convergence point of the rectangle nesting:
// (A, e₁, e₂) starts at the unit-φ rectangle's top-left corner
// and steps inward, shrinking by 1/φ each turn; in the limit A
// lands on the pole. 60 iterations puts the residual below 1e-12
// (each step shrinks by 1/φ ≈ 0.618, so 60 steps multiply the
// error by ≈ 5.4e-13).
const POLE_ITERATIONS = 60;

const iteratePole = (steps: number): Vec2 => {
  let A: Vec2 = { x: 0, y: 0 };
  let e1: Vec2 = { x: PHI, y: 0 };
  let e2: Vec2 = { x: 0, y: 1 };
  for (let i = 0; i < steps; i++) {
    const nA = add(A, e1);
    const nE2 = mul(rot90(e2), 1 / PHI);
    A = nA;
    e1 = e2;
    e2 = nE2;
  }
  return A;
};

/** Convergence point of the rectangle nesting — the centre the
 *  spiral winds into. Computed once at module load. */
export const POLE: Vec2 = iteratePole(POLE_ITERATIONS);

// Anchor vertex: the (0, 1) corner of the unit-φ rectangle. The
// spiral is fit so that `logSpiralPoint(0)` returns V0 exactly,
// which makes every rectangle vertex sit on the spiral at θ = an
// integer multiple of π/2.
const V0: Vec2 = { x: 0, y: 1 };

/** Distance from V0 to POLE — the spiral's base radius (θ = 0). */
export const RHO0: number = Math.hypot(V0.x - POLE.x, V0.y - POLE.y);

/** Polar angle of V0 about POLE — the spiral's base direction. */
export const ALPHA0: number = Math.atan2(V0.y - POLE.y, V0.x - POLE.x);

/** A point on the logarithmic spiral at angular parameter θ.
 *  Increasing θ moves *outward* (radius grows by exp(k·θ)); the
 *  polar angle decreases by θ so the spiral winds in the same
 *  direction as the rectangle nesting (CW for positive θ in
 *  screen coords where y is down). */
export const logSpiralPoint = (theta: number): Vec2 => {
  const rad = RHO0 * Math.exp(K * theta);
  const ang = ALPHA0 - theta;
  return {
    x: POLE.x + rad * Math.cos(ang),
    y: POLE.y + rad * Math.sin(ang),
  };
};

/** Nested φ-rectangles in path order, from the outermost down to
 *  the innermost. `stepsOut` walks the recursion outward from the
 *  unit-φ rectangle to expose a larger frame; `stepsIn` then
 *  walks inward, emitting one rectangle per step. The defaults
 *  (8, 60) match the reference HTML and give enough outer
 *  rectangles to fill any reasonable canvas crop and enough inner
 *  rectangles for the rendering loop to clip below the pole's
 *  sub-pixel limit. */
export const goldenRectangles = (stepsOut = 8, stepsIn = 60): Rectangle[] => {
  let A: Vec2 = { x: 0, y: 0 };
  let e1: Vec2 = { x: PHI, y: 0 };
  let e2: Vec2 = { x: 0, y: 1 };
  // Walk outward to the largest rectangle we'll emit. The outward
  // step inverts the inward recursion: e1 grows by φ (and rotates
  // CW), e2 takes the previous e1, A shifts back by the new e1.
  for (let i = 0; i < stepsOut; i++) {
    const e1p = rotN90(mul(e1, PHI));
    const e2p = e1;
    A = sub(A, e1p);
    e1 = e1p;
    e2 = e2p;
  }
  // Then walk inward, recording each rectangle as we go. The
  // inward step is the same recursion used by `iteratePole`,
  // which guarantees the rectangles converge on POLE.
  const rects: Rectangle[] = [];
  const total = stepsOut + stepsIn;
  for (let n = 0; n < total; n++) {
    rects.push([A, add(A, e1), add(add(A, e1), e2), add(A, e2)]);
    const nA = add(A, e1);
    const nE2 = mul(rot90(e2), 1 / PHI);
    A = nA;
    e1 = e2;
    e2 = nE2;
  }
  return rects;
};

// ─── Chord animation primitives ──────────────────────────────────
//
// The renderer draws a chord between each pair of consecutive
// spiral vertices V_a = logSpiralPoint(θ_a) and V_b =
// logSpiralPoint(θ_a + π/2). The chord animates in lockstep with
// the pencil tip θ:
//
//   progress = (θ_tip − θ_a) / (π/2)         (chord-steps)
//
//   progress  < 0          → hidden
//   0 ≤ p     < 1          → growing (V_a → tip)
//   1 ≤ p     < N          → full    (V_a → V_b, held)
//   N ≤ p     < N + 1      → shrinking
//   N + 1 ≤ p              → hidden
//
// where N = `visibleSteps` (default `CHORD_VISIBLE_STEPS`).
//
// All this lives as pure functions so the timing logic is
// unit-testable independent of canvas / requestAnimationFrame —
// the rendering bug surface is the part we want pinned down.

/** Number of chord-steps a chord stays at full length after the
 *  pencil has finished tracing it. 4 = one full turn worth. */
export const CHORD_VISIBLE_STEPS = 4;

export type ChordPhase = 'hidden' | 'growing' | 'full' | 'shrinking';

export type ChordState = {
  readonly phase: ChordPhase;
  /** `growing`: portion drawn from V_a toward V_b (0..1).
   *  `shrinking` (normal):  fraction *remaining*, anchored on
   *    V_b — draw `[V_a + (1 − fraction)·dir, V_b]`.
   *  `shrinking` (flipped): fraction remaining anchored on
   *    V_a — draw `[V_a, V_a + fraction·dir]`.
   *  `hidden` / `full`: 0 / 1 (not used by the renderer). */
  readonly fraction: number;
};

/** Pure: chord state for the pair (V_a, V_a + π/2) at pencil θ_tip.
 *  `visibleSteps` controls how many chord-steps the chord remains
 *  at full length after the pencil has crossed V_b. */
export const chordStateAt = (
  thetaA: number,
  thetaTip: number,
  visibleSteps: number = CHORD_VISIBLE_STEPS,
): ChordState => {
  const progress = (thetaTip - thetaA) / (Math.PI / 2);
  if (progress < 0) return { phase: 'hidden', fraction: 0 };
  if (progress < 1) return { phase: 'growing', fraction: progress };
  if (progress < visibleSteps) return { phase: 'full', fraction: 1 };
  if (progress < visibleSteps + 1) {
    return { phase: 'shrinking', fraction: visibleSteps + 1 - progress };
  }
  return { phase: 'hidden', fraction: 0 };
};

/** Pure: the on-spiral sub-segment to render given a ChordState.
 *  Returns `null` for hidden chords. `flipShrinkDirection` flips
 *  the shrink anchor (V_a side instead of V_b side) — used for
 *  the central pole-region chords so they shrink toward the
 *  outward direction the pencil is heading. */
export const chordSegment = (
  thetaA: number,
  state: ChordState,
  flipShrinkDirection = false,
): { readonly start: Vec2; readonly end: Vec2 } | null => {
  if (state.phase === 'hidden') return null;
  const va = logSpiralPoint(thetaA);
  const vb = logSpiralPoint(thetaA + Math.PI / 2);
  const lerp = (t: number): Vec2 => ({
    x: va.x + (vb.x - va.x) * t,
    y: va.y + (vb.y - va.y) * t,
  });
  if (state.phase === 'full') return { start: va, end: vb };
  if (state.phase === 'growing') return { start: va, end: lerp(state.fraction) };
  // shrinking
  if (flipShrinkDirection) {
    // Drop V_b side first → remaining slice stays anchored on V_a.
    return { start: va, end: lerp(state.fraction) };
  }
  // Default: drop V_a side first → remaining slice stays on V_b.
  return { start: lerp(1 - state.fraction), end: vb };
};

// Internal helpers retained for the tests so the iteration's
// convergence rate can be asserted without re-deriving it.
export const __internal__ = {
  iteratePole,
  POLE_ITERATIONS,
  TWO_PI,
  V0,
} as const;
