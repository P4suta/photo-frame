import { css } from '../styled-system/css';

// Unsplash-style column masonry: each thumbnail keeps its source
// aspect ratio (portrait, landscape, square — whatever the input
// was) and CSS flows the cards down N columns in order of arrival.
// This produces the dense, mixed-orientation tile look that reads
// as a "photo set" rather than a uniform grid of φ-portraits.
//
// CSS Grid 3-column layout. We tried CSS columns first — they
// would have given true Pinterest-style masonry packing — but
// the column rendering wasn't taking hold when the gallery sat
// inside the φ-split grid stage (suspected: the parent grid
// item's intrinsic-size negotiation interacts badly with a
// `column-count` child, and the count collapses to 1). A flat
// `grid-template-columns: repeat(3, 1fr)` is the boring-but-
// reliable fallback: each card occupies one cell, the natural
// `height: auto` on the thumbnail image lets rows have
// different heights. Visually it's no longer dense-packed
// masonry, but it *does* tile correctly at every width.
export const gallery = css({
  display: 'grid',
  gridTemplateColumns: '[repeat(3, minmax(0, 1fr))]',
  gap: 'phi.m1',
  width: 'full',
  maxHeight: 'full',
  overflowY: 'auto',
  padding: 'phi.m1',
  alignItems: 'start',
  // `<ul>` default styling — null them so the grid isn't
  // offset by left padding / bullets.
  margin: '0',
  listStyle: 'none',
  smDown: {
    gridTemplateColumns: '[repeat(2, minmax(0, 1fr))]',
  },
});

// Card — sits inside a grid cell, takes whatever height the
// thumbnail's natural aspect ratio dictates. Shadow is theme-
// aware: a soft white glow on dark, a classic black drop on
// light, so the card-vs-page boundary reads in either mode.
export const galleryCard = css({
  display: 'block',
  width: 'full',
  padding: 'phi.m1',
  background: 'transparent',
  borderRadius: 'phi.m2',
  position: 'relative',
  boxShadow: '[0 6px 24px rgba(255, 255, 255, 0.10)]',
  _light: {
    boxShadow: '[0 6px 24px rgba(0, 0, 0, 0.18)]',
  },
});

// Thumbnail — width-full container that adopts the image's
// natural aspect ratio via `height: auto`. No fixed aspect, no
// flex centering: the `<img>` is the height authority.
export const galleryThumb = css({
  width: 'full',
  background: 'bg.stage',
  borderRadius: 'phi.m3',
  overflow: 'hidden',
  display: 'block',
});

export const galleryThumbImg = css({
  width: 'full',
  height: '[auto]',
  display: 'block',
  // No object-fit needed — the image's natural ratio drives the
  // wrapper height.
});

// Placeholder shown while a thumbnail is still rendering. The
// `aspect-ratio: 1` reserves a square cell so the masonry column
// doesn't collapse to zero while the thumb is in flight; the
// pulsing opacity is the "working on it" affordance. Once the
// real `<img>` arrives, its natural ratio takes over.
export const galleryThumbPlaceholder = css({
  width: 'full',
  aspectRatio: '[1]',
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

// (The per-card download button was removed — batch downloads
// are funnelled through the single "Download all" affordance in
// the sidebar instead. Cards stay passive `<li>`s.)
