import { describe, expect, test } from 'vitest';
import {
  LONG_EDGE_OPTIONS,
  type LongEdgeKey,
  longEdgeKeyFor,
  PRESETS,
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
