import { css } from '../styled-system/css';

// Layout atomics for App.tsx — every spacing, font size, radius,
// and border value flows through the φ-derived design tokens
// defined in `panda/tokens.ts`. The image-renderer side derives
// every output dimension from `side · φⁿ` (see
// `crates/photo-frame-frame/src/geometry.rs`); the Web chrome
// here speaks the same vocabulary so the artifact and the UI
// that frames it share one geometric language.
//
// Multi-variant component visuals (buttons, segmented control,
// batch rows, drop zone, app shell mode) live in
// `panda/recipes.ts` instead; this file owns the one-off atomic
// layout / typography rules.

// ─── Header ──────────────────────────────────────────────────────

export const appHeader = css({
  gridArea: 'header',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  paddingX: 'phi.0',
  borderBottom: 'soft',
  background: 'bg.stage',
});

export const brand = css({
  display: 'flex',
  alignItems: 'baseline',
  gap: 'phi.m1',
  minWidth: '0',
});

// The wordmark also doubles as the "start over" button. Tag is
// `<button>` so it carries focus + activation semantics; the CSS
// below strips every default button affordance so it still reads
// purely as the brand text. `_hover` underlines for clickability
// hint; `_disabled` (empty mode) suppresses the affordance and
// returns the default cursor.
export const wordmark = css({
  appearance: 'none',
  background: 'transparent',
  border: 'none',
  padding: '0',
  margin: '0',
  font: 'inherit',
  fontWeight: 'medium',
  letterSpacing: 'wordmark',
  color: 'fg.default',
  cursor: 'pointer',
  textAlign: 'inherit',
  _hover: {
    textDecoration: 'underline',
    textUnderlineOffset: '2px',
  },
  _disabled: {
    cursor: 'default',
    _hover: { textDecoration: 'none' },
  },
});

export const tagline = css({
  color: 'fg.dim',
  fontSize: 'meta',
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  // Mobile drops the tagline — at <720 px the wordmark alone
  // has to fit alongside the status, and the tagline is
  // promotional copy without functional value.
  smDown: { display: 'none' },
});

export const headerStatus = css({
  color: 'fg.dim',
  fontSize: 'meta',
  textAlign: 'right',
  // Reserve one line of vertical space so the header doesn't
  // collapse when `status` is empty between operations. `1em`
  // (the current font's em-square) is a CSS-intrinsic primitive
  // that has no φ-token equivalent — the escape hatch is honest
  // about the distinction.
  minHeight: '[1em]',
  paddingLeft: 'phi.0',
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
});

// ─── Stage (single grid cell that swaps content per mode) ────────

export const stage = css({
  gridArea: 'stage',
  display: 'grid',
  placeItems: 'center',
  overflow: 'hidden',
  padding: 'phi.1',
  minWidth: '0',
  minHeight: '0',
  background: 'bg.stage',
});

// Inner content wrapper shared by the empty / batch modes.
// `placeItems: center` here centres the drop zone or batch list
// inside the stage. The canvas variant overrides this — see
// `stageCanvas` below.
const stageInnerBase = css.raw({
  width: 'full',
  height: 'full',
  display: 'grid',
  placeItems: 'center',
  minWidth: '0',
  minHeight: '0',
});

// `position: relative` so the GoldenSpiral SVG (absolutely
// positioned) anchors to this content box rather than the page.
export const stageEmpty = css(stageInnerBase, { position: 'relative' });

// Canvas variant must stretch its child (the <canvas>) so the
// canvas can resolve `height: 100%` against a definite block.
// Centering broke that resolution and the canvas snapped back to
// its intrinsic 1720×1285, hiding the bottom EXIF strip below the
// fold on common laptops — that's the bug Phase A repairs.
export const stageCanvas = css(stageInnerBase, { placeItems: 'stretch' });

// Batch variant stretches both axes for the full-width row list.
export const stageBatch = css(stageInnerBase, {
  alignItems: 'stretch',
  justifyItems: 'stretch',
  gridTemplateRows: '1fr',
});

// `<canvas width=N height=N>` HTML attrs implicitly set the
// canvas's intrinsic `aspect-ratio` for layout, which keeps the
// element from honouring `height: 100%` against the parent (the
// browser solves layout with the aspect first, then ignores the
// percent). `aspect-ratio: auto` strips that intrinsic ratio so
// the canvas fills its grid cell fully (1552×984 instead of
// 1552×1160 on a 1920×1080 viewport), and `object-fit: contain`
// letterboxes the drawing buffer inside that box so portrait and
// landscape outputs both fit without clipping the caption strip.
export const previewCanvas = css({
  width: 'full',
  height: 'full',
  // The contain-fit is done in `paintPreview` (App.tsx): the
  // drawing buffer matches the container × DPR and the image is
  // letterboxed inside it. The canvas element itself has no
  // intrinsic ratio that could fight CSS layout — so no
  // `aspect-ratio` or `object-fit` are needed (or wanted; the
  // latter would only ever apply *inside* the drawing buffer).
  border: 'soft',
  borderRadius: 'phi.m3',
  imageRendering: 'auto',
});

// ─── Sidebar ─────────────────────────────────────────────────────

export const sidebar = css({
  gridArea: 'sidebar',
  overflowY: 'auto',
  paddingX: 'phi.0',
  paddingTop: 'phi.0',
  paddingBottom: 'phi.1',
  borderLeft: 'soft',
  background: 'bg.sidebar',
  display: 'flex',
  flexDirection: 'column',
  gap: 'phi.0',
  // Mobile lifts the sidebar to a bottom row instead of a right
  // column; cap the height so it doesn't dominate the screen.
  smDown: {
    // `55dvh` keeps the sidebar to roughly half the dynamic
    // viewport so the stage above still gets the lion's share.
    // No φ-token equivalent because dvh is intrinsic to the
    // browser's viewport math.
    maxHeight: '[55dvh]',
    borderLeft: 'none',
    borderTop: 'soft',
  },
});

export const sidebarFooter = css({
  marginTop: 'auto',
  paddingTop: 'phi.0',
  borderTop: 'soft',
  color: 'fg.faint',
  fontSize: 'caption',
});

// ─── Controls ────────────────────────────────────────────────────

export const controls = css({
  display: 'flex',
  flexDirection: 'column',
  gap: 'phi.0',
});

export const field = css({
  display: 'flex',
  flexDirection: 'column',
  gap: 'phi.m2',
});

export const fieldLabel = css({
  fontSize: 'caption',
  textTransform: 'uppercase',
  letterSpacing: 'caps',
  color: 'fg.dim',
  fontWeight: 'medium',
});

export const fieldBody = css({ display: 'block' });

// Segmented container — the buttons inside use the
// `segmentedButton` recipe (panda/recipes.ts).
export const segmented = css({
  display: 'grid',
  gridAutoFlow: 'column',
  // `1fr` is a grid-track unit, not a length — sits outside the
  // size-token vocabulary by design.
  gridAutoColumns: '[1fr]',
  border: 'default',
  borderRadius: 'phi.m3',
  overflow: 'hidden',
  background: 'bg.elev',
});

export const sliderRow = css({
  display: 'grid',
  gridTemplateColumns: '1fr auto',
  alignItems: 'center',
  gap: 'phi.m1',
});

export const sliderValue = css({
  minWidth: 'phi.2',
  textAlign: 'right',
  color: 'fg.default',
  fontSize: 'meta',
  fontVariantNumeric: 'tabular-nums',
});

export const resizeRow = css({
  display: 'grid',
  gridTemplateColumns: 'auto 1fr auto',
  alignItems: 'center',
  gap: 'phi.m2',
});

export const checkInline = css({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'phi.m2',
  color: 'fg.default',
  fontSize: 'meta',
  cursor: 'pointer',
  userSelect: 'none',
});

export const suffix = css({ color: 'fg.dim', fontSize: 'meta' });

// ─── Meter (estimated size / render time strip) ─────────────────

export const meter = css({
  display: 'flex',
  flexDirection: 'column',
  gap: 'phi.m2',
  paddingX: 'phi.m1',
  paddingY: 'phi.m1',
  background: 'bg.elev',
  border: 'soft',
  borderRadius: 'phi.m3',
});

export const meterRow = css({
  display: 'flex',
  justifyContent: 'space-between',
  alignItems: 'baseline',
  gap: 'phi.m1',
});

export const meterLabel = css({
  fontSize: 'caption',
  textTransform: 'uppercase',
  letterSpacing: 'caps',
  color: 'fg.dim',
});

export const meterValue = css({
  color: 'fg.default',
  fontSize: 'meta',
  fontVariantNumeric: 'tabular-nums',
});

// Batch list atomics were replaced by `Gallery.tsx` /
// `Gallery.styles.ts` in Phase 3 — the gallery's thumbnail-grid
// shape supersedes the old name-only list, so the corresponding
// atoms here were deleted rather than left as dead code.
