import { css } from '../styled-system/css';

// Unsplash-style column masonry: each thumbnail keeps its source
// aspect ratio (portrait, landscape, square — whatever the input
// was) and CSS flows the cards down N columns in order of arrival.
// This produces the dense, mixed-orientation tile look that reads
// as a "photo set" rather than a uniform grid of φ-portraits.
//
// Column count is *prescribed*, not derived from `column-width`
// alone. Browsers given only a min width pick whatever fits and
// happily settle on 1 column when the stage is narrow — which
// is what produced the earlier "one tall stack of upscaled
// thumbs" look. Pinning the count to 3 by default (and 2 on
// the smDown breakpoint where the sidebar stacks below the
// stage anyway) guarantees a real tile grid at every width
// the layout supports.
//
// `column-width: phi.5` stays as the *minimum* width — if a
// 3-column split would make each column narrower than 178 px,
// the browser drops to fewer columns instead of squashing the
// thumbs to illegible widths. In practice the φ-split keeps
// stage wide enough that 3 always fits at the desktop layout.
export const gallery = css({
  columnCount: '[3]',
  columnWidth: 'phi.5',
  columnGap: 'phi.m1',
  width: 'full',
  maxHeight: 'full',
  overflowY: 'auto',
  padding: 'phi.m1',
  // `<ul>` default styling — null them so the masonry isn't
  // offset by left padding / bullets.
  margin: '0',
  listStyle: 'none',
  smDown: {
    columnCount: '[2]',
  },
});

// Card — natural height, driven by the thumbnail's source aspect
// ratio plus the footer text. No fixed grid rows here; the card
// is just a stack that respects whatever aspect the image came
// in at. `break-inside: avoid` prevents the card from being
// split across a column boundary; `display: inline-block` is the
// idiomatic way to give CSS columns a clean break unit. A subtle
// drop shadow gives each card depth so the boundary between
// framed photo and page stays legible without needing a border.
export const galleryCard = css({
  display: 'inline-block',
  width: 'full',
  marginBottom: 'phi.m1',
  padding: 'phi.m1',
  background: 'transparent',
  borderRadius: 'phi.m2',
  position: 'relative',
  breakInside: 'avoid',
  boxShadow: '[0 6px 24px rgba(0, 0, 0, 0.22)]',
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
