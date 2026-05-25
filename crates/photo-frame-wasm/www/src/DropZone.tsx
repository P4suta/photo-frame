import { createSignal } from 'solid-js';
import { css } from '../styled-system/css';
import { dropZone } from '../styled-system/recipes';

export type DroppedFile = {
  data: Uint8Array;
  name: string;
  /** Source long-edge length in pixels — sniffed once on drop
   *  via `createImageBitmap` so the Long-edge picker can grey
   *  out caps the source can't reach. */
  longEdge: number;
};

type Props = {
  /**
   * Called with every file the user dropped or selected. Files arrive in
   * one batch (preserving the order the browser provided) so the parent
   * can decide whether to handle them as a single preview or a batch.
   */
  onLoad: (files: DroppedFile[]) => void;
};

export const DropZone = (props: Props) => {
  const [over, setOver] = createSignal(false);
  let fileInput: HTMLInputElement | undefined;

  const ingest = async (files: FileList | null): Promise<void> => {
    if (!files || files.length === 0) return;
    const loaded: DroppedFile[] = await Promise.all(
      Array.from(files).map(async (file) => {
        const data = new Uint8Array(await file.arrayBuffer());
        // createImageBitmap is fast — the browser parses just
        // the JPEG/PNG header to learn dimensions, no full
        // decode. We `close()` immediately afterwards so the
        // bitmap memory doesn't linger.
        const bitmap = await createImageBitmap(new Blob([data]));
        const longEdge = Math.max(bitmap.width, bitmap.height);
        bitmap.close();
        return { data, name: file.name, longEdge };
      }),
    );
    props.onLoad(loaded);
  };

  const openPicker = (): void => fileInput?.click();

  return (
    <button
      type="button"
      id="drop-zone"
      class={dropZone({ over: over() })}
      aria-label="Drop images here or press Enter to open the file picker"
      onClick={openPicker}
      onDragOver={(event) => {
        event.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(event) => {
        event.preventDefault();
        setOver(false);
        void ingest(event.dataTransfer?.files ?? null);
      }}
    >
      <p class={paragraphCss}>
        Drop one or many JPEG/PNG images here, or{' '}
        <span class={linkCss}>
          browse
          <input
            ref={fileInput}
            type="file"
            accept="image/jpeg,image/png"
            multiple
            hidden
            onChange={(event) => {
              void ingest(event.currentTarget.files);
            }}
          />
        </span>
        .
      </p>
    </button>
  );
};

const paragraphCss = css({ margin: '0' });

const linkCss = css({
  color: 'fg.default',
  cursor: 'pointer',
  textDecoration: 'underline',
  textUnderlineOffset: '2px',
});
