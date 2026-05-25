import * as fc from 'fast-check';
import { describe, expect, test } from 'vitest';
import {
  LONG_EDGE_OPTIONS,
  type LongEdgeKey,
  longEdgeKeyFor,
  PRESETS,
  pickAutoDemoteKey,
  sourceLongEdgeOf,
} from './long-edge';

describe('LONG_EDGE_OPTIONS table', () => {
  test('always contains a "full" key with null cap', () => {
    expect(LONG_EDGE_OPTIONS.full.maxLongEdge).toBeNull();
  });

  test('numeric caps are strictly descending after full', () => {
    // The order in the segmented should read Full → 4K → FHD →
    // HD so the auto-demote logic always finds a smaller cap
    // by iterating from the start.
    expect(LONG_EDGE_OPTIONS['4k'].maxLongEdge).toBe(3840);
    expect(LONG_EDGE_OPTIONS.fhd.maxLongEdge).toBe(1920);
    expect(LONG_EDGE_OPTIONS.hd.maxLongEdge).toBe(1280);
  });
});

describe('PRESETS table', () => {
  test('SNS preset caps at FHD so the segmented stays in sync', () => {
    expect(PRESETS.sns.maxLongEdge).toBe(LONG_EDGE_OPTIONS.fhd.maxLongEdge);
  });

  test('Standard / Maximum keep the source full', () => {
    expect(PRESETS.standard.maxLongEdge).toBeNull();
    expect(PRESETS.maximum.maxLongEdge).toBeNull();
  });
});

describe('longEdgeKeyFor', () => {
  test('null cap → "full"', () => {
    expect(longEdgeKeyFor(null)).toBe<LongEdgeKey>('full');
  });

  test('each numeric option round-trips to its key', () => {
    expect(longEdgeKeyFor(3840)).toBe<LongEdgeKey>('4k');
    expect(longEdgeKeyFor(1920)).toBe<LongEdgeKey>('fhd');
    expect(longEdgeKeyFor(1280)).toBe<LongEdgeKey>('hd');
  });

  test('unmatched numeric cap falls back to "full"', () => {
    // 2048 was the SNS preset value before it was aligned to
    // FHD; this guards the fallback so a future numeric drift
    // doesn't silently misclassify the preset.
    expect(longEdgeKeyFor(2048)).toBe<LongEdgeKey>('full');
    expect(longEdgeKeyFor(800)).toBe<LongEdgeKey>('full');
  });
});

describe('sourceLongEdgeOf', () => {
  test('returns the single image long edge in single mode', () => {
    expect(sourceLongEdgeOf({ longEdge: 4000 }, null)).toBe(4000);
  });

  test('returns the minimum across a batch', () => {
    expect(
      sourceLongEdgeOf(null, [{ longEdge: 4000 }, { longEdge: 1200 }, { longEdge: 2400 }]),
    ).toBe(1200);
  });

  test('returns null when nothing is loaded', () => {
    expect(sourceLongEdgeOf(null, null)).toBeNull();
  });

  test('returns null for an empty batch (not -Infinity)', () => {
    // `Math.min(...[])` is `Infinity`; the implementation must
    // guard so callers get null instead of a confusing infinity
    // that would auto-enable every option.
    expect(sourceLongEdgeOf(null, [])).toBeNull();
  });

  test('prefers single over batch when both are set', () => {
    // The mode signal makes this impossible at runtime, but the
    // contract is "single wins" — pin it so a future refactor
    // doesn't silently flip it.
    expect(sourceLongEdgeOf({ longEdge: 5000 }, [{ longEdge: 100 }])).toBe(5000);
  });
});

describe('pickAutoDemoteKey', () => {
  test('null source keeps the current selection', () => {
    expect(pickAutoDemoteKey(null, '4k')).toBe<LongEdgeKey>('4k');
    expect(pickAutoDemoteKey(null, 'full')).toBe<LongEdgeKey>('full');
  });

  test('current cap ≤ source: keep current', () => {
    expect(pickAutoDemoteKey(2000, 'hd')).toBe<LongEdgeKey>('hd');
    expect(pickAutoDemoteKey(1920, 'fhd')).toBe<LongEdgeKey>('fhd');
  });

  test("'full' is always kept (its cap is null = source-size)", () => {
    expect(pickAutoDemoteKey(100, 'full')).toBe<LongEdgeKey>('full');
  });

  test('cap > source snaps to the largest valid cap', () => {
    // 4K (3840) selected, source is only 2000 → demote to FHD (1920).
    expect(pickAutoDemoteKey(2000, '4k')).toBe<LongEdgeKey>('fhd');
    // 4K selected, source 1500 → FHD too big → demote to HD (1280).
    expect(pickAutoDemoteKey(1500, '4k')).toBe<LongEdgeKey>('hd');
  });

  test("source smaller than HD falls back to 'full'", () => {
    expect(pickAutoDemoteKey(800, '4k')).toBe<LongEdgeKey>('full');
    expect(pickAutoDemoteKey(800, 'fhd')).toBe<LongEdgeKey>('full');
    expect(pickAutoDemoteKey(800, 'hd')).toBe<LongEdgeKey>('full');
  });

  test('property: result cap is always null or ≤ source', () => {
    fc.assert(
      fc.property(
        fc.integer({ min: 1, max: 8000 }),
        fc.constantFrom('full', '4k', 'fhd', 'hd') as fc.Arbitrary<LongEdgeKey>,
        (src, current) => {
          const out = pickAutoDemoteKey(src, current);
          const outCap = LONG_EDGE_OPTIONS[out].maxLongEdge;
          if (outCap !== null) expect(outCap).toBeLessThanOrEqual(src);
        },
      ),
    );
  });

  test('idempotent: applying twice yields the same result', () => {
    fc.assert(
      fc.property(
        fc.integer({ min: 1, max: 8000 }),
        fc.constantFrom('full', '4k', 'fhd', 'hd') as fc.Arbitrary<LongEdgeKey>,
        (src, current) => {
          const once = pickAutoDemoteKey(src, current);
          const twice = pickAutoDemoteKey(src, once);
          expect(twice).toBe(once);
        },
      ),
    );
  });
});
