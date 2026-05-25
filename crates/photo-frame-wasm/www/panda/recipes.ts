import { defineRecipe } from '@pandacss/dev';

// Component recipes — multi-variant primitives the TSX consumes
// directly as `recipeName({ variant: value })`. One file lists all
// five so the design system stays scannable; co-locating each with
// its component would scatter style decisions across the tree.
//
// What earns recipe status: a component whose visual identity has
// two or more discrete states the markup needs to flip between
// (active / inactive, idle / processing / done / error, etc.).
// One-off layouts go through atomic `css()` calls instead.
//
// Every value below is sourced from the φ-derived token vocabulary
// (`tokens.spacing.phi.*`, `tokens.fontSizes.*`, `tokens.radii.*`,
// `borders.*`) so the controls share the same geometric language
// as the layout chrome they sit on.

// Primary call-to-action and ghost actions used across the sidebar
// and batch list. The `intent` axis carries both colour and
// interaction affordances; `:disabled` styling lives here so the
// recipe stays the single source of truth for button look.
const button = defineRecipe({
  className: 'button',
  base: {
    display: 'block',
    width: 'full',
    border: 'none',
    font: 'inherit',
    cursor: 'pointer',
    transition: 'transform 80ms ease, opacity 120ms ease',
    _disabled: {
      opacity: 0.45,
      cursor: 'progress',
    },
  },
  variants: {
    intent: {
      primary: {
        // Strict monochrome: the "accent" is theme inversion.
        // Dark mode → light bg + dark text; light mode → dark bg +
        // light text. Maximum contrast, no hue dependency.
        bg: 'invert.bg',
        color: 'invert.fg',
        paddingX: 'phi.0',
        paddingY: 'phi.m1',
        borderRadius: 'phi.m2',
        fontSize: 'body',
        fontWeight: 'medium',
        _hover: {
          _enabled: { transform: 'translateY(-1px)' },
        },
        _active: {
          _enabled: { transform: 'translateY(0)' },
        },
      },
      ghost: {
        bg: 'transparent',
        color: 'fg.default',
        border: 'default',
        paddingX: 'phi.m1',
        paddingY: 'phi.m2',
        borderRadius: 'phi.m3',
        fontSize: 'caption',
        width: 'auto',
        transition: 'border-color 100ms ease, color 100ms ease',
        _hover: {
          // Hover lifts the border to the strong weight — typography-
          // only signal, no colour shift.
          borderColor: 'border.strong',
        },
      },
    },
  },
  defaultVariants: { intent: 'primary' },
});

// One button inside the segmented control (preset / theme / layout
// pickers). The container styling lives in the `segmented` atomic;
// this recipe is just the per-button look + active state.
const segmentedButton = defineRecipe({
  className: 'segmentedBtn',
  // The renderer calls `segmentedButton({ active: x === current })`
  // — a dynamic boolean Panda's static extractor cannot resolve at
  // build time. Without `staticCss` only the `active: false` (=
  // default) variant gets emitted, and the selected button reads
  // identically to the unselected ones. Emitting both variants
  // unconditionally is a one-line cost in CSS size and guarantees
  // the active class is always there to win the cascade.
  staticCss: [{ active: [true, false] }],
  base: {
    margin: '0',
    paddingX: 'phi.m1',
    paddingY: 'phi.m2',
    bg: 'transparent',
    color: 'fg.dim',
    border: 'none',
    borderRight: 'default',
    font: 'inherit',
    fontSize: 'meta',
    cursor: 'pointer',
    transition: 'color 120ms ease, background 120ms ease',
    _last: { borderRight: 'none' },
  },
  variants: {
    active: {
      true: {
        // Theme inversion = the "selected" affordance. Reads as
        // strong without needing brand colour.
        color: 'invert.fg',
        bg: 'invert.bg',
        cursor: 'default',
      },
      false: {
        // Hover lift is colour-only — there's no elev tier to
        // shift the background to. `fg.default` reads as a clear
        // step over the base `fg.dim`.
        _hover: { color: 'fg.default' },
      },
    },
  },
  defaultVariants: { active: false },
});

// Empty-state drop target. `over` is the drag-over highlight state
// the DropZone signal flips when files enter / leave the element.
const dropZone = defineRecipe({
  className: 'dropZone',
  base: {
    // Drop target spans the whole empty-state stage so the user
    // can grab any pixel on the page to drop onto — no boxed
    // affordance, no dashed frame, just centred prompt text
    // that brightens on hover. The drag-over state inverts the
    // surface to read as "yes, drop here" without a border.
    width: 'full',
    height: 'full',
    background: 'transparent',
    color: 'fg.dim',
    font: 'inherit',
    fontSize: 'body',
    textAlign: 'center',
    cursor: 'pointer',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    transition: 'color 140ms ease, background 140ms ease',
    _hover: {
      color: 'fg.default',
    },
    _focusVisible: {
      color: 'fg.default',
      outline: 'none',
    },
  },
  variants: {
    over: {
      true: {
        // Drag-over: invert the whole pane so the affordance is
        // unmistakable. No border — the colour swap carries it.
        background: 'invert.bg',
        color: 'invert.fg',
      },
      false: {},
    },
  },
  defaultVariants: { over: false },
});

// Top-level shell grid. `mode` collapses the sidebar column for the
// empty state so the drop zone gets the full viewport width and
// preserves the two-column layout for single / batch modes.
const appShell = defineRecipe({
  className: 'appShell',
  base: {
    // 100dvh fills the dynamic viewport (handles iOS Safari's
    // toolbar shifts); no token analogue.
    height: '[100dvh]',
    overflow: 'hidden',
    display: 'grid',
    // Column ratio is the golden split itself: stage takes the
    // larger φ-share, sidebar takes the smaller 1-share. `1.618fr
    // 1fr` = 61.8% : 38.2% on any wide viewport. The sidebar gets
    // a `min-width` so controls don't crush at narrow widths;
    // below `smDown` (= 720px) the layout collapses to a single
    // column anyway.
    //
    // Header height keeps `phi.2` (≈ 42px) — the row is sized to
    // the natural height of its content (wordmark + buttons),
    // and `phi.2` is the tightest φ-step that fits both.
    gridTemplateRows: '[{sizes.phi.2} 1fr]',
    gridTemplateColumns: '[1.618fr minmax({sizes.phi.5}, 1fr)]',
    gridTemplateAreas: '[ "header header" "stage  sidebar" ]',
    smDown: {
      gridTemplateRows: '[{sizes.phi.2} 1fr auto]',
      gridTemplateColumns: '[1fr]',
      gridTemplateAreas: '[ "header" "stage" "sidebar" ]',
    },
  },
  variants: {
    mode: {
      empty: {
        gridTemplateColumns: '[1fr]',
        gridTemplateAreas: '[ "header" "stage" ]',
        smDown: {
          gridTemplateRows: '[{sizes.phi.2} 1fr]',
          gridTemplateAreas: '[ "header" "stage" ]',
        },
      },
      single: {},
      batch: {},
    },
  },
  defaultVariants: { mode: 'empty' },
});

export const recipes = {
  button,
  segmentedButton,
  dropZone,
  appShell,
};
