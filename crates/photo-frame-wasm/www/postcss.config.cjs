// Panda CSS hooks into the build via PostCSS — Vite auto-detects this
// config and runs PostCSS for any CSS file in the graph. Pairs with
// `panda codegen` (chained from `bun run dev` / `bun run build`)
// which emits the `styled-system/` package the TSX side imports from.
module.exports = {
  plugins: {
    "@pandacss/dev/postcss": {},
  },
};
