import * as fc from 'fast-check';
import { describe, expect, test } from 'vitest';
import { computePaintLayout, type PaintInput } from './paint-preview';

const input = (over: Partial<PaintInput> = {}): PaintInput => ({
  pixels: { width: 1600, height: 1000 },
  cssW: 800,
  cssH: 500,
  rawDpr: 1,
  ...over,
});

describe('computePaintLayout', () => {
  test('returns null when CSS box collapses to zero', () => {
    expect(computePaintLayout(input({ cssW: 0 }))).toBeNull();
    expect(computePaintLayout(input({ cssH: 0 }))).toBeNull();
    expect(computePaintLayout(input({ cssW: -3 }))).toBeNull();
  });

  test('returns null when source pixels collapse to zero', () => {
    expect(computePaintLayout(input({ pixels: { width: 0, height: 100 } }))).toBeNull();
    expect(computePaintLayout(input({ pixels: { width: 100, height: 0 } }))).toBeNull();
  });

  test('contain-fits landscape source into landscape stage (width-bound)', () => {
    // 1600×1000 (1.6) into 800×500 (1.6): exact match.
    const layout = computePaintLayout(input());
    expect(layout).not.toBeNull();
    expect(layout?.dest.dw).toBeCloseTo(800);
    expect(layout?.dest.dh).toBeCloseTo(500);
    expect(layout?.dest.dx).toBeCloseTo(0);
    expect(layout?.dest.dy).toBeCloseTo(0);
  });

  test('contain-fits portrait source into landscape stage (height-bound, letterboxed horizontally)', () => {
    const layout = computePaintLayout(
      input({ pixels: { width: 500, height: 1000 }, cssW: 800, cssH: 500 }),
    );
    expect(layout).not.toBeNull();
    // height-bound: dh=500, dw=500 * (500/1000) = 250
    expect(layout?.dest.dh).toBeCloseTo(500);
    expect(layout?.dest.dw).toBeCloseTo(250);
    // letterboxed equally on both sides: dx=(800-250)/2=275
    expect(layout?.dest.dx).toBeCloseTo(275);
    expect(layout?.dest.dy).toBeCloseTo(0);
  });

  test('contain-fits landscape source into portrait stage (width-bound, letterboxed vertically)', () => {
    const layout = computePaintLayout(
      input({ pixels: { width: 1600, height: 800 }, cssW: 500, cssH: 800 }),
    );
    expect(layout).not.toBeNull();
    // width-bound: dw=500, dh=800 * (500/1600) = 250
    expect(layout?.dest.dw).toBeCloseTo(500);
    expect(layout?.dest.dh).toBeCloseTo(250);
    expect(layout?.dest.dx).toBeCloseTo(0);
    expect(layout?.dest.dy).toBeCloseTo(275);
  });

  test('clamps DPR to [1, 2] inclusive', () => {
    expect(computePaintLayout(input({ rawDpr: 0.3 }))?.dpr).toBe(1);
    expect(computePaintLayout(input({ rawDpr: 1 }))?.dpr).toBe(1);
    expect(computePaintLayout(input({ rawDpr: 1.5 }))?.dpr).toBe(1.5);
    expect(computePaintLayout(input({ rawDpr: 2 }))?.dpr).toBe(2);
    expect(computePaintLayout(input({ rawDpr: 3 }))?.dpr).toBe(2);
    expect(computePaintLayout(input({ rawDpr: Number.NaN }))?.dpr).toBe(1);
  });

  test('drawing-buffer size = cssBox × clamped DPR, rounded', () => {
    const layout = computePaintLayout(input({ cssW: 100, cssH: 50, rawDpr: 1.5 }));
    expect(layout?.canvasW).toBe(150);
    expect(layout?.canvasH).toBe(75);
  });

  test('fast-check: contain-fit preserves source aspect within rounding', () => {
    fc.assert(
      fc.property(
        fc.integer({ min: 1, max: 8000 }),
        fc.integer({ min: 1, max: 8000 }),
        fc.integer({ min: 1, max: 4000 }),
        fc.integer({ min: 1, max: 4000 }),
        fc.double({ min: 0.1, max: 4, noNaN: true }),
        (pw, ph, cssW, cssH, rawDpr) => {
          const layout = computePaintLayout({
            pixels: { width: pw, height: ph },
            cssW,
            cssH,
            rawDpr,
          });
          if (!layout) return true;
          const srcAspect = pw / ph;
          const destAspect = layout.dest.dw / layout.dest.dh;
          // Aspect equality up to floating-point noise.
          return Math.abs(srcAspect - destAspect) / srcAspect < 1e-6;
        },
      ),
    );
  });

  test('fast-check: destination rect stays inside the CSS box', () => {
    fc.assert(
      fc.property(
        fc.integer({ min: 1, max: 8000 }),
        fc.integer({ min: 1, max: 8000 }),
        fc.integer({ min: 1, max: 4000 }),
        fc.integer({ min: 1, max: 4000 }),
        (pw, ph, cssW, cssH) => {
          const layout = computePaintLayout({
            pixels: { width: pw, height: ph },
            cssW,
            cssH,
            rawDpr: 1,
          });
          if (!layout) return true;
          const { dx, dy, dw, dh } = layout.dest;
          const eps = 1e-6;
          return dx >= -eps && dy >= -eps && dx + dw <= cssW + eps && dy + dh <= cssH + eps;
        },
      ),
    );
  });

  test('fast-check: centred letterbox — equal margin on both axes', () => {
    fc.assert(
      fc.property(
        fc.integer({ min: 1, max: 8000 }),
        fc.integer({ min: 1, max: 8000 }),
        fc.integer({ min: 1, max: 4000 }),
        fc.integer({ min: 1, max: 4000 }),
        (pw, ph, cssW, cssH) => {
          const layout = computePaintLayout({
            pixels: { width: pw, height: ph },
            cssW,
            cssH,
            rawDpr: 1,
          });
          if (!layout) return true;
          const { dx, dy, dw, dh } = layout.dest;
          // Left margin = right margin (within fp tolerance).
          const xMargin = cssW - dw;
          const yMargin = cssH - dh;
          return Math.abs(dx - xMargin / 2) < 1e-6 && Math.abs(dy - yMargin / 2) < 1e-6;
        },
      ),
    );
  });
});
