import { Show, createEffect, createSignal, onCleanup } from 'solid-js';
import { DropZone } from './DropZone';
import { type FrameOptions, frameImage } from './frame-client';

const PREVIEW_LONG_EDGE = 1600;

type Loaded = {
  data: Uint8Array;
  name: string;
};

export const App = () => {
  const [loaded, setLoaded] = createSignal<Loaded | null>(null);
  const [previewUrl, setPreviewUrl] = createSignal<string | null>(null);
  const [quality, setQuality] = createSignal(92);
  const [background, setBackground] = createSignal('#ffffff');
  const [showMeta, setShowMeta] = createSignal(true);
  const [status, setStatus] = createSignal('');
  const [busy, setBusy] = createSignal(false);

  const buildOptions = (maxLongEdge: number | null): FrameOptions => {
    const [r, g, b] = parseHex(background());
    return {
      jpeg_quality: quality(),
      bg_r: r,
      bg_g: g,
      bg_b: b,
      show_meta: showMeta(),
      max_long_edge: maxLongEdge,
    };
  };

  // Re-frame the preview whenever any input changes. Touching the signals
  // here is what registers them with Solid's dependency tracker; the effect
  // re-runs only when one of those reads changes.
  createEffect(() => {
    const current = loaded();
    if (!current) return;
    const options = buildOptions(PREVIEW_LONG_EDGE);
    void renderPreview(current, options);
  });

  const renderPreview = async (
    current: Loaded,
    options: FrameOptions,
  ): Promise<void> => {
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
      const blob = await frameImage(current.data, buildOptions(null));
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
              <label>
                Quality
                <input
                  type="range"
                  min={80}
                  max={98}
                  value={quality()}
                  onInput={(event) => setQuality(Number(event.currentTarget.value))}
                />
                <output>{quality()}</output>
              </label>
              <label>
                Background
                <input
                  type="color"
                  value={background()}
                  onInput={(event) => setBackground(event.currentTarget.value)}
                />
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={showMeta()}
                  onChange={(event) => setShowMeta(event.currentTarget.checked)}
                />
                Show metadata
              </label>
              <button type="button" disabled={busy()} onClick={() => void onDownload()}>
                Download
              </button>
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

function parseHex(hex: string): [number, number, number] {
  const cleaned = hex.replace(/^#/, '');
  if (cleaned.length !== 6) return [255, 255, 255];
  return [
    Number.parseInt(cleaned.slice(0, 2), 16),
    Number.parseInt(cleaned.slice(2, 4), 16),
    Number.parseInt(cleaned.slice(4, 6), 16),
  ];
}

function framedName(original: string): string {
  const dot = original.lastIndexOf('.');
  const stem = dot >= 0 ? original.slice(0, dot) : original;
  return `${stem}_framed.jpg`;
}

function stringifyError(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}
