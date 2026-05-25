import { defineGlobalStyles } from '@pandacss/dev';

// Global cascade — selectors that don't belong to any component:
//
//   - Two-or-three-line resets / typography defaults that every
//     page inherits (`html, body, #root`, base `body` styles).
//   - Native form element pseudo-elements (`input[type=range]`'s
//     three-vendor thumb/track triplet, `input[type=number]` look)
//     that recipes can't express because the selector lives on a
//     pseudo-element, not the element itself.
//   - Cross-cutting interaction states (`a:hover`, `:focus-visible`)
//     where wrapping every link in a recipe would be heavier than
//     the rule itself.
//
// Everything else (component visuals, layout primitives) lives in
// recipes or component-local `css()` calls — never here. All
// values flow through the φ-derived design tokens so the global
// cascade reads as one geometric vocabulary with the rest of the
// UI.
export const globalCss = defineGlobalStyles({
  'html, body, #root': {
    margin: '0',
    height: 'full',
  },
  body: {
    background: 'bg.stage',
    color: 'fg.default',
    fontFamily: 'body',
    fontSize: 'meta',
    lineHeight: '1.45',
    fontVariantNumeric: 'tabular-nums',
    WebkitFontSmoothing: 'antialiased',
    textRendering: 'optimizeLegibility',
    overscrollBehavior: 'none',
  },
  '*': {
    boxSizing: 'border-box',
  },

  // Range slider — three-vendor pseudo-element triplet that recipes
  // can't reach (the selector lives on the slider's internal nodes,
  // not the <input> element itself).
  'input[type="range"]': {
    appearance: 'none',
    width: '100%',
    height: 'phi.0',
    background: 'transparent',
    cursor: 'pointer',
  },
  'input[type="range"]::-webkit-slider-runnable-track': {
    height: '2px',
    background: 'border.strong',
    borderRadius: 'phi.m3',
  },
  'input[type="range"]::-moz-range-track': {
    height: '2px',
    background: 'border.strong',
    borderRadius: 'phi.m3',
  },
  'input[type="range"]::-webkit-slider-thumb': {
    appearance: 'none',
    width: 'phi.m1',
    height: 'phi.m1',
    background: 'fg.default',
    borderRadius: '[50%]',
    border: 'none',
    marginTop: '-phi.m3',
    transition: 'transform 120ms ease',
  },
  'input[type="range"]:hover::-webkit-slider-thumb, input[type="range"]:focus-visible::-webkit-slider-thumb':
    {
      transform: 'scale(1.15)',
    },
  'input[type="range"]::-moz-range-thumb': {
    width: 'phi.m1',
    height: 'phi.m1',
    background: 'fg.default',
    borderRadius: '[50%]',
    border: 'none',
  },

  // Number input — page-coloured fill with a thin border. The
  // grey elevation tier is gone; borders carry the affordance.
  'input[type="number"]': {
    width: '100%',
    paddingX: 'phi.m2',
    paddingY: 'phi.m3',
    background: 'transparent',
    color: 'fg.default',
    border: 'default',
    borderRadius: 'phi.m3',
    font: 'inherit',
    fontSize: 'meta',
    fontVariantNumeric: 'tabular-nums',
    textAlign: 'right',
  },
  'input[type="number"]:focus': {
    outline: 'none',
    borderColor: 'fg.default',
  },
  'input[type="number"]:disabled': {
    opacity: 0.4,
    cursor: 'not-allowed',
  },
  'input[type="checkbox"]': {
    accentColor: 'fg.default',
    width: 'phi.m1',
    height: 'phi.m1',
    margin: '0',
    cursor: 'pointer',
  },

  a: {
    color: 'fg.dim',
    textDecoration: 'underline',
    textUnderlineOffset: '2px',
    textDecorationColor: 'border.default',
  },
  'a:hover': {
    color: 'fg.default',
    textDecorationColor: 'fg.default',
  },

  // Accent ring on every interactive element — single source of
  // truth for keyboard focus visibility.
  ':focus-visible': {
    outline: '2px solid {colors.fg.default}',
    outlineOffset: '2px',
    borderRadius: 'phi.m3',
  },
});
