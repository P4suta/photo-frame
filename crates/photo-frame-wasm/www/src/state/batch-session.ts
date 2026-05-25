/**
 * Solid-reactive primitive owning the batch processing pipeline.
 *
 * Bundles two distinct sub-flights that share the same worker queue
 * and the same `files` source:
 *
 *   - **Thumbnail pass** — debounced framed-preview thumbnails for
 *     every batch row, regenerated whenever `files` or the render
 *     settings change. Sequential through the worker so WASM's
 *     decode cache amortises the cost.
 *   - **Full-resolution encode pass** — the heavy batch work, fired
 *     as a tail-call from the thumbnail pass so the worker queue
 *     runs thumb → full in that order (without this the user
 *     stares at pulsing placeholders for the whole batch's
 *     duration).
 *
 * Both flights are gated by `createGenerationGate()` so a settings
 * change or a new batch drop invalidates in-flight replies before
 * they can write into the wrong row. Per-row `thumbnailUrl` and
 * `resultUrl` blob URLs are owned here too and revoked on every
 * regenerate / dispose path.
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
  FrameOptionsForPrepare,
  WorkerReply,
  WorkerRequest,
} from '../frame-client';
import { generateThumbnailBlob } from '../frame-client';
import { createGenerationGate } from '../lib/batch-sequencer';
import { framedName, uint8ToBuffer } from '../lib/format';
import type { MessageTarget } from '../lib/worker-channel';
import type { AppSettings } from './app-settings';

export type BatchRow = {
  key: string;
  name: string;
  status: 'queued' | 'processing' | 'done' | 'error';
  thumbnailUrl?: string;
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
// show_meta toggles. The user often drags through preset states
// before settling.
const THUMBNAIL_DEBOUNCE_MS = 320;

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
      if (row.thumbnailUrl) URL.revokeObjectURL(row.thumbnailUrl);
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
  createRenderEffect(
    on(deps.files, (files) => {
      if (!files) {
        setRows([]);
        return;
      }
      setRows(
        files.map((f) => ({
          key: f.name,
          name: f.name,
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
        return {
          files,
          theme: deps.settings.theme(),
          layout: deps.settings.layout(),
          showMeta: deps.settings.showMeta(),
        };
      },
      (scope) => {
        if (thumbnailDebounce !== null) clearTimeout(thumbnailDebounce);
        if (!scope) return;
        thumbnailDebounce = setTimeout(() => {
          thumbnailDebounce = null;
          void regenerateBatchThumbnails(scope.files, {
            theme: scope.theme,
            layout: scope.layout,
            show_meta: scope.showMeta,
          });
        }, THUMBNAIL_DEBOUNCE_MS);
      },
    ),
  );

  const regenerateBatchThumbnails = async (
    files: DroppedFile[],
    options: Omit<FrameOptionsForPrepare, 'max_long_edge'>,
  ): Promise<void> => {
    const gen = thumbnailGate.bump();
    // Revoke previous thumbnails and clear the field on every row
    // so the gallery shows pulsing placeholders again immediately.
    setRows((rs) => {
      for (const r of rs) {
        if (r.thumbnailUrl) URL.revokeObjectURL(r.thumbnailUrl);
      }
      // Destructure to *omit* `thumbnailUrl` rather than set it to
      // `undefined` — `exactOptionalPropertyTypes` is on in
      // tsconfig, so the two aren't interchangeable.
      return rs.map(({ thumbnailUrl: _, ...rest }) => rest);
    });
    for (const file of files) {
      if (!thumbnailGate.isCurrent(gen)) return;
      try {
        const blob = await generateThumbnailBlob(file.data, options, THUMBNAIL_LONG_EDGE);
        if (!thumbnailGate.isCurrent(gen)) return;
        const url = URL.createObjectURL(blob);
        setRows((rs) =>
          rs.map((r) => {
            if (r.key !== file.name) return r;
            if (r.thumbnailUrl && !thumbnailGate.isCurrent(gen)) {
              URL.revokeObjectURL(url);
              return r;
            }
            if (r.thumbnailUrl) URL.revokeObjectURL(r.thumbnailUrl);
            return { ...r, thumbnailUrl: url };
          }),
        );
      } catch (error) {
        if (!thumbnailGate.isCurrent(gen)) return;
        // Thumbnail failure is non-fatal — the gallery keeps the
        // placeholder visible. Surface the error so it's not
        // silent (memory: "production error handling — no silent
        // fallbacks").
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
        if (match.ok) {
          const blob = new Blob([uint8ToBuffer(match.result)], { type: 'image/jpeg' });
          const url = URL.createObjectURL(blob);
          return { ...r, status: 'done', resultUrl: url, message: `${match.elapsed_ms} ms` };
        }
        return { ...r, status: 'error', message: match.result };
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
        setRows((rs) => rs.map((r) => (r.key === msg.key ? { ...r, status: 'processing' } : r)));
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
      options: deps.settings.buildPipelineOptions(deps.settings.effectiveMaxLongEdge()),
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
