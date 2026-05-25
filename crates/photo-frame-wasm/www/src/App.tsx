import {
  createEffect,
  createMemo,
  createSignal,
  For,
  on,
  onCleanup,
  onMount,
  Show,
} from 'solid-js';
import { appShell, button, segmentedButton } from '../styled-system/recipes';
import {
  appHeader,
  brand,
  checkInline,
  controls,
  field,
  fieldBody,
  fieldLabel,
  headerStatus,
  meter,
  meterLabel,
  meterRow,
  meterValue,
  previewCanvas,
  previewFrame,
  resizeRow,
  segmented,
  sidebar,
  sidebarFooter,
  stage,
  stageBatch,
  stageCanvas,
  stageEmpty,
  suffix,
  tagline,
  wordmark,
} from './App.styles';
import { downloadZip } from 'client-zip';
import { type DroppedFile, DropZone } from './DropZone';
import {
  type BatchItem,
  type BatchResult,
  type CaptionLayout,
  disposeWorker,
  encodeJpeg,
  type FrameOptionsForPrepare,
  type FrameTheme,
  generateThumbnailBlob,
  getWorker,
  type PipelineOptions,
  type PreparedPixels,
  preparePixels,
  type WorkerReply,
  type WorkerRequest,
} from './frame-client';
import { Gallery } from './Gallery';

const PREVIEW_LONG_EDGE = 1600;
// Phase G1 dropped the prepare-side debounce entirely: the WASM
// decoded-photograph cache makes redundant prepares cheap (~50 ms
// frame stage only), and the Phase G2 prefetch loop already
// serialises through the Worker queue. `ESTIMATE_DEBOUNCE_MS` stays
// because quality-slider drag still emits dense ticks that an
// undebounced encode would chase.
const ESTIMATE_DEBOUNCE_MS = 220;

// `value` mirrors the Rust enum (`paper`/`ink`), `label` is the
// UI face — direct colour names read more honestly than the
// material metaphors did.
const THEMES = [
  { value: 'paper' as const, label: 'White', description: 'White frame, dark text' },
  { value: 'ink' as const, label: 'Black', description: 'Black frame, light text' },
] satisfies ReadonlyArray<{ value: FrameTheme; label: string; description: string }>;

// The UI-facing union for the caption picker: either no caption
// at all, or one of the two layout arrangements. We don't store
// this directly — the existing `layout` + `showMeta` signals
// remain the source of truth so the WASM render options struct
// stays unchanged. The handler in `ControlsCommon` translates
// between this UI union and the two underlying signals.
type CaptionMode = 'off' | CaptionLayout;
const CAPTION_MODES = [
  { value: 'off' as const, label: 'Off', description: 'No metadata caption' },
  { value: 'edges' as const, label: 'Edges', description: 'Four-corner liit-style layout' },
  {
    value: 'centered' as const,
    label: 'Centered',
    description: 'Both rows centred under the photo',
  },
] satisfies ReadonlyArray<{ value: CaptionMode; label: string; description: string }>;

/**
 * Mirror of `photo_frame::QualityPreset` — keep in sync with
 * `crates/photo-frame-types/src/preset.rs`. The Rust side is the source
 * of truth; the duplication here keeps the UI snappy without a WASM
 * round-trip for every preset click.
 */
const PRESETS = {
  sns: { label: 'SNS', quality: 78, maxLongEdge: 2048 as number | null },
  standard: { label: 'Standard', quality: 92, maxLongEdge: null as number | null },
  maximum: { label: 'Maximum', quality: 98, maxLongEdge: null as number | null },
} as const satisfies Record<string, { label: string; quality: number; maxLongEdge: number | null }>;

type PresetKey = keyof typeof PRESETS;

// Row state for the batch gallery. `thumbnailUrl` is the small
// framed preview shown while the row sits queued / processing;
// `resultUrl` is the full-resolution JPEG that lets the card
// double as a download trigger once the row reaches `done`.
type BatchRow = {
  key: string;
  name: string;
  status: 'queued' | 'processing' | 'done' | 'error';
  thumbnailUrl?: string;
  resultUrl?: string;
  message?: string;
};

// Thumbnail long-edge cap — sized for the masonry layout's
// column width times a comfortable DPR. Cards span `phi.5`
// (~178 px) per column at the widest viewport, and a 2× DPR
// pass through that demands ~360 px. 480 keeps the thumb sharp
// on retina-class displays without paying for full-resolution
// frame composition.
const THUMBNAIL_LONG_EDGE = 480;
// Debounce window for thumbnail regeneration on theme/layout/
// show_meta toggles. Mirrors `ESTIMATE_DEBOUNCE_MS` rationale —
// the user often drags through preset states before settling.
const THUMBNAIL_DEBOUNCE_MS = 320;

type Mode = 'empty' | 'single' | 'batch';

// Phase G2 — pre-render every (theme × layout × showMeta) variant in
// the background so toggles are signal-swap fast (0 ms WASM round
// trip). `VariantKey` strings index the cache below; `ALL_VARIANTS`
// enumerates the 8 combinations so the prefetch loop has a fixed
// iteration order (deterministic, easier to debug).
type VariantKey = string;
const variantKey = (theme: FrameTheme, layout: CaptionLayout, showMeta: boolean): VariantKey =>
  `${theme}|${layout}|${showMeta}`;
const ALL_VARIANTS: ReadonlyArray<{
  theme: FrameTheme;
  layout: CaptionLayout;
  showMeta: boolean;
}> = (() => {
  const out: { theme: FrameTheme; layout: CaptionLayout; showMeta: boolean }[] = [];
  for (const theme of ['paper', 'ink'] as const) {
    for (const layout of ['edges', 'centered'] as const) {
      for (const showMeta of [true, false] as const) {
        out.push({ theme, layout, showMeta });
      }
    }
  }
  return out;
})();

export const App = () => {
  const [single, setSingle] = createSignal<DroppedFile | null>(null);
  // Phase G2 — keyed by `VariantKey`; entries are populated by the
  // prepare-then-prefetch effect. `previewPixels` below is the
  // derived "what to draw right now" memo.
  const [previewVariants, setPreviewVariants] = createSignal<Map<VariantKey, PreparedPixels>>(
    new Map(),
  );
  const [previewSize, setPreviewSize] = createSignal<number | null>(null);
  const [previewElapsedMs, setPreviewElapsedMs] = createSignal<number | null>(null);
  const [batchRows, setBatchRows] = createSignal<BatchRow[]>([]);
  const [batchFiles, setBatchFiles] = createSignal<DroppedFile[] | null>(null);
  const [preset, setPreset] = createSignal<PresetKey>('standard');
  const [quality, setQuality] = createSignal<number>(PRESETS.standard.quality);
  const [resize, setResize] = createSignal(false);
  const [resizePx, setResizePx] = createSignal<number>(2048);
  const [theme, setTheme] = createSignal<FrameTheme>('paper');
  const [layout, setLayout] = createSignal<CaptionLayout>('edges');
  const [showMeta, setShowMeta] = createSignal(true);
  const [status, setStatus] = createSignal('');
  const [busy, setBusy] = createSignal(false);

  const mode = createMemo<Mode>(() =>
    batchFiles() !== null ? 'batch' : single() !== null ? 'single' : 'empty',
  );

  const applyPreset = (key: PresetKey): void => {
    setPreset(key);
    const p = PRESETS[key];
    setQuality(p.quality);
    if (p.maxLongEdge === null) {
      setResize(false);
    } else {
      setResize(true);
      setResizePx(p.maxLongEdge);
    }
  };

  const effectiveMaxLongEdge = createMemo<number | null>(() =>
    resize() ? Math.max(1, resizePx()) : null,
  );

  const buildFrameOptions = (maxLongEdge: number | null): FrameOptionsForPrepare => ({
    theme: theme(),
    layout: layout(),
    show_meta: showMeta(),
    max_long_edge: maxLongEdge,
  });

  // Phase G2 — `currentVariantKey` tracks which (theme, layout,
  // show_meta) tuple the user is looking at right now;
  // `previewPixels` is the cached preview for that tuple, or `null`
  // if the prepare/prefetch effect hasn't filled it yet.
  const currentVariantKey = createMemo(() => variantKey(theme(), layout(), showMeta()));
  const previewPixels = createMemo<PreparedPixels | null>(
    () => previewVariants().get(currentVariantKey()) ?? null,
  );

  const buildPipelineOptions = (maxLongEdge: number | null): PipelineOptions => ({
    ...buildFrameOptions(maxLongEdge),
    jpeg_quality: quality(),
  });

  // Reset every transient piece of state to the empty drop-zone
  // view. Shared between `onDrop` (which then sets the new files)
  // and `resetToEmpty` (called by the brand wordmark link).
  const clearSession = (): void => {
    revokeAllBatchUrls();
    setPreviewVariants(new Map());
    setPreviewSize(null);
    setPreviewElapsedMs(null);
    setStatus('');
    setSingle(null);
    setBatchFiles(null);
    setBatchRows([]);
  };

  const onDrop = (files: DroppedFile[]): void => {
    clearSession();
    const [first] = files;
    if (files.length === 1 && first) {
      setSingle(first);
    } else {
      setBatchFiles(files);
      setBatchRows(
        files.map((f) => ({
          key: f.name,
          name: f.name,
          status: 'queued',
        })),
      );
    }
  };

  // Wordmark / brand-home click target.
  const resetToEmpty = (): void => {
    clearSession();
  };

  const revokeAllBatchUrls = (): void => {
    for (const row of batchRows()) {
      if (row.thumbnailUrl) URL.revokeObjectURL(row.thumbnailUrl);
      if (row.resultUrl) URL.revokeObjectURL(row.resultUrl);
    }
  };

  // ── prepare + prefetch effect ────────────────────────────────────
  //
  // Phase G2 — every (image, max_long_edge) scope owns its own
  // `previewVariants` map. When the scope changes we wipe the map and:
  //
  //   1. Prepare the *currently displayed* variant first (highest UX
  //      priority — this is the only one the user actively waits for).
  //   2. Sequentially prefetch the remaining 7 (theme × layout ×
  //      show_meta) variants in the background. WASM's Phase G1
  //      `cached_or_decode` reuses the decoded `Photograph` across
  //      these calls so each prefetch only pays the frame stage
  //      (~50–200 ms at preview res), not a full decode.
  //
  // `prepareGeneration` invalidates in-flight prefetches when the
  // user drops a new image or moves the max-long-edge slider — only
  // the *currently relevant* scope ever writes back into the map.
  // The Worker serialises requests via `exchange()`'s request-ID
  // filter, and stale completions are dropped by the generation
  // check before they touch the signal.
  let prepareGeneration = 0;
  createEffect(
    on(
      () => {
        const current = single();
        if (!current) return null;
        return {
          current,
          maxLongEdge: Math.min(PREVIEW_LONG_EDGE, effectiveMaxLongEdge() ?? Infinity),
        };
      },
      (scope) => {
        if (!scope) return;
        prepareGeneration += 1;
        const gen = prepareGeneration;
        setPreviewVariants(new Map());
        const maxLongEdge =
          scope.maxLongEdge === Number.POSITIVE_INFINITY ? null : scope.maxLongEdge;
        // Snapshot the user-visible variant at scope-change time —
        // we prepare this one first so the canvas updates ASAP.
        const initial = {
          theme: theme(),
          layout: layout(),
          showMeta: showMeta(),
        };
        void runPrepareAndPrefetch(scope.current, initial, maxLongEdge, gen);
      },
    ),
  );

  // Phase G2 — if the user toggles theme/layout/show_meta to a
  // variant the prefetch loop hasn't reached yet, kick off an
  // out-of-order prepare for it immediately. The prefetch loop
  // skips variants already in the map (see `runPrepareAndPrefetch`)
  // so we don't redo work. Wrapped in `defer: true` so this effect
  // doesn't fire on the initial signal hookup — the scope effect
  // above already covers that path.
  createEffect(
    on(
      currentVariantKey,
      (key) => {
        const current = single();
        if (!current) return;
        if (previewVariants().has(key)) return; // already cached
        const maxLongEdge = effectiveMaxLongEdge();
        const opts: FrameOptionsForPrepare = {
          theme: theme(),
          layout: layout(),
          show_meta: showMeta(),
          max_long_edge: maxLongEdge === null ? null : Math.min(PREVIEW_LONG_EDGE, maxLongEdge),
        };
        void runPreparePromise(current, opts, key, prepareGeneration);
      },
      { defer: true },
    ),
  );

  const runPreparePromise = async (
    current: DroppedFile,
    opts: FrameOptionsForPrepare,
    key: VariantKey,
    gen: number,
  ): Promise<void> => {
    const started = performance.now();
    setStatus('Framing preview…');
    try {
      const pixels = await preparePixels(current.data, opts);
      if (gen !== prepareGeneration || single() !== current) return;
      setPreviewVariants((m) => {
        const next = new Map(m);
        next.set(key, pixels);
        return next;
      });
      setPreviewElapsedMs(Math.round(performance.now() - started));
      setStatus('');
    } catch (error) {
      if (gen === prepareGeneration) setStatus(`Error: ${stringifyError(error)}`);
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
    if (gen !== prepareGeneration) return;
    // Sequential background prefetch — Worker serialises anyway, and
    // sequencing means a user-initiated `runPreparePromise` from the
    // toggle effect above only has at most one prefetch request in
    // flight ahead of it to wait on (~50–200 ms at preview res).
    for (const v of ALL_VARIANTS) {
      if (gen !== prepareGeneration) return;
      const key = variantKey(v.theme, v.layout, v.showMeta);
      if (previewVariants().has(key)) continue;
      const opts: FrameOptionsForPrepare = {
        theme: v.theme,
        layout: v.layout,
        show_meta: v.showMeta,
        max_long_edge: maxLongEdge,
      };
      try {
        const pixels = await preparePixels(current.data, opts);
        if (gen !== prepareGeneration) return;
        setPreviewVariants((m) => {
          const next = new Map(m);
          next.set(key, pixels);
          return next;
        });
      } catch {
        // Prefetch best-effort — silently skip failed variants so a
        // glitch on one combo doesn't poison the UI status line.
      }
    }
  };

  // ── draw effect ──────────────────────────────────────────────────
  // Paints the cached RGBA into a container-sized drawing buffer
  // with `object-fit: contain` semantics. We deliberately do NOT
  // set the canvas's intrinsic size to the image's pixel
  // dimensions — doing so would make the <canvas> element's
  // intrinsic aspect ratio override `object-fit: contain` and
  // crop portrait images vertically inside a wider container.
  // Instead the drawing buffer matches the container × DPR and
  // the image is centred + scaled to fit inside it.
  let canvasRef: HTMLCanvasElement | undefined;

  const paintPreview = (canvas: HTMLCanvasElement): void => {
    const pixels = previewPixels();
    if (!pixels) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    const cssW = canvas.clientWidth;
    const cssH = canvas.clientHeight;
    if (cssW === 0 || cssH === 0) return;
    const dpr = Math.max(1, Math.min(2, window.devicePixelRatio || 1));
    canvas.width = Math.round(cssW * dpr);
    canvas.height = Math.round(cssH * dpr);
    // Author in CSS pixels — the DPR ride is on the transform so
    // the contain-fit maths below works in container units.
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, cssW, cssH);

    // Phase F3-lite — zero-copy view onto the cached RGBA bytes
    // (the WASM-returned `Uint8Array::new_with_length` buffer is
    // a regular ArrayBuffer, never SharedArrayBuffer, so the
    // ImageData spec accepts it without a memcpy).
    const view = new Uint8ClampedArray(
      pixels.rgba.buffer as ArrayBuffer,
      pixels.rgba.byteOffset,
      pixels.rgba.byteLength,
    );
    // Stage the RGBA into an offscreen canvas so `drawImage` can
    // letterbox it. `putImageData` doesn't honour destination
    // rectangles, so going through an offscreen is the minimal
    // way to compose put + scale in one pipeline.
    const off = document.createElement('canvas');
    off.width = pixels.width;
    off.height = pixels.height;
    const offCtx = off.getContext('2d');
    if (!offCtx) return;
    offCtx.putImageData(new ImageData(view, pixels.width, pixels.height), 0, 0);

    const scale = Math.min(cssW / pixels.width, cssH / pixels.height);
    const dw = pixels.width * scale;
    const dh = pixels.height * scale;
    const dx = (cssW - dw) / 2;
    const dy = (cssH - dh) / 2;
    ctx.drawImage(off, dx, dy, dw, dh);
  };

  createEffect(() => {
    // Subscribe to previewPixels (re-paints when it changes).
    previewPixels();
    if (canvasRef) paintPreview(canvasRef);
  });

  onMount(() => {
    if (!canvasRef) return;
    const canvas = canvasRef;
    const ro = new ResizeObserver(() => paintPreview(canvas));
    ro.observe(canvas);
    onCleanup(() => ro.disconnect());
  });

  // ── estimate effect ──────────────────────────────────────────────
  // Re-encodes the cached RGBA at the current quality just to read the
  // resulting byte length; no blob, no canvas redraw. Debounced so a
  // slider drag posts ≤ ~5 encodes/second.
  let estimateTimer: ReturnType<typeof setTimeout> | null = null;
  let estimateToken = 0;
  createEffect(
    on(
      () => {
        const px = previewPixels();
        if (!px) return null;
        return { rgba: px.rgba, width: px.width, height: px.height, quality: quality() };
      },
      (intent) => {
        if (!intent) return;
        if (estimateTimer !== null) clearTimeout(estimateTimer);
        estimateTimer = setTimeout(async () => {
          estimateTimer = null;
          const token = ++estimateToken;
          try {
            // Phase F3-lite — `encodeJpeg` already does its own
            // internal `.slice()` before transferring to the worker,
            // so an extra slice here was a redundant second 24 MB
            // memcpy per estimate cycle (5× per second during slider
            // drag). Pass `intent.rgba` directly; the cached
            // `previewPixels` buffer stays intact because the worker
            // only ever receives the encodeJpeg-side copy.
            const jpeg = await encodeJpeg(intent.rgba, intent.width, intent.height, intent.quality);
            if (token !== estimateToken) return;
            setPreviewSize(jpeg.length);
          } catch (error) {
            if (token !== estimateToken) return;
            setStatus(`Estimate failed: ${stringifyError(error)}`);
          }
        }, ESTIMATE_DEBOUNCE_MS);
      },
    ),
  );

  // ── batch thumbnail effect ───────────────────────────────────────
  //
  // Generate framed-preview thumbnails for every batch row whenever
  // the batch scope (file set) or the frame settings change. Runs
  // sequentially through the worker queue (WASM's photo cache
  // amortises decode across thumbnails for the same source), so a
  // 30-file batch costs roughly N · (decode-once + frame-stage)
  // not N · 2 decode passes.
  //
  // A `generation` counter invalidates in-flight thumbnails when
  // the user drops a new batch or toggles settings — stale promises
  // resolve into a no-op instead of writing the wrong preview into
  // the wrong row.
  let thumbnailGeneration = 0;
  let thumbnailDebounce: ReturnType<typeof setTimeout> | null = null;
  createEffect(
    on(
      () => {
        const files = batchFiles();
        if (!files) return null;
        // Track the settings that affect thumbnail rendering — when
        // any of these change we re-generate the previews.
        return {
          files,
          theme: theme(),
          layout: layout(),
          showMeta: showMeta(),
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
    thumbnailGeneration += 1;
    const gen = thumbnailGeneration;
    // Revoke previous thumbnails and clear the field on every row so
    // the gallery shows pulsing placeholders again immediately.
    setBatchRows((rows) => {
      for (const r of rows) {
        if (r.thumbnailUrl) URL.revokeObjectURL(r.thumbnailUrl);
      }
      // Destructure to *omit* `thumbnailUrl` rather than set it to
      // `undefined` — `exactOptionalPropertyTypes` is on in
      // tsconfig, so the two aren't interchangeable.
      return rows.map(({ thumbnailUrl: _, ...rest }) => rest);
    });
    for (const file of files) {
      if (gen !== thumbnailGeneration) return;
      try {
        const blob = await generateThumbnailBlob(file.data, options, THUMBNAIL_LONG_EDGE);
        if (gen !== thumbnailGeneration) return;
        const url = URL.createObjectURL(blob);
        setBatchRows((rows) =>
          rows.map((r) => {
            if (r.key !== file.name) return r;
            // Defensive: if a newer generation already wrote a
            // thumbnail for this row, revoke our late arrival.
            if (r.thumbnailUrl && gen !== thumbnailGeneration) {
              URL.revokeObjectURL(url);
              return r;
            }
            if (r.thumbnailUrl) URL.revokeObjectURL(r.thumbnailUrl);
            return { ...r, thumbnailUrl: url };
          }),
        );
      } catch (error) {
        if (gen !== thumbnailGeneration) return;
        // Thumbnail failure is non-fatal — the gallery keeps the
        // placeholder visible. Surface the error so it's not silent
        // (memory: "production error handling — no silent fallbacks").
        // biome-ignore lint/suspicious/noConsole: thumbnail failure diagnostic
        console.warn('thumbnail generation failed', file.name, error);
      }
    }
  };

  onCleanup(() => {
    if (estimateTimer !== null) clearTimeout(estimateTimer);
    if (thumbnailDebounce !== null) clearTimeout(thumbnailDebounce);
    // Phase G2 — bumping the generation invalidates any in-flight
    // prepare/prefetch promises so their completion handlers can't
    // touch the (about-to-be-disposed) signal.
    prepareGeneration += 1;
    thumbnailGeneration += 1;
    revokeAllBatchUrls();
    disposeWorker();
  });

  // ── single-image download ────────────────────────────────────────
  // Re-prepares at full resolution (no preview cap), then encodes.
  const onDownloadSingle = async (): Promise<void> => {
    const current = single();
    if (!current) return;
    setBusy(true);
    setStatus('Framing at full resolution…');
    const started = performance.now();
    try {
      const full = await preparePixels(current.data, buildFrameOptions(effectiveMaxLongEdge()));
      // Phase F3-lite — `encodeJpeg` slices internally before
      // worker transfer; a second slice here was redundant. `full`
      // is consumed immediately after so the cache argument doesn't
      // apply, but symmetry with the estimate path above keeps the
      // call-shape consistent.
      const jpeg = await encodeJpeg(full.rgba, full.width, full.height, quality());
      const blob = new Blob([uint8ToBuffer(jpeg)], { type: 'image/jpeg' });
      triggerDownload(blob, framedName(current.name));
      setStatus(`Saved in ${Math.round(performance.now() - started)} ms`);
    } catch (error) {
      setStatus(`Error: ${stringifyError(error)}`);
    } finally {
      setBusy(false);
    }
  };

  // ── batch run ────────────────────────────────────────────────────
  //
  // Runs eagerly in the background as soon as `batchFiles` is set
  // and re-runs (debounced) whenever any render-affecting signal
  // changes. The generation counter + handler-detach guarantee
  // that stale `done` replies from a superseded run can't write
  // into the new batch's rows. The user never has to click a
  // "Process" button — by the time they reach the download
  // affordance, the resultUrl is already populated.
  let batchProcessGeneration = 0;
  let batchProcessHandler: ((event: MessageEvent<WorkerReply>) => void) | null = null;
  const detachBatchProcessHandler = (): void => {
    if (batchProcessHandler === null) return;
    getWorker().removeEventListener('message', batchProcessHandler);
    batchProcessHandler = null;
  };
  const onProcessBatch = (): void => {
    const files = batchFiles();
    if (!files) return;
    batchProcessGeneration += 1;
    const gen = batchProcessGeneration;
    // Detach the previous run's listener so it can't race into
    // the new generation's row state.
    detachBatchProcessHandler();
    setStatus(`Processing ${files.length} files in the background…`);
    const w = getWorker();
    const handle = (event: MessageEvent<WorkerReply>): void => {
      if (gen !== batchProcessGeneration) return;
      const msg = event.data;
      if (msg.kind === 'progress') {
        setBatchRows((rows) =>
          rows.map((r) => (r.key === msg.key ? { ...r, status: 'processing' } : r)),
        );
      } else if (msg.kind === 'done') {
        applyBatchResults(msg.results);
        const ok = msg.results.filter((r) => r.ok).length;
        setStatus(`Batch done: ${ok}/${msg.results.length} succeeded.`);
        detachBatchProcessHandler();
      } else if (msg.kind === 'error' && msg.requestId === null) {
        setStatus(`Batch failed: ${msg.message}`);
        setBatchRows((rows) => rows.map((r) => ({ ...r, status: 'error', message: msg.message })));
        detachBatchProcessHandler();
      }
      // Other replies (prepared/encoded/non-batch error) belong to other
      // requesters and are ignored here.
    };
    batchProcessHandler = handle;
    w.addEventListener('message', handle);
    const items: BatchItem[] = files.map((f) => ({ key: f.name, bytes: f.data }));
    const request: WorkerRequest = {
      kind: 'batch',
      items,
      options: buildPipelineOptions(effectiveMaxLongEdge()),
    };
    w.postMessage(request);
  };

  // Auto-trigger background batch processing whenever the file
  // set or any render-affecting setting changes, debounced so a
  // theme/quality drag doesn't kick off a new full pass per tick.
  const BATCH_PROCESS_DEBOUNCE_MS = 320;
  let batchProcessDebounce: ReturnType<typeof setTimeout> | null = null;
  createEffect(
    on(
      () => {
        const files = batchFiles();
        if (!files) return null;
        return {
          files,
          theme: theme(),
          layout: layout(),
          showMeta: showMeta(),
          quality: quality(),
          maxLongEdge: effectiveMaxLongEdge(),
        };
      },
      (scope) => {
        if (batchProcessDebounce !== null) clearTimeout(batchProcessDebounce);
        if (!scope) return;
        batchProcessDebounce = setTimeout(() => {
          batchProcessDebounce = null;
          onProcessBatch();
        }, BATCH_PROCESS_DEBOUNCE_MS);
      },
    ),
  );

  const applyBatchResults = (results: BatchResult[]): void => {
    setBatchRows((rows) =>
      rows.map((r) => {
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

  // Bundle every ready row into a single zip and trigger one
  // download. Using `client-zip` (≈3 kB gzip, streaming) avoids
  // the "this site wants to download N files" permission prompt
  // — the user gets exactly one file, named with an ISO-style
  // timestamp so successive batches sort cleanly in Downloads.
  const onDownloadAll = async (): Promise<void> => {
    const ready = batchRows().filter((r) => r.status === 'done' && r.resultUrl);
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
    // unambiguously in the Downloads folder ("photo-frame-12-…
    // .zip" tells the user how many photos are inside without
    // unzipping). Singular vs plural keeps the chrome friendly.
    const noun = entries.length === 1 ? 'photo' : 'photos';
    anchor.download = `photo-frame-${entries.length}${noun}-${timestamp}.zip`;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  // Memoised count of rows whose framed output is ready, used
  // by the sidebar "Download all" button's label / disabled
  // state. `total` lets the label read "Download all (4/10)"
  // while the rest are still encoding.
  const batchDoneCount = createMemo(() => batchRows().filter((r) => r.status === 'done').length);

  // ── render ───────────────────────────────────────────────────────
  return (
    <div class={appShell({ mode: mode() })}>
      <header class={appHeader}>
        <div class={brand}>
          <button
            type="button"
            class={wordmark}
            // Empty mode: nothing to reset. Disabling removes the
            // hover affordance and stops keyboard focus from
            // landing on an inert action.
            disabled={mode() === 'empty'}
            aria-label="Start over"
            title={mode() === 'empty' ? undefined : 'Start over'}
            onClick={resetToEmpty}
          >
            photo-frame
          </button>
          <span class={tagline}>Liit-style golden-ratio framing, in your browser.</span>
        </div>
        <div class={headerStatus} aria-live="polite">
          {status()}
        </div>
      </header>

      <main class={stage}>
        <Show when={mode() === 'empty'}>
          <div class={stageEmpty}>
            <DropZone onLoad={onDrop} />
          </div>
        </Show>

        <Show when={mode() === 'single'}>
          <div class={stageCanvas}>
            {/* φ:1 wrapper carves the preview into a golden
                rectangle; the contain-fit in paintPreview lets
                portrait / landscape sources letterbox inside. */}
            <div class={previewFrame}>
              <canvas ref={canvasRef} class={previewCanvas} />
            </div>
          </div>
        </Show>

        <Show when={mode() === 'batch'}>
          <div class={stageBatch}>
            <Gallery rows={batchRows()} />
          </div>
        </Show>
      </main>

      <Show when={mode() !== 'empty'}>
        <aside class={sidebar}>
          <ControlsCommon
            preset={preset()}
            onPreset={applyPreset}
            resize={resize()}
            onResize={setResize}
            resizePx={resizePx()}
            onResizePx={setResizePx}
            theme={theme()}
            onTheme={setTheme}
            layout={layout()}
            onLayout={setLayout}
            showMeta={showMeta()}
            onShowMeta={setShowMeta}
          />

          <Show when={mode() === 'single'}>
            <div class={meter}>
              <div class={meterRow}>
                <span class={meterLabel}>Estimated size</span>
                <span class={meterValue}>{formatBytes(previewSize())}</span>
              </div>
              <div class={meterRow}>
                <span class={meterLabel}>Render</span>
                <span class={meterValue}>{formatMs(previewElapsedMs())}</span>
              </div>
            </div>
            <button
              type="button"
              class={button({ intent: 'primary' })}
              disabled={busy()}
              onClick={() => void onDownloadSingle()}
            >
              {busy() ? 'Saving…' : 'Download'}
            </button>
          </Show>

          <Show when={mode() === 'batch'}>
            {/* Processing runs in the background as soon as
                files are dropped; this is just the harvest
                button. Label flips through "Download all (N/M)"
                as rows complete, becomes a plain "Download all"
                once every row's ready. */}
            <button
              type="button"
              class={button({ intent: 'primary' })}
              disabled={batchDoneCount() === 0}
              onClick={() => void onDownloadAll()}
            >
              {batchDoneCount() === batchRows().length
                ? `Download all (${batchRows().length})`
                : `Download all (${batchDoneCount()}/${batchRows().length})`}
            </button>
          </Show>

          <footer class={sidebarFooter}>
            <a href="https://github.com/P4suta/photo-frame">Source</a> ·{' '}
            <a href="https://github.com/vercel/geist-font">Geist Sans</a> (
            <a href="fonts/Geist/OFL.txt">OFL 1.1</a>)
          </footer>
        </aside>
      </Show>
    </div>
  );
};

type ControlsProps = {
  preset: PresetKey;
  onPreset: (k: PresetKey) => void;
  resize: boolean;
  onResize: (v: boolean) => void;
  resizePx: number;
  onResizePx: (n: number) => void;
  theme: FrameTheme;
  onTheme: (t: FrameTheme) => void;
  layout: CaptionLayout;
  onLayout: (l: CaptionLayout) => void;
  showMeta: boolean;
  onShowMeta: (v: boolean) => void;
};

// The Quality slider used to live here, but it was a leaky
// abstraction: changing the number didn't snap the Preset
// segmented above back to a sensible state, and a 1-100 dial
// without a live preview doesn't communicate "more / less
// quality" to anyone outside the JPEG encoding world. The
// preset names (SNS / Standard / Maximum) carry the same
// information in user-readable form, so the manual dial is
// gone — `quality` still flows through the signals via
// `applyPreset`, just not editable on its own.
const ControlsCommon = (props: ControlsProps) => (
  <div class={controls}>
    <Field label="Preset">
      <Segmented
        options={Object.entries(PRESETS).map(([key, info]) => ({
          value: key as PresetKey,
          label: info.label,
        }))}
        value={props.preset}
        onChange={props.onPreset}
        ariaLabel="Quality preset"
      />
    </Field>

    <Field label="Long edge">
      <div class={resizeRow}>
        <label class={checkInline}>
          <input
            type="checkbox"
            checked={props.resize}
            onChange={(event) => props.onResize(event.currentTarget.checked)}
          />
          <span>Cap at</span>
        </label>
        <input
          type="number"
          min={1}
          max={20000}
          step={1}
          value={props.resizePx}
          disabled={!props.resize}
          onInput={(event) => props.onResizePx(Number(event.currentTarget.value) || 1)}
        />
        <span class={suffix}>px</span>
      </div>
    </Field>

    <Field label="Background color">
      <Segmented
        options={THEMES.map((t) => ({ value: t.value, label: t.label, title: t.description }))}
        value={props.theme}
        onChange={props.onTheme}
        ariaLabel="Frame background colour"
      />
    </Field>

    {/* Caption is a single 3-state choice rather than the prior
        "Layout" picker + "Show metadata" checkbox: when there's
        no caption, the layout picker has nothing to arrange, so
        a disabled/hidden control was always going to be a kludge.
        Folding the two into one segmented makes the dependency
        explicit — `Off` is its own state, the other two imply
        "show + arrange this way". */}
    <Field label="Caption">
      <Segmented
        options={CAPTION_MODES.map((m) => ({
          value: m.value,
          label: m.label,
          title: m.description,
        }))}
        value={props.showMeta ? props.layout : 'off'}
        onChange={(v) => {
          if (v === 'off') {
            props.onShowMeta(false);
          } else {
            props.onShowMeta(true);
            props.onLayout(v);
          }
        }}
        ariaLabel="Caption metadata"
      />
    </Field>
  </div>
);

const Field = (props: { label: string; children: unknown }) => (
  <div class={field}>
    <div class={fieldLabel}>{props.label}</div>
    <div class={fieldBody}>{props.children as never}</div>
  </div>
);

type SegmentedOption<T extends string> = { value: T; label: string; title?: string };

const Segmented = <T extends string>(props: {
  options: SegmentedOption<T>[];
  value: T;
  onChange: (v: T) => void;
  ariaLabel: string;
}) => (
  <div class={segmented} role="radiogroup" aria-label={props.ariaLabel}>
    <For each={props.options}>
      {(opt) => (
        // biome-ignore lint/a11y/useSemanticElements: segmented buttons keep custom styling; native radios would lose the cohesive look used across the sidebar.
        <button
          type="button"
          role="radio"
          aria-checked={props.value === opt.value}
          title={opt.title}
          class={segmentedButton({ active: props.value === opt.value })}
          onClick={() => props.onChange(opt.value)}
        >
          {opt.label}
        </button>
      )}
    </For>
  </div>
);

function framedName(original: string): string {
  const dot = original.lastIndexOf('.');
  const stem = dot >= 0 ? original.slice(0, dot) : original;
  return `${stem}_framed.jpg`;
}

function triggerDownload(blob: Blob, name: string): void {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = name;
  anchor.click();
  URL.revokeObjectURL(url);
}

function uint8ToBuffer(u8: Uint8Array): ArrayBuffer {
  // TS's Blob constructor rejects Uint8Array<ArrayBufferLike> directly;
  // copy into a fresh ArrayBuffer so the type is unambiguous.
  const buffer = new ArrayBuffer(u8.byteLength);
  new Uint8Array(buffer).set(u8);
  return buffer;
}

function stringifyError(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}

function formatBytes(n: number | null): string {
  if (n === null) return '—';
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

function formatMs(n: number | null): string {
  return n === null ? '—' : `${n} ms`;
}
