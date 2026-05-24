import { For, Show, createEffect, createMemo, createSignal, onCleanup } from 'solid-js';
import { DropZone } from './DropZone';
import {
  type CaptionLayout,
  type FrameOptions,
  type FrameTheme,
  frameImage,
} from './frame-client';

const PREVIEW_LONG_EDGE = 1600;

/**
 * Theme options surfaced in the UI. Labels are display-only; the
 * `value` is what gets handed to the WASM bridge — keep these labels
 * in sync with `FrameTheme::label()` in
 * `crates/photo-frame-frame/src/options.rs`.
 */
const THEMES = [
  { value: 'paper' as const, label: 'Paper', description: 'White frame, dark text' },
  { value: 'ink' as const, label: 'Ink', description: 'Soft-black frame, light text' },
] satisfies ReadonlyArray<{ value: FrameTheme; label: string; description: string }>;

const LAYOUTS = [
  { value: 'edges' as const, label: 'Edges', description: 'Four-corner liit-style layout' },
  { value: 'centered' as const, label: 'Centered', description: 'Both rows centred under the photo' },
] satisfies ReadonlyArray<{ value: CaptionLayout; label: string; description: string }>;

/**
 * Mirror of `photo_frame_core::QualityPreset` — keep in sync with
 * `crates/photo-frame-core/src/options.rs::QualityPreset`. The Rust side
 * is the source of truth; the duplication here keeps the UI snappy
 * without a WASM round-trip for every preset click.
 */
const PRESETS = {
  sns: { label: 'SNS', quality: 78, maxLongEdge: 2048 as number | null },
  standard: { label: 'Standard', quality: 92, maxLongEdge: null as number | null },
  maximum: { label: 'Maximum', quality: 98, maxLongEdge: null as number | null },
} as const satisfies Record<string, { label: string; quality: number; maxLongEdge: number | null }>;

type PresetKey = keyof typeof PRESETS;

type Loaded = {
  data: Uint8Array;
  name: string;
};

export const App = () => {
  const [loaded, setLoaded] = createSignal<Loaded | null>(null);
  const [previewUrl, setPreviewUrl] = createSignal<string | null>(null);
  const [preset, setPreset] = createSignal<PresetKey>('standard');
  const [quality, setQuality] = createSignal<number>(PRESETS.standard.quality);
  const [resize, setResize] = createSignal(false);
  const [resizePx, setResizePx] = createSignal<number>(2048);
  const [theme, setTheme] = createSignal<FrameTheme>('paper');
  const [layout, setLayout] = createSignal<CaptionLayout>('edges');
  const [showMeta, setShowMeta] = createSignal(true);
  const [status, setStatus] = createSignal('');
  const [busy, setBusy] = createSignal(false);

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

  // Re-frame the preview whenever any input changes. Touching the signals
  // here registers them with Solid's dependency tracker; the effect re-runs
  // only when one of those reads actually changes.
  createEffect(() => {
    const current = loaded();
    if (!current) return;
    const options = buildOptions(Math.min(PREVIEW_LONG_EDGE, effectiveMaxLongEdge() ?? Infinity));
    void renderPreview(current, options);
  });

  const renderPreview = async (current: Loaded, options: FrameOptions): Promise<void> => {
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
  });

  const onDownload = async (): Promise<void> => {
    const current = loaded();
    if (!current) return;
    setBusy(true);
    setStatus('Framing at full resolution…');
    const started = performance.now();
    try {
      const blob = await frameImage(current.data, buildOptions(effectiveMaxLongEdge()));
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = framedName(current.name);
      anchor.click();
      URL.revokeObjectURL(url);
      setStatus(`Saved in ${Math.round(performance.now() - started)} ms`);
    } catch (error) {
      setStatus(`Error: ${stringifyError(error)}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <main>
      <header>
        <h1>photo-frame</h1>
        <p class="subtitle">Liit-style golden-ratio framing — fully in your browser.</p>
      </header>

      <DropZone onLoad={(data, name) => setLoaded({ data, name })} />

      <Show when={previewUrl()}>
        {(url) => (
          <section id="preview">
            <img id="preview-image" src={url()} alt="Framed preview" />
            <div class="controls">
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
                          aria-checked={preset() === key}
                          classList={{ active: preset() === key }}
                          onClick={() => applyPreset(key)}
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
                  value={quality()}
                  onInput={(event) => setQuality(Number(event.currentTarget.value))}
                />
                <output>{quality()}</output>
              </label>

              <div class="control-row">
                <label class="control-label">
                  <input
                    type="checkbox"
                    checked={resize()}
                    onChange={(event) => setResize(event.currentTarget.checked)}
                  />
                  Resize long edge to
                </label>
                <input
                  type="number"
                  min={1}
                  max={20000}
                  step={1}
                  value={resizePx()}
                  disabled={!resize()}
                  onInput={(event) => setResizePx(Number(event.currentTarget.value) || 1)}
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
                          aria-checked={theme() === t.value}
                          title={t.description}
                          classList={{ active: theme() === t.value }}
                          onClick={() => setTheme(t.value)}
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
                          aria-checked={layout() === l.value}
                          title={l.description}
                          classList={{ active: layout() === l.value }}
                          onClick={() => setLayout(l.value)}
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
                    checked={showMeta()}
                    onChange={(event) => setShowMeta(event.currentTarget.checked)}
                  />
                  Show metadata
                </span>
              </label>

              <div class="control-row download-row">
                <button
                  type="button"
                  class="primary"
                  disabled={busy()}
                  onClick={() => void onDownload()}
                >
                  Download
                </button>
              </div>
            </div>
            <p class="status">{status()}</p>
          </section>
        )}
      </Show>

      <footer>
        Source on <a href="https://github.com/yasunobu/photo-frame">GitHub</a> · Bundled font:{' '}
        <a href="https://github.com/vercel/geist-font">Geist Sans</a> by Vercel × basement.studio (
        <a href="fonts/Geist/OFL.txt">OFL 1.1</a>)
      </footer>
    </main>
  );
};

function framedName(original: string): string {
  const dot = original.lastIndexOf('.');
  const stem = dot >= 0 ? original.slice(0, dot) : original;
  return `${stem}_framed.jpg`;
}

function stringifyError(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}
