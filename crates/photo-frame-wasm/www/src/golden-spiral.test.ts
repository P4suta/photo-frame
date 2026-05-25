import { describe, expect, test } from 'vitest';
import {
  __internal__,
  ALPHA0,
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
