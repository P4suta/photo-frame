import { describe, expect, test } from 'vitest';
import {
  __internal__,
  ALPHA0,
  CHORD_VISIBLE_STEPS,
  chordSegment,
  chordStateAt,
  goldenRectangles,
  K,
  logSpiralPoint,
  PHI,
  POLE,
  RHO0,
} from './golden-spiral';

describe('logarithmic golden spiral geometry', () => {
  test('K places φ per quarter turn', () => {
    // The whole rendering loop relies on `exp(K · 2π) === φ⁴`.
    // If K drifts (e.g. someone "simplifies" the formula), the
    // 2π-wrap self-similarity collapses and the seam becomes
    // visible. Assert the relation directly so the unit holds
    // even if the constant's literal form is refactored.
    expect(Math.exp(K * (Math.PI / 2))).toBeCloseTo(PHI, 12);
    expect(Math.exp(K * 2 * Math.PI)).toBeCloseTo(PHI ** 4, 9);
  });

  test('POLE converges below 1e-12 after the configured iterations', () => {
    const { iteratePole, POLE_ITERATIONS } = __internal__;
    const a = iteratePole(POLE_ITERATIONS);
    const b = iteratePole(POLE_ITERATIONS + 1);
    expect(Math.hypot(a.x - b.x, a.y - b.y)).toBeLessThan(1e-12);
    // And the publicly-exported POLE matches that iteration.
    expect(POLE.x).toBeCloseTo(a.x, 12);
    expect(POLE.y).toBeCloseTo(a.y, 12);
  });

  test('logSpiralPoint(0) hits the anchor vertex V0 = (0, 1)', () => {
    const p = logSpiralPoint(0);
    expect(p.x).toBeCloseTo(0, 12);
    expect(p.y).toBeCloseTo(1, 12);
    // Sanity: the polar fit is consistent — RHO0 and ALPHA0
    // describe V0 about POLE.
    expect(RHO0).toBeCloseTo(Math.hypot(POLE.x, 1 - POLE.y), 12);
    expect(ALPHA0).toBeCloseTo(Math.atan2(1 - POLE.y, -POLE.x), 12);
  });

  test('one 2π turn multiplies the radius by exactly φ⁴', () => {
    // Self-similarity is the property the rendering loop wraps
    // θ by — at the wrap boundary the canvas scale divides by
    // φ⁴ to land on a visually identical frame. Check the
    // invariant at several θ to rule out a regression that
    // only shows up for specific phases.
    for (const theta of [-3, -1, -0.5, 0, 0.7, 2, 5]) {
      const p0 = logSpiralPoint(theta);
      const p1 = logSpiralPoint(theta + 2 * Math.PI);
      const r0 = Math.hypot(p0.x - POLE.x, p0.y - POLE.y);
      const r1 = Math.hypot(p1.x - POLE.x, p1.y - POLE.y);
      expect(r1 / r0).toBeCloseTo(PHI ** 4, 9);
    }
  });

  test('goldenRectangles(8, 60) emits 68 quads with 1/φ side ratio', () => {
    const rects = goldenRectangles(8, 60);
    expect(rects).toHaveLength(68);
    // Check the 1/φ shrinkage invariant only on the first 30
    // steps. Across 68 nested rectangles the shrinkage
    // accumulates floating-point error of order 10⁻⁷ in the
    // tail (the innermost squares are sub-pixel on screen so
    // this is invisible in practice). The first 30 steps stay
    // within 9-digit precision and that is more than enough to
    // catch any structural regression — wrong rotation, wrong
    // shrink factor, swapped basis vectors.
    for (let i = 1; i < 30; i++) {
      const prev = rects[i - 1];
      const curr = rects[i];
      if (!prev || !curr) continue;
      const sidePrev = Math.hypot(prev[1].x - prev[0].x, prev[1].y - prev[0].y);
      const sideCurr = Math.hypot(curr[1].x - curr[0].x, curr[1].y - curr[0].y);
      expect(sideCurr / sidePrev).toBeCloseTo(1 / PHI, 9);
    }
  });
});

// θ_a fixed at 0 for simplicity — the chord runs from V₀ = (0, 1)
// to V_{π/2}, and the pencil θ_tip relative to it produces the
// chord-step progress that drives every phase transition.
const HALF_PI = Math.PI / 2;

describe('chord animation state machine', () => {
  test('hidden before the pencil reaches V_a (progress < 0)', () => {
    const s = chordStateAt(0, -0.1);
    expect(s.phase).toBe('hidden');
    expect(s.fraction).toBe(0);
  });

  test('progress = 0 enters growing at fraction 0', () => {
    const s = chordStateAt(0, 0);
    expect(s.phase).toBe('growing');
    expect(s.fraction).toBe(0);
  });

  test('growing fraction is linear in progress', () => {
    for (const p of [0.1, 0.25, 0.5, 0.75, 0.9999]) {
      const s = chordStateAt(0, p * HALF_PI);
      expect(s.phase).toBe('growing');
      expect(s.fraction).toBeCloseTo(p, 12);
    }
  });

  test('progress = 1 (pencil at V_b) snaps to full', () => {
    const s = chordStateAt(0, HALF_PI);
    expect(s.phase).toBe('full');
    expect(s.fraction).toBe(1);
  });

  test('chord stays full while 1 ≤ progress < visibleSteps', () => {
    for (const p of [1, 1.5, 2, 3, CHORD_VISIBLE_STEPS - 0.0001]) {
      const s = chordStateAt(0, p * HALF_PI);
      expect(s.phase).toBe('full');
      expect(s.fraction).toBe(1);
    }
  });

  test('progress = visibleSteps starts shrinking at fraction 1', () => {
    const s = chordStateAt(0, CHORD_VISIBLE_STEPS * HALF_PI);
    expect(s.phase).toBe('shrinking');
    expect(s.fraction).toBe(1);
  });

  test('shrinking fraction is linear in progress', () => {
    // Walking through the shrinking window in 0.25 steps the
    // fraction should fall (1, 0.75, 0.5, 0.25) — a pure
    // visualisation of the "how much remains" reading.
    for (const [d, expected] of [
      [0, 1],
      [0.25, 0.75],
      [0.5, 0.5],
      [0.75, 0.25],
    ] as const) {
      const s = chordStateAt(0, (CHORD_VISIBLE_STEPS + d) * HALF_PI);
      expect(s.phase).toBe('shrinking');
      expect(s.fraction).toBeCloseTo(expected, 12);
    }
  });

  test('progress = visibleSteps + 1 hides the chord', () => {
    const s = chordStateAt(0, (CHORD_VISIBLE_STEPS + 1) * HALF_PI);
    expect(s.phase).toBe('hidden');
    expect(s.fraction).toBe(0);
  });

  test('shorter `visibleSteps` collapses the full plateau', () => {
    // With visibleSteps = 1 the chord goes growing → full at
    // p=1 → shrinking starts immediately at p=1 (no plateau).
    // We probe right past p=1 to catch the boundary.
    const s = chordStateAt(0, 1.0001 * HALF_PI, 1);
    expect(s.phase).toBe('shrinking');
    // At p = 1.0001, fraction = (1 + 1) - 1.0001 = 0.9999
    expect(s.fraction).toBeCloseTo(0.9999, 4);
  });
});

describe('chord segment rendering geometry', () => {
  // V_a / V_b for the canonical θ_a = 0 chord — used to assert
  // exact endpoint coincidence (vs. logSpiralPoint reproduction
  // inside chordSegment).
  const va = logSpiralPoint(0);
  const vb = logSpiralPoint(HALF_PI);
  const expectClose = (a: { x: number; y: number }, b: { x: number; y: number }): void => {
    expect(a.x).toBeCloseTo(b.x, 12);
    expect(a.y).toBeCloseTo(b.y, 12);
  };

  test('hidden chord yields no segment', () => {
    expect(chordSegment(0, { phase: 'hidden', fraction: 0 })).toBeNull();
  });

  test('full chord spans V_a → V_b', () => {
    const seg = chordSegment(0, { phase: 'full', fraction: 1 });
    expect(seg).not.toBeNull();
    if (seg) {
      expectClose(seg.start, va);
      expectClose(seg.end, vb);
    }
  });

  test('growing chord starts at V_a and ends at the interpolated point', () => {
    const seg = chordSegment(0, { phase: 'growing', fraction: 0.4 });
    expect(seg).not.toBeNull();
    if (seg) {
      expectClose(seg.start, va);
      expectClose(seg.end, {
        x: va.x + 0.4 * (vb.x - va.x),
        y: va.y + 0.4 * (vb.y - va.y),
      });
    }
  });

  test('shrinking (default) drops the V_a side and keeps the V_b end', () => {
    // fraction = 0.4 → remaining slice is [V_a + 0.6·dir, V_b]
    const seg = chordSegment(0, { phase: 'shrinking', fraction: 0.4 });
    expect(seg).not.toBeNull();
    if (seg) {
      expectClose(seg.start, {
        x: va.x + 0.6 * (vb.x - va.x),
        y: va.y + 0.6 * (vb.y - va.y),
      });
      expectClose(seg.end, vb);
    }
  });

  test('shrinking with flip drops the V_b side and keeps the V_a end', () => {
    // Same fraction but the flip flag — the *outward* (V_b) end
    // is the one that goes away first, so the surviving slice
    // is [V_a, V_a + 0.4·dir].
    const seg = chordSegment(0, { phase: 'shrinking', fraction: 0.4 }, true);
    expect(seg).not.toBeNull();
    if (seg) {
      expectClose(seg.start, va);
      expectClose(seg.end, {
        x: va.x + 0.4 * (vb.x - va.x),
        y: va.y + 0.4 * (vb.y - va.y),
      });
    }
  });
});
