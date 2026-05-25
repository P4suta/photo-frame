// @vitest-environment jsdom

import { fireEvent, render, waitFor } from '@solidjs/testing-library';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import { DropZone, type DroppedFile } from './DropZone';

// Each test gets its own createImageBitmap stub so width/height
// can be tailored to the scenario without leaking across cases.
beforeEach(() => {
  globalThis.createImageBitmap = vi.fn().mockResolvedValue({
    width: 100,
    height: 100,
    close: () => undefined,
  }) as unknown as typeof createImageBitmap;
});

const makeFile = (name: string, bytes = new Uint8Array([1, 2, 3])): File => {
  // Polyfill-friendly: jsdom's File supports the constructor
  // (bits, name) form. The bytes content is irrelevant to the
  // test — createImageBitmap is stubbed.
  return new File([bytes], name, { type: 'image/jpeg' });
};

const dropFiles = (target: HTMLElement, files: File[]): void => {
  const dataTransfer = {
    files,
  } as unknown as DataTransfer;
  fireEvent.drop(target, { dataTransfer });
};

describe('<DropZone>', () => {
  test('invokes onLoad with the dropped files preserved in order', async () => {
    const onLoad = vi.fn<(files: DroppedFile[]) => void>();
    const { getByRole } = render(() => <DropZone onLoad={onLoad} />);
    const zone = getByRole('button', { name: /drop images here/i });
    dropFiles(zone, [makeFile('a.jpg'), makeFile('b.jpg'), makeFile('c.jpg')]);
    await waitFor(() => expect(onLoad).toHaveBeenCalledTimes(1));
    const files = onLoad.mock.calls[0]?.[0];
    expect(files?.map((f) => f.name)).toEqual(['a.jpg', 'b.jpg', 'c.jpg']);
  });

  test('records longEdge = max(width, height) from createImageBitmap', async () => {
    // Mirror a real Z-series export: portrait 4000×6000 → longEdge 6000.
    (globalThis.createImageBitmap as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
      width: 4000,
      height: 6000,
      close: () => undefined,
    });
    const onLoad = vi.fn<(files: DroppedFile[]) => void>();
    const { getByRole } = render(() => <DropZone onLoad={onLoad} />);
    dropFiles(getByRole('button', { name: /drop images here/i }), [makeFile('portrait.jpg')]);
    await waitFor(() => expect(onLoad).toHaveBeenCalledTimes(1));
    expect(onLoad.mock.calls[0]?.[0]?.[0]?.longEdge).toBe(6000);
  });

  test('also takes the landscape long edge correctly (width-side max)', async () => {
    (globalThis.createImageBitmap as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
      width: 6048,
      height: 4032,
      close: () => undefined,
    });
    const onLoad = vi.fn<(files: DroppedFile[]) => void>();
    const { getByRole } = render(() => <DropZone onLoad={onLoad} />);
    dropFiles(getByRole('button', { name: /drop images here/i }), [makeFile('landscape.jpg')]);
    await waitFor(() => expect(onLoad).toHaveBeenCalledTimes(1));
    expect(onLoad.mock.calls[0]?.[0]?.[0]?.longEdge).toBe(6048);
  });

  test('a drop with no files is a no-op (does not call onLoad)', async () => {
    const onLoad = vi.fn();
    const { getByRole } = render(() => <DropZone onLoad={onLoad} />);
    // An empty FileList — the user dragged something the browser
    // didn't expose as a file (URL, text, etc.). The zone must
    // not invoke onLoad with an empty array.
    dropFiles(getByRole('button', { name: /drop images here/i }), []);
    // Give the (skipped) async ingest a tick to be sure.
    await Promise.resolve();
    expect(onLoad).not.toHaveBeenCalled();
  });

  test('dragover prevents default (so the drop target accepts the file)', () => {
    // Without preventDefault on dragover the browser navigates
    // to the dropped file's URL instead of firing the drop
    // handler — a common bug that has historically broken the
    // zone. Pin the contract.
    const onLoad = vi.fn();
    const { getByRole } = render(() => <DropZone onLoad={onLoad} />);
    const zone = getByRole('button', { name: /drop images here/i });
    const event = new Event('dragover', { bubbles: true, cancelable: true });
    zone.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
  });
});
