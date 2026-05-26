import { For, Show, type JSX } from 'solid-js';
import {
  gallery,
  galleryCard,
  galleryCardStatus,
  galleryFooter,
  galleryName,
  galleryProgressFill,
  galleryProgressTrack,
  galleryStatus,
  galleryThumb,
  galleryThumbImg,
  galleryThumbPlaceholder,
} from './Gallery.styles';
import { statusLabel } from './lib/format';

/**
 * Per-row state the gallery renders. Mirrors `BatchRow` in
 * `state/batch-session.ts` but is parameterised here so this
 * component stays unaware of the larger session lifecycle (single
 * mode, status line, the depth-2 thumbnail cache, etc.).
 *
 * `thumb.url` is the low-resolution framed preview shown while
 * the row sits queued or processing — the session owns its
 * lifetime via a per-row depth-2 LRU cache, so it's safe to assume
 * the URL stays valid for the entire interval it sits on the row.
 * `resultUrl` is the full-resolution framed JPEG used by the
 * sidebar's "Download all" affordance; the gallery itself is
 * passive — display only, no per-card download button.
 *
 * `transitionName` is the stable `view-transition-name` for this
 * row's `<img>` so the View Transitions API can match the same
 * element across renders and animate per-row crossfades
 * independently (concurrent transitions with distinct names
 * don't conflict).
 */
export type GalleryRow = {
  key: string;
  name: string;
  transitionName: string;
  status: 'queued' | 'processing' | 'done' | 'error';
  /** Cumulative per-item pipeline progress, 0..100. Drives the
   * progress bar that replaces the pulse animation while
   * `status === 'processing'`. */
  percent?: number;
  thumb?: { url: string };
  resultUrl?: string;
  message?: string;
};

type Props = {
  rows: readonly GalleryRow[];
};

// `statusLabel` lives in `lib/format.ts` and is shared between
// the gallery and any future row-status surface.

export const Gallery = (props: Props): JSX.Element => (
  <ul class={gallery}>
    <For each={props.rows}>
      {(row) => (
        <li class={`${galleryCard} ${galleryCardStatus}`} data-status={row.status}>
          <div class={`${galleryThumb} gallery-thumb`}>
            {row.thumb ? (
              // `view-transition-name` lives inline because Panda's
              // static-extraction can't templatise per-row idents.
              // Each row's `<img>` becomes its own View Transition
              // pseudo-element on the next swap, so the session's
              // depth-2 cache rotation crossfades GPU-side instead
              // of hard-cutting.
              <img
                class={galleryThumbImg}
                src={row.thumb.url}
                alt=""
                style={{ 'view-transition-name': row.transitionName }}
              />
            ) : (
              <div class={galleryThumbPlaceholder} />
            )}
          </div>
          <div class={galleryName} title={row.name}>
            {row.name}
          </div>
          <div class={galleryFooter}>
            <span class={galleryStatus}>{statusLabel(row.status)}</span>
          </div>
          <Show when={row.status === 'processing'}>
            <div
              class={galleryProgressTrack}
              role="progressbar"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={row.percent ?? 0}
            >
              <div class={galleryProgressFill} style={{ width: `${row.percent ?? 0}%` }} />
            </div>
          </Show>
        </li>
      )}
    </For>
  </ul>
);
