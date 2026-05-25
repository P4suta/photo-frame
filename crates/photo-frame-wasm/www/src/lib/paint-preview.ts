/**
 * Pure geometry for the preview canvas paint pass.
 *
 * `computePaintLayout` takes the source pixel dimensions, the CSS-pixel
 * box the canvas occupies, and the raw `window.devicePixelRatio`, and
 * returns every number the caller needs to drive `<canvas>` + 2D
 * context APIs: the drawing-buffer size, the clamped DPR, and the
 * contain-fit destination rect for `drawImage`.
 *
 * Splitting the math out of `paintPreview` (which still touches DOM —
 * `document.createElement`, `getContext`, `drawImage`) lets the
 * lopsided-aspect / zero-stage / DPR-clamp edge cases land under
 * fast-check pinning without spinning up a real canvas.
 */

/** Maximum DPR we'll honour. Past 2× the visual gain is invisible to
 *  most users and the GPU upload cost grows quadratically — capping
 *  here keeps a 4K-DPR phone from torching the paint budget. */
const MAX_DPR = 2;

export type PaintInput = {
  /** Source RGBA dimensions (from the WASM-prepared pixel buffer). */
  pixels: { width: number; height: number };
  /** Canvas CSS-pixel width (= `canvas.clientWidth`). */
  cssW: number;
  /** Canvas CSS-pixel height (= `canvas.clientHeight`). */
  cssH: number;
  /** Raw `window.devicePixelRatio` — clamped internally. */
  rawDpr: number;
};

export type PaintLayout = {
  /** Drawing-buffer width — feeds `canvas.width`. */
  canvasW: number;
  /** Drawing-buffer height — feeds `canvas.height`. */
  canvasH: number;
  /** Clamped DPR — feeds `ctx.setTransform(dpr, 0, 0, dpr, 0, 0)`. */
  dpr: number;
  /** Letterbox destination rect for `drawImage(src, dx, dy, dw, dh)`. */
  dest: { dx: number; dy: number; dw: number; dh: number };
};

/**
 * Returns the layout that paints `pixels` contain-fitted inside a CSS
 * box of `cssW × cssH`, or `null` when there's nothing meaningful to
 * draw (zero stage, zero source).
 *
 * The caller is responsible for the DOM mutations (resize the
 * drawing buffer, set the transform, `clearRect`, etc.); this just
 * hands back the numbers.
 */
export const computePaintLayout = (input: PaintInput): PaintLayout | null => {
  const { pixels, cssW, cssH, rawDpr } = input;
  if (cssW <= 0 || cssH <= 0) return null;
  if (pixels.width <= 0 || pixels.height <= 0) return null;

  const dpr = Math.max(1, Math.min(MAX_DPR, rawDpr || 1));
  const canvasW = Math.round(cssW * dpr);
  const canvasH = Math.round(cssH * dpr);

  const scale = Math.min(cssW / pixels.width, cssH / pixels.height);
  const dw = pixels.width * scale;
  const dh = pixels.height * scale;
  const dx = (cssW - dw) / 2;
  const dy = (cssH - dh) / 2;

  return { canvasW, canvasH, dpr, dest: { dx, dy, dw, dh } };
};
