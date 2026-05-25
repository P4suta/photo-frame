/**
 * Canvas-backed preview component.
 *
 * Renders the `<div class={previewFrame}><canvas/></div>` shell
 * with two effects pinning the canvas to the live preview state:
 *
 *   - a pixel-change effect that paints + crossfades via the View
 *     Transitions API on every `pixels()` update;
 *   - a canvas `ResizeObserver` that direct-repaints (no crossfade)
 *     when only the canvas's CSS box changes — wrapping a window-
 *     resize tick in `startViewTransition` would "ghost" the canvas
 *     during the drag.
 *
 * The pure paint geometry (DPR clamp, contain-fit dest rect) lives
 * in `lib/paint-preview.ts` so the edge cases are pinned by
 * fast-check; only the DOM-mutation half (offscreen canvas,
 * `putImageData`, `drawImage`) is in here.
 */

import { type Accessor, createEffect, type JSX, onCleanup, onMount } from 'solid-js';
import type { PreparedPixels } from '../frame-client';
import { computePaintLayout } from '../lib/paint-preview';
import { previewCanvas, previewFrame } from './CanvasPreview.styles';

type Props = {
  pixels: Accessor<PreparedPixels | null>;
  /** Inline `width` / `height` for the wrapper, or null until the
   *  stage is measured and the source aspect is known. */
  frameSize: Accessor<{ width: string; height: string } | null>;
};

// View Transitions API wrapper — Chrome 111+/Safari 18+/Firefox 132+
// (~Nov 2024) implement `startViewTransition`. The browser takes a
// screenshot of the marked element (= the preview canvas, via
// `view-transition-name: preview-canvas` in the styles below),
// runs the synchronous DOM mutation in `cb`, then GPU-crossfades
// the screenshot into the post-mutation state. The fallback for
// older engines is the hard cut we had before — which, combined
// with the stale-while-revalidate variant cache, still reads as
// smooth, just without the fade.
const withViewTransition = (cb: () => void): void => {
  const docVT = document as Document & {
    startViewTransition?: (callback: () => void) => unknown;
  };
  if (typeof docVT.startViewTransition === 'function') {
    docVT.startViewTransition(cb);
  } else {
    cb();
  }
};

const paint = (canvas: HTMLCanvasElement, pixels: PreparedPixels): void => {
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  const layout = computePaintLayout({
    pixels: { width: pixels.width, height: pixels.height },
    cssW: canvas.clientWidth,
    cssH: canvas.clientHeight,
    rawDpr: window.devicePixelRatio,
  });
  if (!layout) return;
  canvas.width = layout.canvasW;
  canvas.height = layout.canvasH;
  // Author in CSS pixels — the DPR ride is on the transform so the
  // contain-fit maths in `computePaintLayout` works in container
  // units.
  ctx.setTransform(layout.dpr, 0, 0, layout.dpr, 0, 0);
  // Browsers default `imageSmoothingQuality` to `low` (= cheap
  // bilinear); `high` engages the proper Lanczos-class resampler
  // so a 3200 px source scales down to a 1500 px canvas without
  // the stippled-edge "jaggy" look.
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = 'high';
  ctx.clearRect(0, 0, canvas.clientWidth, canvas.clientHeight);

  // Phase F3-lite — zero-copy view onto the cached RGBA bytes (the
  // WASM-returned `Uint8Array::new_with_length` buffer is a regular
  // ArrayBuffer, never SharedArrayBuffer, so the ImageData spec
  // accepts it without a memcpy).
  const view = new Uint8ClampedArray(
    pixels.rgba.buffer as ArrayBuffer,
    pixels.rgba.byteOffset,
    pixels.rgba.byteLength,
  );
  // Stage the RGBA into an offscreen canvas so `drawImage` can
  // letterbox it. `putImageData` doesn't honour destination
  // rectangles, so going through an offscreen is the minimal way
  // to compose put + scale in one pipeline.
  const off = document.createElement('canvas');
  off.width = pixels.width;
  off.height = pixels.height;
  const offCtx = off.getContext('2d');
  if (!offCtx) return;
  offCtx.putImageData(new ImageData(view, pixels.width, pixels.height), 0, 0);

  const { dx, dy, dw, dh } = layout.dest;
  ctx.drawImage(off, dx, dy, dw, dh);
};

export const CanvasPreview = (props: Props): JSX.Element => {
  let canvasRef: HTMLCanvasElement | undefined;

  // Pixel-change paint: every `pixels()` update goes through the
  // View Transitions crossfade.
  createEffect(() => {
    const px = props.pixels();
    if (!canvasRef || !px) return;
    const canvas = canvasRef;
    withViewTransition(() => paint(canvas, px));
  });

  // Size-only re-paint: direct, no crossfade. Wrapping these in
  // `startViewTransition` would "ghost" the canvas while the user
  // drags the window edge.
  onMount(() => {
    if (!canvasRef) return;
    const canvas = canvasRef;
    const ro = new ResizeObserver(() => {
      const px = props.pixels();
      if (px) paint(canvas, px);
    });
    ro.observe(canvas);
    onCleanup(() => ro.disconnect());
  });

  return (
    <div
      class={previewFrame}
      style={
        props.frameSize() ?? {
          width: '0',
          height: '0',
          visibility: 'hidden',
        }
      }
    >
      <canvas ref={canvasRef} class={previewCanvas} />
    </div>
  );
};
