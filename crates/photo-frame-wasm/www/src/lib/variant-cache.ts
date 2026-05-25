// Two-layer variant cache for stale-while-revalidate preview
// updates. The motivating UX problem: when the user changes
// Preset or Resolution, the scope (image, maxLongEdge) tuple
// changes and every cached variant becomes stale (different
// pixel dimensions). Naively wiping the cache blanks the canvas
// for the 50–700 ms of the new prepare's hop through the worker,
// which reads as "did I just break it?".
//
// The cache solves this by keeping the just-wiped layer around
// as `previous`. `get` falls back to it transparently, so the
// canvas keeps showing the old (still-correct shape, just-wrong-
// size) preview until the new prepare lands and `set` writes the
// fresh variant into `current`. The atomic swap means no flash.
//
// The structure is intentionally immutable — every mutator
// returns a new instance — so it can drop into a SolidJS signal
// without an extra "version" channel: `setSignal((c) => c.set(…))`
// just works.

export type VariantCache<K, V> = {
  /** Total entries across both layers (mostly useful for
   *  introspection and tests). */
  readonly size: number;
  /** True if `key` resolves in either layer. Use this when the
   *  question is "can `get(key)` return something?" — e.g. the
   *  draw effect asking whether it has anything to paint. */
  has: (key: K) => boolean;
  /** True if `key` is in the *current* (= fresh) layer only.
   *  Use this when the question is "do I need to (re)compute
   *  this variant?" — e.g. the prefetch loop asking which
   *  variants under the new scope still need preparing.
   *  Entries promoted to `previous` by `rotate` are stale, so
   *  `has` would return true but `hasFresh` correctly says no. */
  hasFresh: (key: K) => boolean;
  /** Lookup: prefers `current`, falls back to `previous` for
   *  the same key. Returns `undefined` only when neither layer
   *  knows the key. */
  get: (key: K) => V | undefined;
  /** Iterate keys present in either layer (current wins on a
   *  tie — same as `get`). Useful for "is everything cached
   *  yet?" predicates in the prefetch loop. */
  keys: () => IterableIterator<K>;
  /** Write `(key, value)` into the `current` layer. Returns a
   *  fresh cache; the receiver is untouched. */
  set: (key: K, value: V) => VariantCache<K, V>;
  /** Promote `current` to `previous` and start a fresh `current`.
   *  Use this on scope changes so the about-to-be-replaced
   *  variants stay visible during the prepare gap. */
  rotate: () => VariantCache<K, V>;
  /** Wipe both layers. Use this on full session reset (e.g.
   *  return to the drop zone, or a brand-new image), where
   *  there's no "previous" we want to keep showing. */
  clear: () => VariantCache<K, V>;
};

class VariantCacheImpl<K, V> implements VariantCache<K, V> {
  constructor(
    private readonly current: ReadonlyMap<K, V>,
    private readonly previous: ReadonlyMap<K, V>,
  ) {}

  get size(): number {
    // Count keys that appear in either layer exactly once —
    // `current.size + previous.size` would double-count keys
    // that exist in both, which is a misleading "size" if a
    // caller is asking "how many entries can I `get`?".
    let n = this.current.size;
    for (const k of this.previous.keys()) {
      if (!this.current.has(k)) n += 1;
    }
    return n;
  }

  has(key: K): boolean {
    return this.current.has(key) || this.previous.has(key);
  }

  hasFresh(key: K): boolean {
    return this.current.has(key);
  }

  get(key: K): V | undefined {
    const v = this.current.get(key);
    if (v !== undefined) return v;
    return this.previous.get(key);
  }

  *keys(): IterableIterator<K> {
    for (const k of this.current.keys()) yield k;
    for (const k of this.previous.keys()) {
      if (!this.current.has(k)) yield k;
    }
  }

  set(key: K, value: V): VariantCache<K, V> {
    const next = new Map(this.current);
    next.set(key, value);
    return new VariantCacheImpl(next, this.previous);
  }

  rotate(): VariantCache<K, V> {
    return new VariantCacheImpl(new Map(), this.current);
  }

  clear(): VariantCache<K, V> {
    return new VariantCacheImpl(new Map(), new Map());
  }
}

export const createVariantCache = <K, V>(): VariantCache<K, V> =>
  new VariantCacheImpl<K, V>(new Map(), new Map());
