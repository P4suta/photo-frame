import { describe, expect, test } from 'vitest';
import { createVariantCache } from './variant-cache';

describe('createVariantCache (empty)', () => {
  test('reports size 0 and misses every lookup', () => {
    const cache = createVariantCache<string, number>();
    expect(cache.size).toBe(0);
    expect(cache.has('any')).toBe(false);
    expect(cache.get('any')).toBeUndefined();
    expect(Array.from(cache.keys())).toEqual([]);
  });
});

describe('has vs hasFresh — which layer is being asked about', () => {
  test('after rotate, has(k) stays true but hasFresh(k) goes false', () => {
    // The contract the prefetch loop relies on: a key promoted
    // into `previous` is still resolvable via `get` (so the
    // canvas stays painted) but needs re-preparing — so
    // `hasFresh` correctly says "no, you still need to fetch
    // this one" while `has` says "yes, get(k) returns something".
    const before = createVariantCache<string, number>().set('a', 1);
    expect(before.has('a')).toBe(true);
    expect(before.hasFresh('a')).toBe(true);
    const after = before.rotate();
    expect(after.has('a')).toBe(true);
    expect(after.hasFresh('a')).toBe(false);
  });

  test('a set after rotate makes hasFresh true again', () => {
    const cache = createVariantCache<string, number>().set('a', 1).rotate().set('a', 2);
    expect(cache.has('a')).toBe(true);
    expect(cache.hasFresh('a')).toBe(true);
  });
});

describe('set / get on the current layer', () => {
  test('a single set is readable via get and has', () => {
    const cache = createVariantCache<string, number>().set('a', 1);
    expect(cache.size).toBe(1);
    expect(cache.has('a')).toBe(true);
    expect(cache.get('a')).toBe(1);
    expect(cache.get('missing')).toBeUndefined();
  });

  test('subsequent sets stack up and the last write wins per key', () => {
    const cache = createVariantCache<string, number>().set('a', 1).set('b', 2).set('a', 99);
    expect(cache.size).toBe(2);
    expect(cache.get('a')).toBe(99);
    expect(cache.get('b')).toBe(2);
  });

  test('set returns a fresh cache; the original is unchanged', () => {
    // Immutability is what makes this safe to drop into a
    // SolidJS signal — the signal compares by reference and
    // only fires reactives when the cache identity flips.
    const a = createVariantCache<string, number>().set('a', 1);
    const b = a.set('b', 2);
    expect(a.size).toBe(1);
    expect(a.has('b')).toBe(false);
    expect(b.size).toBe(2);
    expect(b.get('a')).toBe(1);
    expect(b.get('b')).toBe(2);
  });
});

describe('rotate', () => {
  test('moves current entries into previous, leaving current empty', () => {
    const before = createVariantCache<string, number>().set('a', 1).set('b', 2);
    const after = before.rotate();
    // From the outside, every key is still resolvable — the
    // whole point of the previous layer.
    expect(after.has('a')).toBe(true);
    expect(after.has('b')).toBe(true);
    expect(after.get('a')).toBe(1);
    expect(after.get('b')).toBe(2);
    expect(after.size).toBe(2);
  });

  test('after rotate, set writes into a fresh current layer', () => {
    const cache = createVariantCache<string, number>().set('a', 1).rotate().set('a', 99);
    // A new value for the same key after rotate — `current`
    // takes precedence over `previous`, so the new value wins.
    expect(cache.get('a')).toBe(99);
    expect(cache.size).toBe(1);
  });

  test('rotate twice without a write in between empties everything', () => {
    // First rotate moves current → previous; second rotate
    // moves (now-empty) current → previous and the originally-
    // promoted previous evaporates. This is the safety property
    // that prevents an unbounded backlog of stale variants.
    const cache = createVariantCache<string, number>().set('a', 1).rotate().rotate();
    expect(cache.size).toBe(0);
    expect(cache.has('a')).toBe(false);
  });

  test('current writes shadow previous values for the same key', () => {
    // The main UX guarantee: while the new prepare is in
    // flight, get(key) returns the *previous* (= stale-but-
    // correct-shape) value. The moment set(key, new) lands,
    // get(key) flips to the new value atomically.
    const cache = createVariantCache<string, number>().set('a', 1).rotate();
    expect(cache.get('a')).toBe(1); // previous layer
    const next = cache.set('a', 2);
    expect(next.get('a')).toBe(2); // current shadows previous
  });
});

describe('clear', () => {
  test('clears both layers', () => {
    const cache = createVariantCache<string, number>().set('a', 1).rotate().set('b', 2).clear();
    expect(cache.size).toBe(0);
    expect(cache.has('a')).toBe(false);
    expect(cache.has('b')).toBe(false);
  });
});

describe('keys iteration', () => {
  test('yields each key exactly once across both layers (current wins)', () => {
    const cache = createVariantCache<string, number>()
      .set('a', 1)
      .set('b', 2)
      .rotate()
      .set('a', 99)
      .set('c', 3);
    // current: a, c (a shadows previous's a)
    // previous: a, b
    // Unique keys: a, c, b — order is current-first, then
    // previous-without-duplicates.
    expect(Array.from(cache.keys())).toEqual(['a', 'c', 'b']);
  });
});

describe('typical scope-change sequence', () => {
  test('rotate then incrementally fill current — get falls back during the gap', () => {
    // Mirror the App.tsx flow: scope change rotates; the
    // prepare effect then fills variants one by one. At each
    // intermediate step, unfilled variants still resolve to
    // their (stale, previous-scope) value — no blank flash.
    let cache = createVariantCache<string, string>()
      .set('paper-edges-true', 'A')
      .set('paper-edges-false', 'A')
      .set('ink-edges-true', 'A');
    cache = cache.rotate();
    expect(cache.get('paper-edges-true')).toBe('A'); // previous
    cache = cache.set('paper-edges-true', 'B'); // new prepare lands
    expect(cache.get('paper-edges-true')).toBe('B'); // atomic swap
    expect(cache.get('paper-edges-false')).toBe('A'); // still stale
    expect(cache.get('ink-edges-true')).toBe('A'); // still stale
    cache = cache.set('paper-edges-false', 'B');
    expect(cache.get('paper-edges-false')).toBe('B');
  });
});
