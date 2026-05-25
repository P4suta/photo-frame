/**
 * Solid-reactive primitive owning the single-image preview pipeline
 * end-to-end:
 *
 *   - the two-layer variant cache (current + previous, `rotate()`d on
 *     scope changes so the canvas keeps showing the stale-but-correctly-
 *     shaped preview while the new prepare flight is in the air);
 *   - the prepare-then-prefetch flight (initial variant first, the
 *     remaining seven background-prefetched, generation-gated so a
 *     scope change cancels in-flight writes);
 *   - the variant-toggle out-of-order prepare (the user flips
 *     theme/layout/showMeta to a tuple the prefetch loop hasn't reached
 *     yet, we jump it to the front of the queue);
 *   - the stage `ResizeObserver` → `frameSize` chain that drives the
 *     `<div class={previewFrame}>` wrapper's inline size;
 *   - the full-resolution download flow (re-prepare without the preview
 *     cap, encode, save) and the `busy` flag the sidebar button reads.
 *
 * Consumers (App, CanvasPreview) read through `state.X` accessors and
 * mutate through `actions.X`. The raw variant-cache signal and the
 * three generation gates never leak — every consumer-facing path goes
 * through this primitive.
 */

import { type Accessor, createEffect, createMemo, createSignal, on, onCleanup } from 'solid-js';
import type { DroppedFile } from '../DropZone';
import {
  type CaptionLayout,
  encodeJpeg,
  type FrameOptionsForPrepare,
  type FrameTheme,
  type PreparedPixels,
  preparePixels,
} from '../frame-client';
import { createGenerationGate } from '../lib/batch-sequencer';
import { containFit } from '../lib/contain-fit';
import { framedName, stringifyError, uint8ToBuffer } from '../lib/format';
import { createVariantCache, type VariantCache } from '../lib/variant-cache';
import { ALL_VARIANTS, type VariantKey, variantKey } from '../lib/variants';
import type { AppSettings } from './app-settings';

export type PreviewSession = {
  state: {
    /** Current variant's cached PreparedPixels, or null until the
     *  first prepare lands. During scope-change gaps this returns
     *  the stale (previous-layer) variant so the canvas doesn't
     *  blank — see `lib/variant-cache.ts` for the rotate
     *  semantics. */
    pixels: Accessor<PreparedPixels | null>;
    /** Inline `width` / `height` for the `<div class={previewFrame}>`
     *  wrapper, or null while the stage size or preview pixels are
     *  not yet measurable. */
    frameSize: Accessor<{ width: string; height: string } | null>;
    /** True while a full-resolution download is in flight (drives
     *  the sidebar button label). */
    busy: Accessor<boolean>;
  };
  actions: {
    /** Ref setter for the stage container `<div>`. Installs a
     *  `ResizeObserver` that drives `frameSize`. */
    setStageEl: (el: HTMLDivElement | undefined) => void;
    /** Full-resolution re-prepare + encode + trigger save. */
    onDownload: () => Promise<void>;
  };
  /** Reset the session — wipes both cache layers and invalidates any
   *  in-flight prepares/prefetches. Called from `clearSession`
   *  (start-over) and from the app's `onCleanup`. */
  dispose: () => void;
};

export const createPreviewSession = (deps: {
  source: Accessor<DroppedFile | null>;
  settings: AppSettings['state'];
  /** One-way write seam for the shell-owned status line. */
  setStatus: (s: string) => void;
}): PreviewSession => {
  // ── variant cache ────────────────────────────────────────────────
  //
  // Phase H — two-layer cache backing the preview. Entries are
  // populated by the prepare-then-prefetch effect; on a scope
  // change (image swap or Resolution / Preset bump) the cache
  // `rotate()`s instead of wiping, so the canvas keeps showing
  // the stale-but-correctly-shaped variant until the new prepare
  // lands. See `lib/variant-cache.ts` for the full rationale.
  const [variants, setVariants] = createSignal<VariantCache<VariantKey, PreparedPixels>>(
    createVariantCache<VariantKey, PreparedPixels>(),
  );

  const currentVariantKey = createMemo(() =>
    variantKey(deps.settings.theme(), deps.settings.layout(), deps.settings.showMeta()),
  );
  const pixels = createMemo<PreparedPixels | null>(
    () => variants().get(currentVariantKey()) ?? null,
  );

  // ── prepare + prefetch flight ────────────────────────────────────
  //
  // `prepareGate` invalidates in-flight prefetches when the user
  // drops a new image or moves the max-long-edge slider — only
  // the *currently relevant* scope ever writes back into the map.
  // The Worker serialises requests via `exchange()`'s request-ID
  // filter, and stale completions are dropped by the gate's
  // `isCurrent` check before they touch the signal.
  const prepareGate = createGenerationGate();

  const runPreparePromise = async (
    current: DroppedFile,
    opts: FrameOptionsForPrepare,
    key: VariantKey,
    gen: number,
  ): Promise<void> => {
    deps.setStatus('Framing preview…');
    try {
      const result = await preparePixels(current.data, opts);
      if (!prepareGate.isCurrent(gen) || deps.source() !== current) return;
      setVariants((c) => c.set(key, result));
      deps.setStatus('');
    } catch (error) {
      if (prepareGate.isCurrent(gen)) deps.setStatus(`Error: ${stringifyError(error)}`);
    }
  };

  const runPrepareAndPrefetch = async (
    current: DroppedFile,
    initial: { theme: FrameTheme; layout: CaptionLayout; showMeta: boolean },
    maxLongEdge: number | null,
    gen: number,
  ): Promise<void> => {
    const initialKey = variantKey(initial.theme, initial.layout, initial.showMeta);
    const initialOpts: FrameOptionsForPrepare = {
      theme: initial.theme,
      layout: initial.layout,
      show_meta: initial.showMeta,
      max_long_edge: maxLongEdge,
    };
    await runPreparePromise(current, initialOpts, initialKey, gen);
    if (!prepareGate.isCurrent(gen)) return;
    // Sequential background prefetch — Worker serialises anyway, and
    // sequencing means a user-initiated `runPreparePromise` from the
    // toggle effect below only has at most one prefetch request in
    // flight ahead of it to wait on (~50–200 ms at preview res).
    for (const v of ALL_VARIANTS) {
      if (!prepareGate.isCurrent(gen)) return;
      const key = variantKey(v.theme, v.layout, v.showMeta);
      if (variants().hasFresh(key)) continue;
      const opts: FrameOptionsForPrepare = {
        theme: v.theme,
        layout: v.layout,
        show_meta: v.showMeta,
        max_long_edge: maxLongEdge,
      };
      try {
        const result = await preparePixels(current.data, opts);
        if (!prepareGate.isCurrent(gen)) return;
        setVariants((c) => c.set(key, result));
      } catch {
        // Prefetch best-effort — silently skip failed variants so a
        // glitch on one combo doesn't poison the UI status line.
      }
    }
  };

  // ── scope-change effect ──────────────────────────────────────────
  //
  // Phase G2 — every (image, max_long_edge) scope owns its own cache
  // population. When the scope changes we rotate (not wipe) so the
  // stale variant holds the canvas, then prepare the currently-
  // displayed variant first and prefetch the rest.
  createEffect(
    on(
      () => {
        const current = deps.source();
        if (!current) return null;
        return { current, maxLongEdge: deps.settings.effectiveMaxLongEdge() };
      },
      (scope) => {
        if (!scope) return;
        const gen = prepareGate.bump();
        setVariants((c) => c.rotate());
        const initial = {
          theme: deps.settings.theme(),
          layout: deps.settings.layout(),
          showMeta: deps.settings.showMeta(),
        };
        void runPrepareAndPrefetch(scope.current, initial, scope.maxLongEdge, gen);
      },
    ),
  );

  // ── variant-toggle effect ────────────────────────────────────────
  //
  // If the user toggles theme/layout/show_meta to a variant the
  // prefetch loop hasn't reached yet, kick off an out-of-order
  // prepare. `defer: true` so this doesn't fire on initial setup
  // (the scope effect above already covers that path).
  createEffect(
    on(
      currentVariantKey,
      (key) => {
        const current = deps.source();
        if (!current) return;
        if (variants().hasFresh(key)) return;
        const opts = deps.settings.buildFrameOptions(deps.settings.effectiveMaxLongEdge());
        void runPreparePromise(current, opts, key, prepareGate.current());
      },
      { defer: true },
    ),
  );

  // ── stage size + frameSize memo ──────────────────────────────────
  //
  // The preview wrapper's pixel size is computed here from the
  // measured stage rect and the source aspect, then handed to the
  // wrapper as inline `width` / `height`. This is the only
  // reliably-rendering contain-fit when the child of the wrapper is
  // a `<canvas>` (whose own intrinsic size confuses CSS grid-item
  // min-content negotiation).
  //
  // The stage `<div>` mounts/unmounts as `mode` flips between
  // empty / single / batch (via `<Show>`), so the ref signal lets
  // `createEffect` rebuild the ResizeObserver each time the node
  // is recreated — a one-shot `onMount` would have only ever
  // caught the initial empty-mode null.
  const [stageEl, setStageEl] = createSignal<HTMLDivElement | undefined>();
  const [stageSize, setStageSize] = createSignal<{ w: number; h: number }>({ w: 0, h: 0 });

  createEffect(() => {
    const el = stageEl();
    if (!el) {
      setStageSize({ w: 0, h: 0 });
      return;
    }
    // Seed with a synchronous measurement so the first paint doesn't
    // have to wait for the observer to tick.
    setStageSize({ w: el.clientWidth, h: el.clientHeight });
    const ro = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      setStageSize({ w: entry.contentRect.width, h: entry.contentRect.height });
    });
    ro.observe(el);
    onCleanup(() => ro.disconnect());
  });

  const frameSize = createMemo<{ width: string; height: string } | null>(() => {
    const px = pixels();
    const stage = stageSize();
    if (!px) return null;
    const fit = containFit({ srcW: px.width, srcH: px.height, stageW: stage.w, stageH: stage.h });
    if (!fit) return null;
    return { width: `${fit.width}px`, height: `${fit.height}px` };
  });

  // ── busy + onDownload ────────────────────────────────────────────
  const [busy, setBusy] = createSignal(false);

  const onDownload = async (): Promise<void> => {
    const current = deps.source();
    if (!current) return;
    setBusy(true);
    deps.setStatus('Framing at full resolution…');
    const started = performance.now();
    try {
      const full = await preparePixels(
        current.data,
        deps.settings.buildFrameOptions(deps.settings.effectiveMaxLongEdge()),
      );
      // Phase F3-lite — `encodeJpeg` slices internally before worker
      // transfer; a second slice here was redundant.
      const jpeg = await encodeJpeg(full.rgba, full.width, full.height, deps.settings.quality());
      const blob = new Blob([uint8ToBuffer(jpeg)], { type: 'image/jpeg' });
      triggerDownload(blob, framedName(current.name));
      deps.setStatus(`Saved in ${Math.round(performance.now() - started)} ms`);
    } catch (error) {
      deps.setStatus(`Error: ${stringifyError(error)}`);
    } finally {
      setBusy(false);
    }
  };

  // ── lifecycle: bump gate on owner unmount ────────────────────────
  // The owning reactive scope's `onCleanup` chain catches the stage
  // ResizeObserver via the inner `onCleanup` above; this catch-all
  // ensures any in-flight prepares/prefetches see "not current" too.
  onCleanup(() => {
    prepareGate.bump();
  });

  return {
    state: { pixels, frameSize, busy },
    actions: { setStageEl, onDownload },
    dispose: () => {
      prepareGate.bump();
      setVariants((c) => c.clear());
    },
  };
};

const triggerDownload = (blob: Blob, name: string): void => {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = name;
  anchor.click();
  URL.revokeObjectURL(url);
};
