import * as fc from 'fast-check';
import { describe, expect, test } from 'vitest';
import {
  LONG_EDGE_OPTIONS,
  type LongEdgeKey,
  longEdgeKeyFor,
  pickAutoDemoteKey,
  presetDisplayName,
  sourceLongEdgeOf,
} from './long-edge';

describe('LONG_EDGE_OPTIONS table', () => {
  test('always contains a "full" key with null cap', () => {
    expect(LONG_EDGE_OPTIONS.full.maxLongEdge).toBeNull();
  });

  test('numeric caps follow the canonical screen-size ladder', () => {
    expect(LONG_EDGE_OPTIONS['4k'].maxLongEdge).toBe(3840);
    expect(LONG_EDGE_OPTIONS.fhd.maxLongEdge).toBe(1920);
    expect(LONG_EDGE_OPTIONS.hd.maxLongEdge).toBe(1280);
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

  test('snaps a non-listed cap to the closest UI cap ≤ target', () => {
    // The Rust-side `PipelineSpec::SNS` carries `max_long_edge =
    // 2048`; the JS picker offers 4K / FHD / HD / Full. With
    // "closest cap ≤ target" semantics, 2048 selects FHD (1920) —
    // still within the preset's promise, and the picker reflects
    // a meaningful state instead of silently falling back to Full.
    expect(longEdgeKeyFor(2048)).toBe<LongEdgeKey>('fhd');
    expect(longEdgeKeyFor(2560)).toBe<LongEdgeKey>('fhd');
    expect(longEdgeKeyFor(3839)).toBe<LongEdgeKey>('fhd');
    expect(longEdgeKeyFor(3841)).toBe<LongEdgeKey>('4k');
  });

  test('target smaller than every numeric option falls back to "full"', () => {
    expect(longEdgeKeyFor(800)).toBe<LongEdgeKey>('full');
    expect(longEdgeKeyFor(1279)).toBe<LongEdgeKey>('full');
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
    expect(pickAutoDemoteKey(2000, '4k')).toBe<LongEdgeKey>('fhd');
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

describe('presetDisplayName', () => {
  test('maps canonical Rust labels to UI-styled names', () => {
    expect(presetDisplayName('sns')).toBe('SNS');
    expect(presetDisplayName('standard')).toBe('Standard');
    expect(presetDisplayName('maximum')).toBe('Maximum');
  });

  test('falls back to the raw label for unknown values', () => {
    expect(presetDisplayName('archival')).toBe('archival');
  });
});
