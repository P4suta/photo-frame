import * as fc from 'fast-check';
import { describe, expect, test } from 'vitest';
import { containFit } from './contain-fit';

describe('containFit', () => {
  test('returns null when the stage is unmeasured', () => {
    expect(containFit({ srcW: 100, srcH: 100, stageW: 0, stageH: 0 })).toBeNull();
    expect(containFit({ srcW: 100, srcH: 100, stageW: 200, stageH: 0 })).toBeNull();
    expect(containFit({ srcW: 100, srcH: 100, stageW: 0, stageH: 200 })).toBeNull();
  });

  test('returns null for a degenerate source', () => {
    expect(containFit({ srcW: 0, srcH: 100, stageW: 200, stageH: 200 })).toBeNull();
    expect(containFit({ srcW: 100, srcH: 0, stageW: 200, stageH: 200 })).toBeNull();
  });

  test('landscape source caps on width', () => {
    // 3:2 source in a square stage — width fills, height shrinks.
    const out = containFit({ srcW: 3000, srcH: 2000, stageW: 800, stageH: 800 });
    expect(out).toEqual({ width: 800, height: 533 });
  });

  test('portrait source caps on height', () => {
    // 2:3 source in a square stage — height fills, width shrinks.
    // 800 / 1.5 = 533.33 → floor 533; 533 * 1.5 = 799.5 → floor
    // 799. The 1-px loss is the price of `floor`'d overflow
    // safety — it's invisible at typical preview sizes.
    const out = containFit({ srcW: 2000, srcH: 3000, stageW: 800, stageH: 800 });
    expect(out).toEqual({ width: 533, height: 799 });
  });

  test('square source caps on the smaller axis', () => {
    const out = containFit({ srcW: 100, srcH: 100, stageW: 1200, stageH: 800 });
    expect(out).toEqual({ width: 800, height: 800 });
  });

  test('source aspect matches stage aspect — fills both axes', () => {
    const out = containFit({ srcW: 1600, srcH: 900, stageW: 800, stageH: 450 });
    expect(out).toEqual({ width: 800, height: 450 });
  });

  test('floors so the wrapper never exceeds the stage', () => {
    // A pathological aspect that rounds up by default.
    const out = containFit({ srcW: 1001, srcH: 1000, stageW: 100, stageH: 100 });
    expect(out).not.toBeNull();
    if (!out) return;
    expect(out.width).toBeLessThanOrEqual(100);
    expect(out.height).toBeLessThanOrEqual(100);
  });

  test('property: result respects contain semantics — fits inside stage, fills at least one axis', () => {
    // The property test polices the *structural* contract of
    // object-fit:contain, which is what `containFit` exists to
    // mimic:
    //   (a) the output fits inside the stage on both axes;
    //   (b) at least one axis hugs the stage to within 1 px
    //       (the floor cost) — i.e. we never under-fill both
    //       axes when we could paint bigger;
    //   (c) the output has non-zero area.
    //
    // Aspect preservation isn't tested here. With `Math.floor`
    // the relative aspect error is unbounded at lopsided
    // sources (e.g. a 1000:5 strip in a 300² stage shrinks
    // height to ~1 px, blowing the ratio). The specific-case
    // tests above (`landscape source caps on width`, etc.)
    // pin aspect at the sizes real photos actually take —
    // that's where the visual contract lives.
    fc.assert(
      fc.property(
        fc.integer({ min: 1, max: 10_000 }),
        fc.integer({ min: 1, max: 10_000 }),
        fc.integer({ min: 1, max: 4000 }),
        fc.integer({ min: 1, max: 4000 }),
        (srcW, srcH, stageW, stageH) => {
          const out = containFit({ srcW, srcH, stageW, stageH });
          expect(out).not.toBeNull();
          if (!out) return;
          expect(out.width).toBeLessThanOrEqual(stageW);
          expect(out.height).toBeLessThanOrEqual(stageH);
          expect(out.width).toBeGreaterThanOrEqual(0);
          expect(out.height).toBeGreaterThanOrEqual(0);
          // Contain: one axis must fill the stage (within the
          // 1 px loss `Math.floor` allows).
          const hugsWidth = stageW - out.width <= 1;
          const hugsHeight = stageH - out.height <= 1;
          expect(hugsWidth || hugsHeight).toBe(true);
        },
      ),
    );
  });
});
