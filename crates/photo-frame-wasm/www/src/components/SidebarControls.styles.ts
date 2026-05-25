import { css } from '../../styled-system/css';

// `flex: '1.618 1 0'` claims the φ-share of the sidebar's
// vertical space — paired with whatever band sits below at
// `flex: 1 1 0` the two stack in a 1.618:1 (= φ:1) ratio,
// with the footer pinned by its own `margin-top: auto`.
export const controls = css({
  display: 'flex',
  flexDirection: 'column',
  gap: 'phi.0',
  flex: '[1.618 1 0]',
  minHeight: '0',
});

// `<details>` + `<summary>` wrapper for "advanced / secondary"
// controls — the ones that aren't part of the default happy
// path. Used to demote the resolution picker out of the primary
// control stack: most users want full resolution, and only the
// minority who deliberately want a smaller export should have
// to engage with it. Closed by default; the summary is a quiet
// dim row that brightens on hover.
export const advancedGroup = css({
  marginTop: 'phi.m2',
});

export const advancedSummary = css({
  cursor: 'pointer',
  color: 'fg.dim',
  fontSize: 'caption',
  textTransform: 'uppercase',
  letterSpacing: 'caps',
  fontWeight: 'medium',
  paddingY: 'phi.m2',
  userSelect: 'none',
  // Strip the default `<summary>` list-marker — we let the
  // text alone carry the affordance, and the marker is a tiny
  // arrow that clashes with the otherwise marker-less chrome.
  listStyle: 'none',
  '&::-webkit-details-marker': { display: 'none' },
  _hover: { color: 'fg.default' },
});

// Inner body of `<details>` — the field(s) that show when the
// disclosure is open. A small top gap separates them from the
// summary line.
export const advancedBody = css({
  paddingTop: 'phi.m2',
});
