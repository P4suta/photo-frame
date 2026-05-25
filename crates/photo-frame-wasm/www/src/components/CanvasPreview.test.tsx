// @vitest-environment jsdom

import { render } from '@solidjs/testing-library';
import { createSignal } from 'solid-js';
import { describe, expect, test, vi } from 'vitest';
import type { PreparedPixels } from '../frame-client';
import { CanvasPreview } from './CanvasPreview';

const px = (label: number, width = 4, height = 4): PreparedPixels => ({
  rgba: new Uint8Array(width * height * 4).fill(label),
  width,
  height,
});

describe('<CanvasPreview>', () => {
  test('hides the frame when frameSize is null', () => {
    const { container } = render(() => (
      <CanvasPreview pixels={() => null} frameSize={() => null} />
    ));
    const frame = container.firstElementChild as HTMLElement | null;
    expect(frame).not.toBeNull();
    expect(frame?.style.visibility).toBe('hidden');
  });

  test('applies inline width/height from frameSize', () => {
    const { container } = render(() => (
      <CanvasPreview pixels={() => null} frameSize={() => ({ width: '320px', height: '180px' })} />
    ));
    const frame = container.firstElementChild as HTMLElement;
    expect(frame.style.width).toBe('320px');
    expect(frame.style.height).toBe('180px');
  });

  test('paint effect fires when pixels become available', async () => {
    // jsdom doesn't lay out elements (clientWidth/Height = 0), so
    // computePaintLayout returns null and the paint function bails
    // before reaching drawImage. We can still pin "the effect ran"
    // by spying on getContext — that's called inside paint as the
    // first DOM-touching step and gets invoked once per pixel
    // change. Deeper paint geometry is tested in
    // `lib/paint-preview.test.ts`.
    const getContext = vi.fn(() => ({
      setTransform: vi.fn(),
      drawImage: vi.fn(),
      putImageData: vi.fn(),
      clearRect: vi.fn(),
      imageSmoothingEnabled: false,
      imageSmoothingQuality: 'low' as ImageSmoothingQuality,
    }));
    const originalGetContext = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = getContext as unknown as typeof originalGetContext;

    try {
      const [pixels, setPixels] = createSignal<PreparedPixels | null>(null);
      render(() => (
        <CanvasPreview pixels={pixels} frameSize={() => ({ width: '40px', height: '30px' })} />
      ));
      const callsBefore = getContext.mock.calls.length;
      setPixels(px(128, 8, 6));
      await Promise.resolve();
      // After pixels become non-null, the effect calls
      // canvas.getContext('2d') at least once more than baseline.
      expect(getContext.mock.calls.length).toBeGreaterThan(callsBefore);
    } finally {
      HTMLCanvasElement.prototype.getContext = originalGetContext;
    }
  });
});
