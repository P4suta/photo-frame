import { describe, expect, test } from 'vitest';
import { ALL_VARIANTS, parseVariantKey, variantKey } from './variants';

describe('ALL_VARIANTS', () => {
  test('enumerates exactly the 2 × 2 × 2 = 8 combinations', () => {
    expect(ALL_VARIANTS).toHaveLength(8);
  });

  test('every variant produces a unique key', () => {
    const keys = new Set(ALL_VARIANTS.map((v) => variantKey(v.theme, v.layout, v.showMeta)));
    expect(keys.size).toBe(ALL_VARIANTS.length);
  });
});

describe('variantKey / parseVariantKey roundtrip', () => {
  test('every ALL_VARIANTS entry round-trips through parse', () => {
    for (const v of ALL_VARIANTS) {
      const round = parseVariantKey(variantKey(v.theme, v.layout, v.showMeta));
      expect(round).toEqual(v);
    }
  });

  test('returns null for malformed input', () => {
    expect(parseVariantKey('')).toBeNull();
    expect(parseVariantKey('paper|edges')).toBeNull();
    expect(parseVariantKey('gold|edges|true')).toBeNull();
    expect(parseVariantKey('paper|diagonal|true')).toBeNull();
    expect(parseVariantKey('paper|edges|maybe')).toBeNull();
  });
});
