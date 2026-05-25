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

// Header is a 2-column grid split on the golden ratio: the
// brand (wordmark + tagline) takes the φ-share on the left,
// the status block takes the 1-share on the right. The split
// is structural, not flex space-between — readers can predict
// exactly how the bar will rebalance at any width.
export const appHeader = css({
  gridArea: 'header',
  display: 'grid',
  gridTemplateColumns: '[1.618fr 1fr]',
  alignItems: 'center',
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

// Canvas variant frames the preview as a golden rectangle —
// the stage area carves out a φ:1 (1.618:1) region the canvas
// lives in. The image inside is letterboxed via the contain-fit
// paint pass in `components/CanvasPreview.tsx` (geometry pinned
// by `lib/paint-preview.ts`), so portrait sources still display
// intact, just bordered by the φ-shaped frame.
export const stageCanvas = css(stageInnerBase, {
  placeItems: 'center',
});

// Batch variant stretches both axes for the full-width row list.
export const stageBatch = css(stageInnerBase, {
  alignItems: 'stretch',
  justifyItems: 'stretch',
  gridTemplateRows: '1fr',
});

// ─── Sidebar ─────────────────────────────────────────────────────
//
// Internal vertical rhythm follows the golden split: each child
// section (controls / mode-specific block / footer) takes its
// share via `flex` so the sidebar's three bands are visually in
// the φ ratio rather than a sequence of fixed-height cushions.
// The numeric scale on `gap` is a φ-token, but the *bands* are
// proportional — that's the structural use of φ the rest of the
// chrome was missing.
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
