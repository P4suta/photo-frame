import { createEffect, createMemo, createSignal, For, on, onCleanup, Show } from 'solid-js';
import { type DroppedFile, DropZone } from './DropZone';
import {
  type BatchItem,
  type BatchResult,
  type CaptionLayout,
  disposeWorker,
  encodeJpeg,
  type FrameOptionsForPrepare,
  type FrameTheme,
  getWorker,
  type PipelineOptions,
  type PreparedPixels,
  preparePixels,
  type WorkerReply,
  type WorkerRequest,
} from './frame-client';

const PREVIEW_LONG_EDGE = 1600;
// Phase G1 — the prepare path used to debounce by 120 ms so a rapid
// preset click (which flips quality + max_long_edge together) only
// dispatched one prepare. After Phase G1's WASM decoded-photograph
// cache landed, the marginal cost of a redundant prepare is just the
// frame stage (~50 ms at preview res) — so the debounce buys nothing
// while shaving 120 ms off every theme / layout / showMeta toggle.
// 0 ms means "next tick"; the worker still serialises requests and
// `exchange()`'s request-ID filter drops stale replies.
const PREPARE_DEBOUNCE_MS = 0;
const ESTIMATE_DEBOUNCE_MS = 220;

const THEMES = [
  { value: 'paper' as const, label: 'Paper', description: 'White frame, dark text' },
  { value: 'ink' as const, label: 'Ink', description: 'Soft-black frame, light text' },
] satisfies ReadonlyArray<{ value: FrameTheme; label: string; description: string }>;

const LAYOUTS = [
  { value: 'edges' as const, label: 'Edges', description: 'Four-corner liit-style layout' },
  {
    value: 'centered' as const,
    label: 'Centered',
    description: 'Both rows centred under the photo',
  },
] satisfies ReadonlyArray<{ value: CaptionLayout; label: string; description: string }>;

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

type BatchRow = {
  key: string;
  name: string;
  status: 'queued' | 'processing' | 'done' | 'error';
  blobUrl?: string;
  message?: string;
};

type Mode = 'empty' | 'single' | 'batch';

export const App = () => {
  const [single, setSingle] = createSignal<DroppedFile | null>(null);
  const [previewPixels, setPreviewPixels] = createSignal<PreparedPixels | null>(null);
  const [previewSize, setPreviewSize] = createSignal<number | null>(null);
  const [previewElapsedMs, setPreviewElapsedMs] = createSignal<number | null>(null);
  const [batchRows, setBatchRows] = createSignal<BatchRow[]>([]);
  const [batchFiles, setBatchFiles] = createSignal<DroppedFile[] | null>(null);
  const [batchBusy, setBatchBusy] = createSignal(false);
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

  const buildPipelineOptions = (maxLongEdge: number | null): PipelineOptions => ({
    ...buildFrameOptions(maxLongEdge),
    jpeg_quality: quality(),
  });

  const onDrop = (files: DroppedFile[]): void => {
    revokeAllBatchUrls();
    setPreviewPixels(null);
    setPreviewSize(null);
    setPreviewElapsedMs(null);
    setStatus('');
    const [first] = files;
    if (files.length === 1 && first) {
      setBatchFiles(null);
      setBatchRows([]);
      setSingle(first);
    } else {
      setSingle(null);
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

  const revokeAllBatchUrls = (): void => {
    for (const row of batchRows()) {
      if (row.blobUrl) URL.revokeObjectURL(row.blobUrl);
    }
  };

  // ── prepare effect ───────────────────────────────────────────────
  // Re-runs whenever the photo or any non-quality frame option changes.
  // Debounced so chained signal updates (preset click changes quality
  // and resize simultaneously) only fire one WASM call.
  let prepareTimer: ReturnType<typeof setTimeout> | null = null;
  let preparePending: { current: DroppedFile; opts: FrameOptionsForPrepare } | null = null;
  createEffect(
    on(
      () => {
        const current = single();
        if (!current) return null;
        return {
          current,
          opts: buildFrameOptions(Math.min(PREVIEW_LONG_EDGE, effectiveMaxLongEdge() ?? Infinity)),
        };
      },
      (intent) => {
        if (!intent) return;
        preparePending = intent;
        if (prepareTimer !== null) clearTimeout(prepareTimer);
        prepareTimer = setTimeout(() => {
          prepareTimer = null;
          const pending = preparePending;
          preparePending = null;
          if (pending) void runPrepare(pending.current, pending.opts);
        }, PREPARE_DEBOUNCE_MS);
      },
    ),
  );

  const runPrepare = async (current: DroppedFile, opts: FrameOptionsForPrepare): Promise<void> => {
    setStatus('Framing preview…');
    const started = performance.now();
    try {
      const pixels = await preparePixels(current.data, opts);
      // Guard against an even-newer file being dropped mid-flight.
      if (single() !== current) return;
      setPreviewPixels(pixels);
      setPreviewElapsedMs(Math.round(performance.now() - started));
      setStatus('');
    } catch (error) {
      setStatus(`Error: ${stringifyError(error)}`);
    }
  };

  // ── draw effect ──────────────────────────────────────────────────
  // Paints the cached RGBA onto the canvas. Cheap; runs synchronously
  // whenever previewPixels updates.
  let canvasRef: HTMLCanvasElement | undefined;
  createEffect(() => {
    const pixels = previewPixels();
    if (!pixels || !canvasRef) return;
    canvasRef.width = pixels.width;
    canvasRef.height = pixels.height;
    const ctx = canvasRef.getContext('2d');
    if (!ctx) return;
    // Phase F3-lite — zero-copy view onto the cached RGBA bytes.
    // The three-arg `Uint8ClampedArray(buffer, byteOffset, length)`
    // constructor produces a view (not a copy) so the canvas read
    // amortises the WASM-returned buffer instead of paying a 24 MB
    // memcpy per render at 24 MP. Safe because:
    //  - `pixels.rgba.buffer` is a regular `ArrayBuffer` (the WASM
    //    `Uint8Array::new_with_length` constructor never returns a
    //    SharedArrayBuffer), so the ImageData spec accepts it.
    //  - The cached `pixels` signal outlives the canvas write, so
    //    the view's storage stays alive.
    // The `as ArrayBuffer` cast narrows TS's `ArrayBufferLike` to
    // the concrete type ImageData wants; runtime guard not needed
    // because the buffer's origin (`Uint8Array::new_with_length`
    // in `photo-frame-wasm`) never produces SharedArrayBuffer.
    const view = new Uint8ClampedArray(
      pixels.rgba.buffer as ArrayBuffer,
      pixels.rgba.byteOffset,
      pixels.rgba.byteLength,
    );
    ctx.putImageData(new ImageData(view, pixels.width, pixels.height), 0, 0);
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
            const jpeg = await encodeJpeg(
              intent.rgba,
              intent.width,
              intent.height,
              intent.quality,
            );
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

  onCleanup(() => {
    if (prepareTimer !== null) clearTimeout(prepareTimer);
    if (estimateTimer !== null) clearTimeout(estimateTimer);
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
  const onProcessBatch = (): void => {
    const files = batchFiles();
    if (!files || batchBusy()) return;
    setBatchBusy(true);
    setStatus(`Processing ${files.length} files in the background…`);
    const w = getWorker();
    const handle = (event: MessageEvent<WorkerReply>): void => {
      const msg = event.data;
      if (msg.kind === 'progress') {
        setBatchRows((rows) =>
          rows.map((r) => (r.key === msg.key ? { ...r, status: 'processing' } : r)),
        );
      } else if (msg.kind === 'done') {
        applyBatchResults(msg.results);
        setBatchBusy(false);
        const ok = msg.results.filter((r) => r.ok).length;
        setStatus(`Batch done: ${ok}/${msg.results.length} succeeded.`);
        w.removeEventListener('message', handle);
      } else if (msg.kind === 'error' && msg.requestId === null) {
        setStatus(`Batch failed: ${msg.message}`);
        setBatchRows((rows) => rows.map((r) => ({ ...r, status: 'error', message: msg.message })));
        setBatchBusy(false);
        w.removeEventListener('message', handle);
      }
      // Other replies (prepared/encoded/non-batch error) belong to other
      // requesters and are ignored here.
    };
    w.addEventListener('message', handle);
    const items: BatchItem[] = files.map((f) => ({ key: f.name, bytes: f.data }));
    const request: WorkerRequest = {
      kind: 'batch',
      items,
      options: buildPipelineOptions(effectiveMaxLongEdge()),
    };
    w.postMessage(request);
  };

  const applyBatchResults = (results: BatchResult[]): void => {
    setBatchRows((rows) =>
      rows.map((r) => {
        const match = results.find((res) => res.key === r.key);
        if (!match) return r;
        if (match.ok) {
          const blob = new Blob([uint8ToBuffer(match.result)], { type: 'image/jpeg' });
          const url = URL.createObjectURL(blob);
          return { ...r, status: 'done', blobUrl: url, message: `${match.elapsed_ms} ms` };
        }
        return { ...r, status: 'error', message: match.result };
      }),
    );
  };

  const onDownloadRow = (row: BatchRow): void => {
    if (!row.blobUrl) return;
    const anchor = document.createElement('a');
    anchor.href = row.blobUrl;
    anchor.download = framedName(row.name);
    anchor.click();
  };

  // ── render ───────────────────────────────────────────────────────
  return (
    <div class="app-shell" classList={{ [`mode-${mode()}`]: true }}>
      <header class="app-header">
        <div class="brand">
          <span class="wordmark">photo-frame</span>
          <span class="tagline">Liit-style golden-ratio framing, in your browser.</span>
        </div>
        <div class="header-status" aria-live="polite">
          {status()}
        </div>
      </header>

      <main class="stage">
        <Show when={mode() === 'empty'}>
          <div class="stage-empty">
            <DropZone onLoad={onDrop} />
          </div>
        </Show>

        <Show when={mode() === 'single'}>
          <div class="stage-canvas">
            <canvas ref={canvasRef} class="preview-canvas" />
          </div>
        </Show>

        <Show when={mode() === 'batch'}>
          <div class="stage-batch">
            <ul class="batch-list">
              <For each={batchRows()}>
                {(row) => (
                  <li classList={{ row: true, [row.status]: true }}>
                    <span class="batch-name">{row.name}</span>
                    <span class="batch-status">{row.status}</span>
                    <span class="batch-meta">{row.message ?? ''}</span>
                    <Show when={row.status === 'done'}>
                      <button type="button" class="ghost" onClick={() => onDownloadRow(row)}>
                        Save
                      </button>
                    </Show>
                  </li>
                )}
              </For>
            </ul>
          </div>
        </Show>
      </main>

      <Show when={mode() !== 'empty'}>
        <aside class="sidebar">
          <ControlsCommon
            preset={preset()}
            onPreset={applyPreset}
            quality={quality()}
            onQuality={setQuality}
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
            <div class="meter">
              <div class="meter-row">
                <span class="meter-label">Estimated size</span>
                <span class="meter-value">{formatBytes(previewSize())}</span>
              </div>
              <div class="meter-row">
                <span class="meter-label">Render</span>
                <span class="meter-value">{formatMs(previewElapsedMs())}</span>
              </div>
            </div>
            <button
              type="button"
              class="primary"
              disabled={busy()}
              onClick={() => void onDownloadSingle()}
            >
              {busy() ? 'Saving…' : 'Download'}
            </button>
          </Show>

          <Show when={mode() === 'batch'}>
            <button type="button" class="primary" disabled={batchBusy()} onClick={onProcessBatch}>
              {batchBusy() ? 'Processing…' : `Process ${batchRows().length} files`}
            </button>
          </Show>

          <footer class="sidebar-footer">
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
  quality: number;
  onQuality: (n: number) => void;
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

const ControlsCommon = (props: ControlsProps) => (
  <div class="controls">
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

    <Field label="Quality">
      <div class="slider-row">
        <input
          type="range"
          min={1}
          max={100}
          value={props.quality}
          onInput={(event) => props.onQuality(Number(event.currentTarget.value))}
        />
        <output class="slider-value">{props.quality}</output>
      </div>
    </Field>

    <Field label="Long edge">
      <div class="resize-row">
        <label class="check-inline">
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
        <span class="suffix">px</span>
      </div>
    </Field>

    <Field label="Theme">
      <Segmented
        options={THEMES.map((t) => ({ value: t.value, label: t.label, title: t.description }))}
        value={props.theme}
        onChange={props.onTheme}
        ariaLabel="Frame theme"
      />
    </Field>

    <Field label="Layout">
      <Segmented
        options={LAYOUTS.map((l) => ({ value: l.value, label: l.label, title: l.description }))}
        value={props.layout}
        onChange={props.onLayout}
        ariaLabel="Caption layout"
      />
    </Field>

    <Field label="Metadata">
      <label class="check-inline">
        <input
          type="checkbox"
          checked={props.showMeta}
          onChange={(event) => props.onShowMeta(event.currentTarget.checked)}
        />
        <span>Show below frame</span>
      </label>
    </Field>
  </div>
);

const Field = (props: { label: string; children: unknown }) => (
  <div class="field">
    <div class="field-label">{props.label}</div>
    <div class="field-body">{props.children as never}</div>
  </div>
);

type SegmentedOption<T extends string> = { value: T; label: string; title?: string };

const Segmented = <T extends string>(props: {
  options: SegmentedOption<T>[];
  value: T;
  onChange: (v: T) => void;
  ariaLabel: string;
}) => (
  <div class="segmented" role="radiogroup" aria-label={props.ariaLabel}>
    <For each={props.options}>
      {(opt) => (
        // biome-ignore lint/a11y/useSemanticElements: segmented buttons keep custom styling; native radios would lose the cohesive look used across the sidebar.
        <button
          type="button"
          role="radio"
          aria-checked={props.value === opt.value}
          title={opt.title}
          classList={{ active: props.value === opt.value }}
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
