import { For, type JSX } from 'solid-js';
import {
  gallery,
  galleryCard,
  galleryCardButton,
  galleryCardStatus,
  galleryFooter,
  galleryName,
  galleryStatus,
  galleryThumb,
  galleryThumbImg,
  galleryThumbPlaceholder,
} from './Gallery.styles';

/**
 * Per-row state the gallery renders. Mirrors `BatchRow` in
 * `App.tsx` but is parameterised here so this component stays
 * unaware of the larger session lifecycle (single mode, status
 * line, etc.).
 *
 * `thumbnailUrl` is the low-resolution framed preview shown
 * while the row sits queued or processing. `resultUrl` is the
 * full-resolution framed JPEG that lets the card double as a
 * download trigger once the row reaches `done`.
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
  // Called when the user clicks a `done` card to download its JPEG.
  onDownload: (row: GalleryRow) => void;
};

// Footer status label — short caption shown under the thumbnail.
const statusLabel = (status: GalleryRow['status']): string => {
  switch (status) {
    case 'queued':
      return 'Queued';
    case 'processing':
      return 'Processing';
    case 'done':
      return '✓ Done';
    case 'error':
      return '✗ Error';
  }
};

// Button-face label — shown on the card-button itself. Reads as
// an action when the row's ready, as a status while it works.
const buttonLabel = (status: GalleryRow['status']): string => {
  switch (status) {
    case 'queued':
      return 'Queued…';
    case 'processing':
      return 'Processing…';
    case 'done':
      return 'Download';
    case 'error':
      return 'Failed';
  }
};

export const Gallery = (props: Props): JSX.Element => (
  <ul class={gallery}>
    <For each={props.rows}>
      {(row) => {
        // The whole card is always a button — disabled until the
        // background processing completes, label flips from
        // "Processing…" to "Download" the moment the row's ready.
        // This gives the user a single, predictable target whose
        // affordance state advertises the row's progress.
        const ready = (): boolean => row.status === 'done';
        return (
          <li class={`${galleryCard} ${galleryCardStatus}`} data-status={row.status}>
            <button
              type="button"
              class={galleryCardButton}
              disabled={!ready()}
              onClick={() => ready() && props.onDownload(row)}
              title={ready() ? `Download ${row.name}` : `${buttonLabel(row.status)} — ${row.name}`}
            >
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
                <span class={galleryStatus}>{buttonLabel(row.status)}</span>
              </div>
            </button>
          </li>
        );
      }}
    </For>
  </ul>
);
