/**
 * Solid-reactive primitive owning the batch processing pipeline.
 *
 * Bundles two distinct sub-flights that share the same worker queue
 * and the same `files` source:
 *
 *   - **Thumbnail pass** — debounced framed-preview thumbnails for
 *     every batch row, regenerated whenever `files` or the render
 *     settings change. Sequential through the worker so WASM's
 *     decode cache amortises the cost. Per-row depth-2 cache
 *     (current + previous) makes the typical "toggle a setting,
 *     then toggle back" round-trip instant (no worker re-call) and
 *     keeps the stale-while-revalidate UX — the gallery never
 *     blanks while a new variant is in flight.
 *   - **Full-resolution encode pass** — the heavy batch work, fired
 *     as a tail-call from the thumbnail pass so the worker queue
 *     runs thumb → full in that order (without this the user
 *     stares at pulsing placeholders for the whole batch's
 *     duration).
 *
 * Both flights are gated by `createGenerationGate()` so a settings
 * change or a new batch drop invalidates in-flight replies before
 * they can write into the wrong row. Per-row thumbnail and
 * `resultUrl` blob URLs are owned here too and revoked at the
 * appropriate lifecycle moment (LRU evict, file-set swap, dispose).
 *
 * Every per-row swap of the visible thumbnail is wrapped in the
 * View Transitions API helper (`lib/view-transition.ts`) so the
 * crossfade is GPU-composited rather than a hard cut. The
 * `transitionName` field on each row gives the API a stable
 * identity to match snapshots across renders, and crucially it
 * scopes the animation to *that* `<img>` so concurrent per-row
 * transitions don't conflict.
 */

import { downloadZip } from 'client-zip';
import {
  type Accessor,
  createEffect,
  createMemo,
  createRenderEffect,
  createSignal,
  on,
  onCleanup,
} from 'solid-js';
import type { DroppedFile } from '../DropZone';
import type {
  BatchItem,
  BatchResult,
  PipelineSpec,
  WorkerReply,
  WorkerRequest,
} from '../frame-client';
import { generateThumbnailBlob } from '../frame-client';
import { createGenerationGate } from '../lib/batch-sequencer';
import { framedName, uint8ToBuffer } from '../lib/format';
import { variantKey, type VariantKey } from '../lib/variants';
import { withViewTransition } from '../lib/view-transition';
import type { MessageTarget } from '../lib/worker-channel';
import type { AppSettings } from './app-settings';

/** One depth of the per-row thumbnail cache. `key` records the
 *  spec the URL was generated under, so the cache lookup can ask
 *  "is this the variant the user just asked for?". */
type CachedThumb = { url: string; key: VariantKey };

export type BatchRow = {
  key: string;
  name: string;
  /** Stable identifier for the per-row View Transition. Set once
   *  at row creation and never reassigned; the View Transitions
   *  API uses it to match the same `<img>` element across
   *  renders so concurrent per-row crossfades don't conflict. */
  transitionName: string;
  status: 'queued' | 'processing' | 'done' | 'error';
  /** Cumulative pipeline progress for this item, 0..100. Only
   * meaningful while `status === 'processing'`. */
  percent?: number;
  /** Last pipeline stage that finished for this item; absent before
   * decode completes. */
  stage?: 'decode' | 'frame' | 'encode';
  /** Active thumbnail (what the gallery displays). `undefined`
   *  only until the first regenerate finishes. */
  thumb?: CachedThumb;
  /** Previous thumbnail kept around for one generation so a
   *  toggle-back to the prior spec is instant (no worker round-
   *  trip). Evicted + revoked when a *new* spec lands. */
  prevThumb?: CachedThumb;
  resultUrl?: string;
  message?: string;
};

// Thumbnail long-edge cap — sized for the masonry layout's column
// width times a comfortable DPR. Cards span `phi.5` (~178 px) per
// column at the widest viewport, and a 2× DPR pass through that
// demands ~360 px. 480 keeps the thumb sharp on retina-class
// displays without paying for full-resolution frame composition.
const THUMBNAIL_LONG_EDGE = 480;
// Debounce window for thumbnail regeneration on theme/layout/
// meta-policy toggles. The user often drags through preset states
// before settling.
const THUMBNAIL_DEBOUNCE_MS = 320;

/** Project the user's full `PipelineSpec` onto the subset of
 *  fields that actually affect a thumbnail's pixels. The
 *  thumbnail pass always uses `THUMBNAIL_LONG_EDGE` and a fixed
 *  quality, so neither contributes to the cache identity. */
const thumbnailKey = (spec: PipelineSpec): VariantKey =>
  variantKey(spec.frame_style, spec.theme, spec.layout, spec.meta_policy);

export type BatchSession = {
  state: {
    rows: Accessor<readonly BatchRow[]>;
    doneCount: Accessor<number>;
  };
  actions: {
    onDownloadAll: () => Promise<void>;
  };
  /** Wipe rows, invalidate in-flight gates, detach worker listener,
   *  and revoke every owned blob URL. Called from `clearSession`
   *  (start-over) and the app's `onCleanup`. */
  dispose: () => void;
};

export const createBatchSession = (deps: {
  files: Accessor<DroppedFile[] | null>;
  settings: AppSettings['state'];
  /** One-way write seam for the shell-owned status line. */
  setStatus: (s: string) => void;
  /** Worker that owns the WASM module. Same singleton used by
   *  `preparePixels` / `encodeJpeg` — passing it through the seam
   *  lets the batch listener attach/detach independently of the
   *  request-ID `exchange()` callers without contention. */
  workerTarget: MessageTarget;
}): BatchSession => {
  const [rows, setRows] = createSignal<BatchRow[]>([]);

  const thumbnailGate = createGenerationGate();
  let thumbnailDebounce: ReturnType<typeof setTimeout> | null = null;

  const batchGate = createGenerationGate();
  let batchProcessHandler: ((event: MessageEvent<unknown>) => void) | null = null;

  // ── row lifecycle ────────────────────────────────────────────────
  const revokeAllUrls = (): void => {
    for (const row of rows()) {
      if (row.thumb) URL.revokeObjectURL(row.thumb.url);
      if (row.prevThumb) URL.revokeObjectURL(row.prevThumb.url);
      if (row.resultUrl) URL.revokeObjectURL(row.resultUrl);
    }
  };

  // Seed rows whenever a new batch arrives. Each row starts queued
  // (no thumbnail, no result); the thumbnail effect below paints in
  // the previews and the worker pass populates `resultUrl`.
  //
  // `createRenderEffect` (not `createEffect`) so the seed lands in
  // the same synchronous tick as the `batchFiles` signal update —
  // otherwise the Gallery would briefly render with `rows = []`
  // between `setBatchFiles(...)` and the next microtask, flashing
  // empty before the queued placeholders appear.
  //
  // File-set transitions are also URL-lifecycle boundaries: every
  // thumbnail (current + previous) and `resultUrl` the previous
  // batch owned becomes unreachable when we replace `rows`, so
  // revoke them up-front to keep the blob registry from growing
  // across drop-and-redrop cycles.
  createRenderEffect(
    on(deps.files, (files) => {
      revokeAllUrls();
      if (!files) {
        setRows([]);
        return;
      }
      setRows(
        files.map((f, idx) => ({
          key: f.name,
          name: f.name,
          // `gallery-thumb-${idx}` is a fresh CSS ident per row
          // and stays valid across re-renders because batch rows
          // never reorder (sorted by drop order at this point).
          transitionName: `gallery-thumb-${idx}`,
          status: 'queued' as const,
        })),
      );
    }),
  );

  // ── thumbnail effect ─────────────────────────────────────────────
  //
  // Regenerate framed-preview thumbnails when the batch scope (file
  // set) or the frame settings change. Debounced so a drag through
  // preset states doesn't fire one regen per intermediate value.
  createEffect(
    on(
      () => {
        const files = deps.files();
        if (!files) return null;
        // Build a thumbnail spec from the user's current frame
        // settings; `generateThumbnailBlob` overrides
        // `max_long_edge` (and the JPEG quality) to the thumbnail
        // pass values.
        return { files, spec: deps.settings.buildSpec(null) };
      },
      (scope) => {
        if (thumbnailDebounce !== null) clearTimeout(thumbnailDebounce);
        if (!scope) return;
        thumbnailDebounce = setTimeout(() => {
          thumbnailDebounce = null;
          void regenerateBatchThumbnails(scope.files, scope.spec);
        }, THUMBNAIL_DEBOUNCE_MS);
      },
    ),
  );

  const regenerateBatchThumbnails = async (
    files: DroppedFile[],
    baseSpec: PipelineSpec,
  ): Promise<void> => {
    const gen = thumbnailGate.bump();
    const newKey = thumbnailKey(baseSpec);

    // Phase 1 (sync): classify each row against the cache.
    //   - Already-current: no work.
    //   - Toggle-back hit: prevThumb matches the new key — record
    //     the row for a same-tick swap.
    //   - Cache miss: queue the file for sequential regen below.
    //
    // The classification runs OUTSIDE `withViewTransition` because
    // `document.startViewTransition` defers its callback to the
    // next rendering tick. If we built `toRegen` inside that
    // callback, the for-loop below would see an empty list and
    // bail before any thumbnail got generated. The cache-hit
    // swap is wrapped separately so the cache-hit case still
    // animates as a GPU crossfade per row.
    const toRegen: DroppedFile[] = [];
    const swapTargets = new Set<string>();
    for (const r of rows()) {
      const f = files.find((file) => file.name === r.key);
      if (!f) continue;
      if (r.thumb && r.thumb.key === newKey) continue;
      if (r.prevThumb && r.prevThumb.key === newKey) {
        swapTargets.add(r.key);
        continue;
      }
      toRegen.push(f);
    }

    if (swapTargets.size > 0) {
      withViewTransition(() => {
        setRows((rs) =>
          rs.map((r) => {
            if (!swapTargets.has(r.key) || !r.prevThumb) return r;
            // Swap the two depth-2 layers — both stay alive, just
            // their roles flip. The newly-promoted layer becomes
            // visible; the demoted layer waits in `prevThumb` for
            // the next toggle-back.
            return {
              ...r,
              thumb: r.prevThumb,
              ...(r.thumb ? { prevThumb: r.thumb } : {}),
            };
          }),
        );
      });
    }

    for (const file of toRegen) {
      if (!thumbnailGate.isCurrent(gen)) return;
      try {
        const blob = await generateThumbnailBlob(file.data, baseSpec, THUMBNAIL_LONG_EDGE);
        if (!thumbnailGate.isCurrent(gen)) return;
        const url = URL.createObjectURL(blob);
        const fresh: CachedThumb = { url, key: newKey };
        withViewTransition(() => {
          setRows((rs) =>
            rs.map((r) => {
              if (r.key !== file.name) return r;
              if (!thumbnailGate.isCurrent(gen)) {
                // The setRows closure can run a tick after the
                // await above; if a newer generation has bumped
                // in the meantime, drop this URL on the floor so
                // it isn't adopted as the row's value.
                URL.revokeObjectURL(url);
                return r;
              }
              // Rotate the depth-2 LRU: evict the oldest entry
              // (current `prevThumb`), demote the visible `thumb`
              // to `prevThumb`, install `fresh` as the new
              // visible layer. Revoking happens before any of
              // the references are dropped, so the registry
              // stays accurate even after a regen storm.
              if (r.prevThumb) URL.revokeObjectURL(r.prevThumb.url);
              return {
                ...r,
                thumb: fresh,
                ...(r.thumb ? { prevThumb: r.thumb } : {}),
              };
            }),
          );
        });
      } catch (error) {
        if (!thumbnailGate.isCurrent(gen)) return;
        // Thumbnail failure is non-fatal — the gallery keeps the
        // previous (or empty) thumbnail visible. Surface the
        // error so it's not silent (memory: "production error
        // handling — no silent fallbacks").
        // biome-ignore lint/suspicious/noConsole: thumbnail failure diagnostic
        console.warn('thumbnail generation failed', file.name, error);
      }
    }
    // Thumbnails are all in flight now; kick off the full-
    // resolution background pass so the Worker queue runs thumb →
    // full in that order. Without this sequencing the batch encode
    // (which dwarfs each thumb in cost) would grab the Worker first
    // and the user would stare at pulsing placeholders for the
    // whole batch's duration.
    if (thumbnailGate.isCurrent(gen)) onProcessBatch();
  };

  // ── batch processing pass ────────────────────────────────────────
  const detachBatchProcessHandler = (): void => {
    if (batchProcessHandler === null) return;
    deps.workerTarget.removeEventListener('message', batchProcessHandler);
    batchProcessHandler = null;
  };

  const applyBatchResults = (results: BatchResult[]): void => {
    setRows((rs) =>
      rs.map((r) => {
        const match = results.find((res) => res.key === r.key);
        if (!match) return r;
        // Both branches own the lifecycle of any pre-existing
        // `resultUrl` on this row (the previous batch's output).
        // Revoke before overwriting so a settings-change → regen
        // cycle doesn't leak the prior blob into the registry.
        if (r.resultUrl) URL.revokeObjectURL(r.resultUrl);
        if (match.ok) {
          const blob = new Blob([uint8ToBuffer(match.result)], { type: 'image/jpeg' });
          const url = URL.createObjectURL(blob);
          return { ...r, status: 'done', resultUrl: url, message: `${match.elapsed_ms} ms` };
        }
        const { resultUrl: _drop, ...rest } = r;
        return { ...rest, status: 'error', message: match.result };
      }),
    );
  };

  const onProcessBatch = (): void => {
    const files = deps.files();
    if (!files) return;
    const gen = batchGate.bump();
    // Detach the previous run's listener so it can't race into the
    // new generation's row state.
    detachBatchProcessHandler();
    deps.setStatus(`Processing ${files.length} files in the background…`);
    const handle = (event: MessageEvent<unknown>): void => {
      if (!batchGate.isCurrent(gen)) return;
      const msg = event.data as WorkerReply;
      if (msg.kind === 'progress') {
        setRows((rs) =>
          rs.map((r) =>
            r.key === msg.key
              ? {
                  ...r,
                  status: 'processing',
                  percent: msg.percent,
                  ...(msg.stage ? { stage: msg.stage } : {}),
                }
              : r,
          ),
        );
      } else if (msg.kind === 'done') {
        applyBatchResults(msg.results);
        const ok = msg.results.filter((r) => r.ok).length;
        deps.setStatus(`Batch done: ${ok}/${msg.results.length} succeeded.`);
        detachBatchProcessHandler();
      } else if (msg.kind === 'error' && msg.requestId === null) {
        deps.setStatus(`Batch failed: ${msg.message}`);
        setRows((rs) => rs.map((r) => ({ ...r, status: 'error', message: msg.message })));
        detachBatchProcessHandler();
      }
      // Other replies (prepared/encoded/non-batch error) belong to
      // other requesters and are ignored here.
    };
    batchProcessHandler = handle;
    deps.workerTarget.addEventListener('message', handle);
    const items: BatchItem[] = files.map((f) => ({ key: f.name, bytes: f.data }));
    const request: WorkerRequest = {
      kind: 'batch',
      items,
      spec: deps.settings.buildSpec(deps.settings.effectiveMaxLongEdge()),
    };
    deps.workerTarget.postMessage(request, []);
  };

  // ── derived ──────────────────────────────────────────────────────
  const doneCount = createMemo(() => rows().filter((r) => r.status === 'done').length);

  // ── download all ─────────────────────────────────────────────────
  //
  // Bundle every ready row into a single zip and trigger one
  // download. Using `client-zip` (≈3 kB gzip, streaming) avoids
  // the "this site wants to download N files" permission prompt —
  // the user gets exactly one file, named with an ISO-style
  // timestamp so successive batches sort cleanly in Downloads.
  const onDownloadAll = async (): Promise<void> => {
    const ready = rows().filter((r) => r.status === 'done' && r.resultUrl);
    if (ready.length === 0) return;
    const entries = await Promise.all(
      ready.map(async (r) => {
        // `resultUrl` is a blob: URL — `fetch` round-trips back
        // into the original Blob without copying the underlying
        // bytes (the blob registry hands out a reference).
        const blob = await fetch(r.resultUrl as string).then((res) => res.blob());
        return { name: framedName(r.name), input: blob, lastModified: new Date() };
      }),
    );
    const zipBlob = await downloadZip(entries).blob();
    const url = URL.createObjectURL(zipBlob);
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
    const anchor = document.createElement('a');
    anchor.href = url;
    // File-count in the archive name so successive batches read
    // unambiguously in the Downloads folder ("photo-frame-12-
    // photos-…zip" tells the user how many photos are inside
    // without unzipping).
    const noun = entries.length === 1 ? 'photo' : 'photos';
    anchor.download = `photo-frame-${entries.length}${noun}-${timestamp}.zip`;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  // ── lifecycle ────────────────────────────────────────────────────
  onCleanup(() => {
    if (thumbnailDebounce !== null) clearTimeout(thumbnailDebounce);
    thumbnailGate.bump();
    batchGate.bump();
    detachBatchProcessHandler();
    revokeAllUrls();
  });

  return {
    state: { rows, doneCount },
    actions: { onDownloadAll },
    dispose: () => {
      if (thumbnailDebounce !== null) {
        clearTimeout(thumbnailDebounce);
        thumbnailDebounce = null;
      }
      thumbnailGate.bump();
      batchGate.bump();
      detachBatchProcessHandler();
      revokeAllUrls();
      setRows([]);
    },
  };
};
