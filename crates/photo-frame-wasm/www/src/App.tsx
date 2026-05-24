import { For, Show, createEffect, createMemo, createSignal, onCleanup } from 'solid-js';
import { DropZone, type DroppedFile } from './DropZone';
import {
  type CaptionLayout,
  type FrameOptions,
  type FrameTheme,
  frameImage,
} from './frame-client';
import type {
  BatchItem,
  BatchResult,
  WorkerReply,
  WorkerRequest,
} from './frame-worker';

const PREVIEW_LONG_EDGE = 1600;

const THEMES = [
  { value: 'paper' as const, label: 'Paper', description: 'White frame, dark text' },
  { value: 'ink' as const, label: 'Ink', description: 'Soft-black frame, light text' },
] satisfies ReadonlyArray<{ value: FrameTheme; label: string; description: string }>;

const LAYOUTS = [
  { value: 'edges' as const, label: 'Edges', description: 'Four-corner liit-style layout' },
  { value: 'centered' as const, label: 'Centered', description: 'Both rows centred under the photo' },
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

export const App = () => {
  const [single, setSingle] = createSignal<DroppedFile | null>(null);
  const [previewUrl, setPreviewUrl] = createSignal<string | null>(null);
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

  // Worker is created lazily on the first batch run so the single-file
  // preview path doesn't pay the cost of an extra WASM module init.
  let worker: Worker | null = null;
  const ensureWorker = (): Worker => {
    worker ??= new Worker(new URL('./frame-worker.ts', import.meta.url), { type: 'module' });
    return worker;
  };

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

  const buildOptions = (maxLongEdge: number | null): FrameOptions => ({
    jpeg_quality: quality(),
    theme: theme(),
    layout: layout(),
    show_meta: showMeta(),
    max_long_edge: maxLongEdge,
  });

  const onDrop = (files: DroppedFile[]): void => {
    revokeAllBatchUrls();
    const [first] = files;
    if (files.length === 1 && first) {
      setBatchFiles(null);
      setBatchRows([]);
      setSingle(first);
    } else {
      setSingle(null);
      const prev = previewUrl();
      if (prev) {
        URL.revokeObjectURL(prev);
        setPreviewUrl(null);
      }
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

  // Single-image preview path. Touches every signal it depends on so
  // Solid re-runs the effect when controls change.
  createEffect(() => {
    const current = single();
    if (!current) return;
    const options = buildOptions(Math.min(PREVIEW_LONG_EDGE, effectiveMaxLongEdge() ?? Infinity));
    void renderPreview(current, options);
  });

  const renderPreview = async (current: DroppedFile, options: FrameOptions): Promise<void> => {
    setStatus('Framing preview…');
    const started = performance.now();
    try {
      const blob = await frameImage(current.data, options);
      const url = URL.createObjectURL(blob);
      const prev = previewUrl();
      if (prev) URL.revokeObjectURL(prev);
      setPreviewUrl(url);
      setStatus(`Preview rendered in ${Math.round(performance.now() - started)} ms`);
    } catch (error) {
      setStatus(`Error: ${stringifyError(error)}`);
    }
  };

  onCleanup(() => {
    const url = previewUrl();
    if (url) URL.revokeObjectURL(url);
    revokeAllBatchUrls();
    worker?.terminate();
  });

  const onDownloadSingle = async (): Promise<void> => {
    const current = single();
    if (!current) return;
    setBusy(true);
    setStatus('Framing at full resolution…');
    const started = performance.now();
    try {
      const blob = await frameImage(current.data, buildOptions(effectiveMaxLongEdge()));
      triggerDownload(blob, framedName(current.name));
      setStatus(`Saved in ${Math.round(performance.now() - started)} ms`);
    } catch (error) {
      setStatus(`Error: ${stringifyError(error)}`);
    } finally {
      setBusy(false);
    }
  };

  const onProcessBatch = (): void => {
    const files = batchFiles();
    if (!files || batchBusy()) return;
    setBatchBusy(true);
    setStatus(`Processing ${files.length} files in the background…`);
    const w = ensureWorker();
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
      } else {
        setStatus(`Batch failed: ${msg.message}`);
        setBatchRows((rows) => rows.map((r) => ({ ...r, status: 'error', message: msg.message })));
        setBatchBusy(false);
        w.removeEventListener('message', handle);
      }
    };
    w.addEventListener('message', handle);
    const items: BatchItem[] = files.map((f) => ({ key: f.name, bytes: f.data }));
    const request: WorkerRequest = {
      kind: 'batch',
      items,
      options: buildOptions(effectiveMaxLongEdge()),
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

  const isBatch = createMemo(() => batchFiles() !== null);

  return (
    <main>
      <header>
        <h1>photo-frame</h1>
        <p class="subtitle">Liit-style golden-ratio framing — fully in your browser.</p>
      </header>

      <DropZone onLoad={onDrop} />

      <Show when={previewUrl()}>
        {(url) => (
          <section id="preview">
            <img id="preview-image" src={url()} alt="Framed preview" />
            <div class="controls">
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
              <div class="control-row download-row">
                <button
                  type="button"
                  class="primary"
                  disabled={busy()}
                  onClick={() => void onDownloadSingle()}
                >
                  Download
                </button>
              </div>
            </div>
            <p class="status">{status()}</p>
          </section>
        )}
      </Show>

      <Show when={isBatch()}>
        <section id="batch">
          <div class="controls">
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
            <div class="control-row download-row">
              <button
                type="button"
                class="primary"
                disabled={batchBusy()}
                onClick={onProcessBatch}
              >
                {batchBusy() ? 'Processing…' : `Process ${batchRows().length} files`}
              </button>
            </div>
          </div>
          <ul class="batch-list">
            <For each={batchRows()}>
              {(row) => (
                <li classList={{ row: true, [row.status]: true }}>
                  <span class="batch-name">{row.name}</span>
                  <span class="batch-status">{row.status}</span>
                  <span class="batch-meta">{row.message ?? ''}</span>
                  <Show when={row.status === 'done'}>
                    <button type="button" onClick={() => onDownloadRow(row)}>
                      Download
                    </button>
                  </Show>
                </li>
              )}
            </For>
          </ul>
          <p class="status">{status()}</p>
        </section>
      </Show>

      <footer>
        Source on <a href="https://github.com/P4suta/photo-frame">GitHub</a> · Bundled font:{' '}
        <a href="https://github.com/vercel/geist-font">Geist Sans</a> by Vercel × basement.studio (
        <a href="fonts/Geist/OFL.txt">OFL 1.1</a>)
      </footer>
    </main>
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

/**
 * Shared control panel between the single-preview and batch modes.
 * Lifting it out keeps the JSX tree readable and ensures the two
 * surfaces stay visually identical.
 */
const ControlsCommon = (props: ControlsProps) => (
  <>
    <div class="control-row">
      <span class="control-label">Preset</span>
      <div class="segmented" role="radiogroup" aria-label="Quality preset">
        <For each={Object.entries(PRESETS) as [PresetKey, (typeof PRESETS)[PresetKey]][]}>
          {([key, info]) => (
            <>
              {/* biome-ignore lint/a11y/useSemanticElements: segmented-button radiogroup keeps custom styling; replacing with <input type=radio> would lose the styled-button visuals. */}
              <button
                type="button"
                role="radio"
                aria-checked={props.preset === key}
                classList={{ active: props.preset === key }}
                onClick={() => props.onPreset(key)}
              >
                {info.label}
              </button>
            </>
          )}
        </For>
      </div>
    </div>

    <label class="control-row">
      <span class="control-label">Quality</span>
      <input
        type="range"
        min={1}
        max={100}
        value={props.quality}
        onInput={(event) => props.onQuality(Number(event.currentTarget.value))}
      />
      <output>{props.quality}</output>
    </label>

    <div class="control-row">
      <label class="control-label">
        <input
          type="checkbox"
          checked={props.resize}
          onChange={(event) => props.onResize(event.currentTarget.checked)}
        />
        Resize long edge to
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
      <span class="control-suffix">px</span>
    </div>

    <div class="control-row">
      <span class="control-label">Theme</span>
      <div class="segmented" role="radiogroup" aria-label="Frame theme">
        <For each={THEMES}>
          {(t) => (
            <>
              {/* biome-ignore lint/a11y/useSemanticElements: matches the Preset segmented control above; keeps a single visual idiom for grouped option pickers. */}
              <button
                type="button"
                role="radio"
                aria-checked={props.theme === t.value}
                title={t.description}
                classList={{ active: props.theme === t.value }}
                onClick={() => props.onTheme(t.value)}
              >
                {t.label}
              </button>
            </>
          )}
        </For>
      </div>
    </div>

    <div class="control-row">
      <span class="control-label">Layout</span>
      <div class="segmented" role="radiogroup" aria-label="Caption layout">
        <For each={LAYOUTS}>
          {(l) => (
            <>
              {/* biome-ignore lint/a11y/useSemanticElements: matches the Theme segmented control above. */}
              <button
                type="button"
                role="radio"
                aria-checked={props.layout === l.value}
                title={l.description}
                classList={{ active: props.layout === l.value }}
                onClick={() => props.onLayout(l.value)}
              >
                {l.label}
              </button>
            </>
          )}
        </For>
      </div>
    </div>

    <label class="control-row">
      <span class="control-label">
        <input
          type="checkbox"
          checked={props.showMeta}
          onChange={(event) => props.onShowMeta(event.currentTarget.checked)}
        />
        Show metadata
      </span>
    </label>
  </>
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
