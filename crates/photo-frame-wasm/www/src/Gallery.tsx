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

export const Gallery = (props: Props): JSX.Element => (
  <ul class={gallery}>
    <For each={props.rows}>
      {(row) => {
        // Shared card body — the thumbnail block + filename + footer.
        const body = (
          <>
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
          </>
        );

        // Once the row is `done`, the whole card becomes a single
        // button — easier touch target than a small Save button
        // tucked in the footer. Until then it's a passive <li>.
        return (
          <li class={`${galleryCard} ${galleryCardStatus}`} data-status={row.status}>
            {row.status === 'done' ? (
              <button
                type="button"
                class={galleryCardButton}
                onClick={() => props.onDownload(row)}
                title={`Download ${row.name}`}
              >
                {body}
              </button>
            ) : (
              body
            )}
          </li>
        );
      }}
    </For>
  </ul>
);
