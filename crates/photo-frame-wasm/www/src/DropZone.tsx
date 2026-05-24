import { createSignal } from 'solid-js';

type Props = {
  onLoad: (data: Uint8Array, name: string) => void;
};

export const DropZone = (props: Props) => {
  const [over, setOver] = createSignal(false);
  let fileInput: HTMLInputElement | undefined;

  const handleFile = async (file: File): Promise<void> => {
    const buffer = await file.arrayBuffer();
    props.onLoad(new Uint8Array(buffer), file.name);
  };

  return (
    <section
      id="drop-zone"
      class="drop"
      classList={{ over: over() }}
      onClick={() => fileInput?.click()}
      onDragOver={(event) => {
        event.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(event) => {
        event.preventDefault();
        setOver(false);
        const file = event.dataTransfer?.files[0];
        if (file) void handleFile(file);
      }}
    >
      <p>
        Drop a JPEG or PNG here, or{' '}
        <span class="link">
          browse
          <input
            ref={fileInput}
            type="file"
            accept="image/jpeg,image/png"
            hidden
            onChange={(event) => {
              const file = event.currentTarget.files?.[0];
              if (file) void handleFile(file);
            }}
          />
        </span>
        .
      </p>
    </section>
  );
};
