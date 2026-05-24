import { createSignal } from 'solid-js';

export type DroppedFile = {
  data: Uint8Array;
  name: string;
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
      Array.from(files).map(async (file) => ({
        data: new Uint8Array(await file.arrayBuffer()),
        name: file.name,
      })),
    );
    props.onLoad(loaded);
  };

  const openPicker = (): void => fileInput?.click();

  return (
    <button
      type="button"
      id="drop-zone"
      class="drop"
      classList={{ over: over() }}
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
      <p>
        Drop one or many JPEG/PNG images here, or{' '}
        <span class="link">
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
