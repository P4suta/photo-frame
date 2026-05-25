import { defineSemanticTokens, defineTokens } from '@pandacss/dev';

// Design system — three layers, one purpose each:
//
//   `palette` (inside `tokens.colors`)
//     Concrete pigment values. Names describe *what the colour is*
//     (`ink0`, `brass`), never *where it's used*. Components never
//     reach a pigment directly; re-skinning then means a project-
//     wide find-and-replace instead of a single edit.
//
//     Eleven entries — two parallel three-step surface gradients
//     (ink-dark / paper-light), two text colours, one muted tone
//     that works on both themes, plus brand and danger hues.
//
//   `phi` (inside spacing / fontSizes / radii / sizes)
//     The geometric core. Every layout dimension is `base · φⁿ`
//     for some integer `n`, mirroring how the image renderer
//     derives `pad_x`, `bottom`, `font_height` from `side` (see
//     `crates/photo-frame-frame/src/geometry.rs`). One φ
//     vocabulary spans spacing (phi.m3 ≈ 3.8px → phi.3 ≈ 68px)
//     and sizing (phi.m3 ≈ 3.8px → phi.8 ≈ 752px).
//
//   `semantic` (the `semanticTokens` export)
//     Role-named aliases (`bg.stage`, `accent.ink`, `border.soft`).
//     Components only ever reach for these. Each token has a
//     `base:` value (dark theme, the default) and a `_light:`
//     value that activates under `@media (prefers-color-scheme:
//     light)` — Panda's built-in condition. Adding a manual
//     theme toggle later means flipping a `[data-theme=light]`
//     selector, not editing any component.

// φ = (1 + √5) / 2 ≈ 1.6180339887... A literal here so a reader
// can trace the rem values below back to integer powers of the
// constant.
const PHI = 1.618_033_988_749_895;
const rem = (n: number) => `${n.toFixed(3).replace(/\.?0+$/, '')}rem`;

export const tokens = defineTokens({
  colors: {
    palette: {
      // Strict monochrome — nine pigments, no accent, no signal
      // colour, no warmth. Pure neutral greyscale: every value is
      // straight on the white→black axis (R=G=B), so the chrome
      // never competes with the framed image for tonal attention.
      // Active / primary states get expressed by inverting the
      // theme; danger and success states use typography
      // (font-weight, leading glyph, border emphasis) instead of
      // hue. Black-and-white is the design statement.
      //
      // Dark theme surfaces — three tiers (page → elevated → hover).
      // Pure black (#000) at the page, lifted ever so slightly on
      // the elevated tiers so card edges stay readable.
      ink0: { value: '#000000' },
      ink1: { value: '#141414' },
      ink2: { value: '#1f1f1f' },
      // Light theme surfaces — pure white at the page, lightly
      // greyed on the elevated tiers.
      paper0: { value: '#ffffff' },
      paper1: { value: '#ededed' },
      paper2: { value: '#e0e0e0' },
      // Text on each theme — near-pure ends of the greyscale so
      // body copy has the highest readable contrast against the
      // page tier.
      fgOnDark: { value: '#f5f5f5' },
      fgOnLight: { value: '#0a0a0a' },
      // Muted neutral — a mid grey that reads as "secondary" on
      // either theme. No hue, so it can't drift into colour bias
      // under either light or dark conditions.
      fgMuted: { value: '#808080' },
    },
  },

  // Golden-ratio spacing scale — every gap, padding, margin in the
  // UI is `1rem · φⁿ` for some integer `n` in [-3, 3]. Indexed by
  // power directly (`phi.m3` = φ⁻³, `phi.3` = φ³); `m` reads as
  // "minus" (below the 1rem base).
  spacing: {
    '0': { value: '0' },
    px: { value: '1px' }, // hairline (batch list dividers)
    phi: {
      m3: { value: rem(PHI ** -3) }, // 0.236rem ≈ 3.8px
      m2: { value: rem(PHI ** -2) }, // 0.382rem ≈ 6.1px
      m1: { value: rem(PHI ** -1) }, // 0.618rem ≈ 9.9px
      '0': { value: '1rem' }, // base 16px (φ⁰)
      '1': { value: rem(PHI) }, // 1.618rem ≈ 25.9px
      '2': { value: rem(PHI ** 2) }, // 2.618rem ≈ 41.9px
      '3': { value: rem(PHI ** 3) }, // 4.236rem ≈ 67.8px
    },
  },

  // Type scale — four steps along √φ ≈ 1.272 (the geometric mean
  // of 1 and φ). Strict integer-φ on text is too coarse for a UI
  // surface: only two sizes between "tiny caps label" and
  // "comfortable body" leaves no room for sidebar controls.
  // Stepping by √φ keeps the scale geometric (so it composes with
  // the strict-φ spacing) while doubling the granularity. Each
  // size is exactly two √φ steps from its φ neighbour.
  fontSizes: {
    caption: { value: rem(PHI ** -1) }, // 0.618rem ≈ 9.9px — uppercased caps labels
    meta: { value: rem(PHI ** -0.5) }, // 0.786rem ≈ 12.6px — sidebar control text
    body: { value: '1rem' }, // 1rem = 16px — primary UI body
    heading: { value: rem(PHI) }, // 1.618rem ≈ 25.9px — wordmark / call-out
  },

  // Same φ-cadence on radii, sharing the spacing-φ values directly
  // so a corner radius is always one φ-step less than its padding
  // — the same geometric relationship the image renderer's frame
  // honours between `side` (border) and `font_height` (caption
  // glyph height = side · φ⁻²).
  radii: {
    phi: {
      m3: { value: rem(PHI ** -3) }, // preview canvas frame, segmented control
      m2: { value: rem(PHI ** -2) }, // buttons, meter cards, batch list
      m1: { value: rem(PHI ** -1) }, // reserved for future large surfaces
    },
  },

  // The unified φ-size scale — every width, height, max-width, and
  // grid track in the UI reaches into this single namespace.
  // Spans `phi.m3` (≈ 3.8px hairline) all the way to `phi.8`
  // (≈ 752px content cap), so layout decisions get expressed as
  // "which φ-step is this?" rather than as bare rem values.
  //
  // Notable anchors used across the chrome:
  //   phi.2 ≈ 42px — header bar height
  //   phi.6 ≈ 287px — sidebar width
  //   phi.7 ≈ 464px — drop-zone content cap
  //   phi.8 ≈ 752px — batch list content cap
  sizes: {
    full: { value: '100%' }, // canvas / container fill — only non-φ size
    phi: {
      m3: { value: rem(PHI ** -3) },
      m2: { value: rem(PHI ** -2) },
      m1: { value: rem(PHI ** -1) },
      '0': { value: '1rem' },
      '1': { value: rem(PHI) },
      '2': { value: rem(PHI ** 2) }, // 42px — header
      '3': { value: rem(PHI ** 3) },
      '4': { value: rem(PHI ** 4) },
      '5': { value: rem(PHI ** 5) },
      '6': { value: rem(PHI ** 6) }, // 287px — sidebar
      '7': { value: rem(PHI ** 7) }, // 464px — drop zone
      '8': { value: rem(PHI ** 8) }, // 752px — batch list
    },
  },

  // Two weights only — Geist exposes more but the UI surface only
  // uses regular body text and a slightly-bolder accent on labels,
  // wordmark, and primary buttons. Resisting the seven-step Tailwind
  // weight scale keeps the visual rhythm tight.
  fontWeights: {
    regular: { value: '400' },
    medium: { value: '500' },
  },

  // Letter-spacing values for uppercased caps labels and the brand
  // wordmark. The values aren't φ-derived (letter-spacing is too
  // subtle for a ratio that coarse), but naming them as tokens
  // still keeps every value looked-up through one vocabulary.
  letterSpacings: {
    caps: { value: '0.08em' }, // uppercased field labels, batch status pill
    wordmark: { value: '-0.01em' }, // brand wordmark tighten
  },

  fonts: {
    // Geist first — the WASM renderer bakes the same family into
    // framed captions so chrome and image content read as one face.
    body: {
      value: '"Geist", -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif',
    },
    mono: { value: 'ui-monospace, SFMono-Regular, Menlo, monospace' },
  },
});

export const semanticTokens = defineSemanticTokens({
  colors: {
    bg: {
      // Page background — `stage` and `sidebar` share the same hue
      // by design; the border between them is the only divider.
      // Past iterations split them; merging eliminates a token
      // without losing the visual separation.
      stage: {
        value: { base: '{colors.palette.ink0}', _light: '{colors.palette.paper0}' },
      },
      sidebar: {
        value: { base: '{colors.palette.ink0}', _light: '{colors.palette.paper0}' },
      },
      // Elevated surfaces — meter cards, segmented background,
      // number input fills. One φ step lighter than the page.
      elev: {
        value: { base: '{colors.palette.ink1}', _light: '{colors.palette.paper1}' },
      },
      // Hover-state surface — segmented button hover lifts to this.
      elev2: {
        value: { base: '{colors.palette.ink2}', _light: '{colors.palette.paper2}' },
      },
    },
    fg: {
      default: {
        value: { base: '{colors.palette.fgOnDark}', _light: '{colors.palette.fgOnLight}' },
      },
      // Muted neutral — same hue both themes; ash-grey reads as
      // "secondary" against either light or dark surfaces.
      dim: {
        value: { base: '{colors.palette.fgMuted}', _light: '{colors.palette.fgMuted}' },
      },
      // Faintest tier — sidebar footer, attribution text. One
      // perceptual step below `dim` via 60% opacity. `color-mix`
      // keeps the relationship semantic instead of a magic hex.
      faint: {
        value: {
          base: 'color-mix(in oklab, {colors.palette.fgMuted} 60%, transparent)',
          _light: 'color-mix(in oklab, {colors.palette.fgMuted} 60%, transparent)',
        },
      },
    },
    // Theme-inverted surfaces — the "accent" of a strict monochrome
    // palette. Active segmented options and primary buttons paint
    // `invert.bg` with `invert.fg` text, so on dark mode you get a
    // light bg + dark text (and vice versa). The contrast is
    // absolute; no hue dimension to lose in colour-blind users or
    // greyscale prints.
    invert: {
      bg: {
        value: { base: '{colors.palette.paper0}', _light: '{colors.palette.ink0}' },
      },
      fg: {
        value: { base: '{colors.palette.ink0}', _light: '{colors.palette.paper0}' },
      },
    },
    border: {
      // Opacity-driven borders flip from white-on-dark to
      // black-on-light. Three weights — `soft` for cards, `default`
      // for buttons, `strong` for the drop-zone outline.
      default: {
        value: { base: 'rgba(255, 255, 255, 0.08)', _light: 'rgba(0, 0, 0, 0.08)' },
      },
      soft: {
        value: { base: 'rgba(255, 255, 255, 0.04)', _light: 'rgba(0, 0, 0, 0.04)' },
      },
      strong: {
        value: { base: 'rgba(255, 255, 255, 0.14)', _light: 'rgba(0, 0, 0, 0.14)' },
      },
    },
  },

  // Border shorthand bundles — the four (`width` + `style` +
  // `color`) combinations the UI uses. Defining them here lets
  // every border declaration collapse to `border: 'soft'` etc.,
  // and any width/style change cascades through one edit.
  borders: {
    default: { value: { base: '1px solid {colors.border.default}' } },
    soft: { value: { base: '1px solid {colors.border.soft}' } },
    strong: { value: { base: '1px solid {colors.border.strong}' } },
    dashedStrong: { value: { base: '1px dashed {colors.border.strong}' } },
  },
});
