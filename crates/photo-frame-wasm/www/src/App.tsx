import { createMemo, createSignal, onCleanup, Show } from 'solid-js';
import { appShell, button } from '../styled-system/recipes';
import { sidebar, sidebarFooter, stage, stageBatch, stageCanvas, stageEmpty } from './App.styles';
import { AppHeader } from './components/AppHeader';
import { CanvasPreview } from './components/CanvasPreview';
import { SidebarControls } from './components/SidebarControls';
import { type DroppedFile, DropZone } from './DropZone';
import { disposeWorker, getWorker } from './frame-client';
import { Gallery } from './Gallery';
import { sourceLongEdgeOf } from './lib/long-edge';
import { createAppSettings } from './state/app-settings';
import { createBatchSession } from './state/batch-session';
import { createPreviewSession } from './state/preview-session';

type Mode = 'empty' | 'single' | 'batch';

export const App = () => {
  // ── session source (inline — small, App-only) ─────────────────
  // The three signals below define which mode the shell is in: a
  // single image, a batch, or empty. Everything downstream (sessions,
  // sidebar, header) derives from these.
  const [single, setSingle] = createSignal<DroppedFile | null>(null);
  const [batchFiles, setBatchFiles] = createSignal<DroppedFile[] | null>(null);
  const [status, setStatus] = createSignal('');

  const mode = createMemo<Mode>(() =>
    batchFiles() !== null ? 'batch' : single() !== null ? 'single' : 'empty',
  );

  // Source long-edge (min across the batch, or the single image's).
  // Fed into settings for auto-demote and into SidebarControls for
  // the Long-edge picker's oversize warnings.
  const sourceLongEdge = createMemo<number | null>(() => sourceLongEdgeOf(single(), batchFiles()));

  // ── settings + sessions ───────────────────────────────────────
  const settings = createAppSettings({ sourceLongEdge });

  const preview = createPreviewSession({
    source: single,
    settings: settings.state,
    setStatus,
  });

  const batch = createBatchSession({
    files: batchFiles,
    settings: settings.state,
    setStatus,
    workerTarget: getWorker(),
  });

  // ── session transitions ───────────────────────────────────────
  const clearSession = (): void => {
    preview.dispose();
    batch.dispose();
    setStatus('');
    setSingle(null);
    setBatchFiles(null);
  };

  const onDrop = (files: DroppedFile[]): void => {
    clearSession();
    const [first] = files;
    if (files.length === 1 && first) {
      setSingle(first);
    } else {
      setBatchFiles(files);
    }
  };

  // The shared worker is process-global — disposed once at app
  // unmount. Sessions clean up their own listeners/gates via
  // `dispose()`; we route through them here for symmetry.
  onCleanup(() => {
    preview.dispose();
    batch.dispose();
    disposeWorker();
  });

  return (
    <div class={appShell({ mode: mode() })}>
      <AppHeader
        status={status}
        disabled={() => mode() === 'empty'}
        onResetToEmpty={clearSession}
      />

      <main class={stage}>
        <Show when={mode() === 'empty'}>
          <div class={stageEmpty}>
            <DropZone onLoad={onDrop} />
          </div>
        </Show>

        <Show when={mode() === 'single'}>
          <div class={stageCanvas} ref={preview.actions.setStageEl}>
            <CanvasPreview pixels={preview.state.pixels} frameSize={preview.state.frameSize} />
          </div>
        </Show>

        <Show when={mode() === 'batch'}>
          <div class={stageBatch}>
            <Gallery rows={batch.state.rows()} />
          </div>
        </Show>
      </main>

      <Show when={mode() !== 'empty'}>
        <aside class={sidebar}>
          <SidebarControls settings={settings} sourceLongEdge={sourceLongEdge} />

          <Show when={mode() === 'single'}>
            <button
              type="button"
              class={button({ intent: 'primary' })}
              disabled={preview.state.busy()}
              onClick={() => void preview.actions.onDownload()}
            >
              {preview.state.busy() ? 'Saving…' : 'Download'}
            </button>
          </Show>

          <Show when={mode() === 'batch'}>
            {/* Processing runs in the background as soon as files
                are dropped (see `state/batch-session.ts`); this is
                just the harvest button. Label flips through
                "Download all (N/M)" as rows complete, becomes
                "Download all (M)" once every row is ready. */}
            <button
              type="button"
              class={button({ intent: 'primary' })}
              disabled={batch.state.doneCount() === 0}
              onClick={() => void batch.actions.onDownloadAll()}
            >
              {batch.state.doneCount() === batch.state.rows().length
                ? `Download all (${batch.state.rows().length})`
                : `Download all (${batch.state.doneCount()}/${batch.state.rows().length})`}
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
