import { css } from '../../styled-system/css';

// The actual aspect-shaped frame the preview canvas sits in.
// Wraps `<canvas>` so the canvas resolves `width / height: 100%`
// against a definite block.
//
// `width` / `height` are set inline by the owning session from a JS
// contain-fit calculation against the stage's measured size. CSS
// `aspect-ratio` + `max-w/h` was unreliable here: the preview
// canvas's intrinsic size leaked back into the parent grid item's
// min-content negotiation and the wrapper would either burst the
// stage or collapse to zero depending on the source aspect. Sizing
// it from JS is boring but always lands.
export const previewFrame = css({
  display: 'flex',
  alignItems: 'stretch',
  justifyContent: 'stretch',
  borderRadius: 'phi.m3',
  boxShadow: '[0 12px 48px rgba(255, 255, 255, 0.12)]',
  _light: {
    boxShadow: '[0 12px 48px rgba(0, 0, 0, 0.18)]',
  },
});

// The drawing buffer matches the container × DPR and the image is
// letterboxed inside it (see `lib/paint-preview.ts` for the
// contain-fit math). The canvas element itself has no intrinsic
// ratio that could fight CSS layout.
export const previewCanvas = css({
  width: 'full',
  height: 'full',
  border: 'soft',
  borderRadius: 'phi.m3',
  imageRendering: 'auto',
  // Opt this canvas into the View Transitions API — the global
  // `::view-transition-old/new(preview-canvas)` rule (see
  // `panda/global-css.ts`) crossfades the canvas content on every
  // paint that runs inside `document.startViewTransition`, so
  // Preset / Resolution changes don't hard-cut. The browser
  // GPU-composites the crossfade against the screenshot of the
  // canvas's pre-paint state — no JS animation loop required.
  viewTransitionName: '[preview-canvas]',
});
