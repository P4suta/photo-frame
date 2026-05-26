import { describe, expect, test } from 'vitest';
import { ALL_VARIANTS, parseVariantKey, variantKey } from './variants';

describe('ALL_VARIANTS', () => {
  test('enumerates the full 2 × 2 × 2 × 2 = 16 combinations', () => {
    expect(ALL_VARIANTS).toHaveLength(16);
  });

  test('every variant produces a unique key', () => {
    const keys = new Set(
      ALL_VARIANTS.map((v) => variantKey(v.frameStyle, v.theme, v.layout, v.metaPolicy)),
    );
    expect(keys.size).toBe(ALL_VARIANTS.length);
  });
});

describe('variantKey / parseVariantKey roundtrip', () => {
  test('every ALL_VARIANTS entry round-trips through parse', () => {
    for (const v of ALL_VARIANTS) {
      const round = parseVariantKey(variantKey(v.frameStyle, v.theme, v.layout, v.metaPolicy));
      expect(round).toEqual(v);
    }
  });

  test('Polaroid + Edges and Polaroid + Centered produce distinct keys', () => {
    // Layout is independent of FrameStyle — both arrangements are
    // valid inside the Polaroid bottom band, so their cache entries
    // must stay separate.
    expect(variantKey('polaroid', 'paper', 'edges', 'auto')).not.toBe(
      variantKey('polaroid', 'paper', 'centered', 'auto'),
    );
  });

  test('returns null for malformed input', () => {
    expect(parseVariantKey('')).toBeNull();
    expect(parseVariantKey('standard|paper|edges')).toBeNull();
    expect(parseVariantKey('square|paper|edges|auto')).toBeNull();
    expect(parseVariantKey('standard|gold|edges|auto')).toBeNull();
    expect(parseVariantKey('standard|paper|diagonal|auto')).toBeNull();
    expect(parseVariantKey('standard|paper|edges|maybe')).toBeNull();
  });
});
