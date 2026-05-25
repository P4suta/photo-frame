import { For, type JSX } from 'solid-js';
import {
  gallery,
  galleryCard,
  galleryCardStatus,
  galleryFooter,
  galleryName,
  galleryStatus,
  galleryThumb,
  galleryThumbImg,
  galleryThumbPlaceholder,
} from './Gallery.styles';
import { statusLabel } from './lib/format';

/**
 * Per-row state the gallery renders. Mirrors `BatchRow` in
 * `App.tsx` but is parameterised here so this component stays
 * unaware of the larger session lifecycle (single mode, status
 * line, etc.).
 *
 * `thumbnailUrl` is the low-resolution framed preview shown
 * while the row sits queued or processing. `resultUrl` is the
 * full-resolution framed JPEG used by the sidebar's "Download
 * all" affordance; the gallery itself is passive — display
 * only, no per-card download button.
 */
export type GalleryRow = {
  key: string;
  name: string;
  status: 'queued' | 'processing' | 'done' | 'error';
  thumbnailUrl?: string;
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
            {row.thumbnailUrl ? (
              <img class={galleryThumbImg} src={row.thumbnailUrl} alt="" />
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
        </li>
      )}
    </For>
  </ul>
);
