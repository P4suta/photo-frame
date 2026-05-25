import { css } from '../styled-system/css';

// Atomics for the batch gallery — the queued-card grid and the
// processed-result grid share one layout, only the card body
// changes. Card minimum width = `phi.6` (≈ 287px), gap = `phi.0`
// (1rem); when the viewport allows, cards reflow into more
// columns automatically (`repeat(auto-fill, …)`).

// Container — the grid that holds every card. Width caps at
// `phi.8` (≈ 752px = roughly two cards wide at the minimum size),
// keeping the gallery from feeling sparse on ultra-wide screens.
export const gallery = css({
  display: 'grid',
  gridTemplateColumns: '[repeat(auto-fill, minmax({sizes.phi.6}, 1fr))]',
  gap: 'phi.0',
  width: '[min({sizes.phi.8}, 100%)]',
  maxHeight: 'full',
  overflowY: 'auto',
  alignSelf: 'center',
  justifySelf: 'center',
  padding: 'phi.m1',
});

// Card — vertical grid of thumbnail and metadata-footer, split
// in the golden ratio (thumb : footer = φ:1) so the card itself
// is a 1:φ golden rectangle. The thumb is a 1:1 square inside
// the larger φ-share; the footer takes the smaller 1-share.
// Stacking grid rows in `1.618fr 1fr` keeps the proportion exact
// at any column width.
export const galleryCard = css({
  display: 'grid',
  gridTemplateRows: '[1.618fr 1fr]',
  aspectRatio: '[1 / 1.618]',
  gap: 'phi.m1',
  padding: 'phi.m1',
  background: 'transparent',
  border: 'soft',
  borderRadius: 'phi.m2',
  position: 'relative',
});

// Thumbnail — fills the φ-share row of the card (the upper
// 1.618fr track). `object-fit: contain` on the image (see
// galleryThumbImg) lets framed previews of either orientation
// letterbox inside without distortion.
export const galleryThumb = css({
  width: 'full',
  height: 'full',
  background: 'bg.stage',
  borderRadius: 'phi.m3',
  overflow: 'hidden',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  minHeight: '0',
});

export const galleryThumbImg = css({
  width: 'full',
  height: 'full',
  objectFit: 'contain',
});

// Placeholder shown while a thumbnail is still rendering. Pulsing
// opacity gives a "working" cue without spawning a spinner widget.
export const galleryThumbPlaceholder = css({
  width: 'full',
  height: 'full',
  background: 'bg.sidebar',
  animation: '[gallery-pulse 1.4s ease-in-out infinite]',
});

export const galleryName = css({
  fontFamily: 'mono',
  fontSize: 'caption',
  color: 'fg.default',
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  // The status-aware footer above places this on its own line —
  // tighten the line-height so two cards of similar filename
  // length line up vertically.
  lineHeight: '[1.2]',
});

export const galleryFooter = css({
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: 'phi.m1',
});

// Status pill — caps label with optional leading glyph (set via
// the consumer's text content, e.g. `✓ DONE`). Colour comes from
// `data-status` on the parent card so a single rule covers all
// four states.
export const galleryStatus = css({
  textTransform: 'uppercase',
  letterSpacing: 'caps',
  fontSize: 'caption',
  color: 'fg.dim',
  // Done = bold, no colour shift (strict monochrome).
  '[data-status="done"] &': {
    fontWeight: 'medium',
    color: 'fg.default',
  },
  // Error = bold + leading rule on the card itself (see
  // `galleryCard`'s data-status selector below).
  '[data-status="error"] &': {
    fontWeight: 'medium',
    color: 'fg.default',
  },
});

// Per-status card decoration. The error variant carries a strong
// left rule — the same φ-thick monochrome signal the batch list
// used pre-gallery refactor.
export const galleryCardStatus = css({
  '&[data-status="error"]': {
    borderLeft: 'strong',
  },
  '&[data-status="processing"] .gallery-thumb': {
    animation: '[gallery-pulse 1.2s ease-in-out infinite]',
  },
});

// Whole-card click target for downloads — the card becomes a
// button once its row reaches `done`. Hover bumps the card's
// border to the strong weight; there's no elevation tier to
// shift to in the new strict-monochrome palette.
export const galleryCardButton = css({
  appearance: 'none',
  background: 'transparent',
  border: 'none',
  font: 'inherit',
  textAlign: 'inherit',
  // `currentColor` keeps the button text aligned with the
  // surrounding card colour; `inherit` isn't in the colour-token
  // namespace so the escape hatch flags the CSS-keyword nature.
  color: '[inherit]',
  cursor: 'pointer',
  padding: '0',
  width: 'full',
  // Hover affordance — a subtle opacity drop on the card,
  // monochrome, no bg shift. Suppressed while the row is still
  // processing (button is disabled there) so the card doesn't
  // imply clickability before the result is ready.
  transition: '[opacity 120ms ease]',
  _hover: {
    opacity: 0.75,
  },
  _disabled: {
    cursor: 'default',
    _hover: { opacity: 1 },
  },
  _focusVisible: {
    outline: '[2px solid {colors.fg.default}]',
    outlineOffset: '[2px]',
  },
});
