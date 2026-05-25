import { defineConfig } from '@pandacss/dev';

import { conditions } from './panda/conditions';
import { globalCss } from './panda/global-css';
import { keyframes } from './panda/keyframes';
import { recipes } from './panda/recipes';
import { semanticTokens, tokens } from './panda/tokens';

// Panda CSS entry point — pure composition. Each design concern
// lives in its own file under `panda/`:
//
//   panda/tokens.ts       — palette + semanticTokens (the design vocabulary)
//   panda/recipes.ts      — component recipes (multi-variant primitives)
//   panda/global-css.ts   — global cascade (resets, form pseudo-elements)
//   panda/conditions.ts   — breakpoints / context-scoped selectors
//
// `strictTokens: true` makes any token typo a compile-time error —
// the whole reason we picked Panda over hand-rolled CSS variables.
// `hash: true` produces stable, conflict-free class names in
// production. `preflight: true` ships a normalize/reset at the
// bottom of the cascade so we don't carry one in `global.css`.
//
// `jsxFramework` is intentionally undefined: Panda's `<styled.div>`
// JSX runtime is React-shaped and would lose Solid's reactive
// `class={...}` semantics. We use the framework-agnostic
// `cva()` / recipe / `css()` APIs instead.
export default defineConfig({
  preflight: true,
  strictTokens: true,
  hash: true,

  include: ['./src/**/*.{ts,tsx}'],
  exclude: [],

  outdir: 'styled-system',

  conditions: { extend: conditions },

  theme: {
    extend: {
      tokens,
      semanticTokens,
      recipes,
      keyframes,
    },
  },

  globalCss,
});
