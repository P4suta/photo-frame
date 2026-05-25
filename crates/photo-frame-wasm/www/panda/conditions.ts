// Conditions — Panda's name for "context-scoped style overrides"
// (media queries, pseudo-classes, ancestor classes, etc.). The
// recipe / css() / semanticTokens APIs reference them by key
// (e.g. `_smDown: { ... }`, `_light: { value: ... }`).
export const conditions = {
  // Single breakpoint by design: the pre-migration stylesheet had
  // one `@media (max-width: 720px)` rule that swapped the grid to a
  // single column. We resist adding more breakpoints until a concrete
  // second layout actually exists — a continuum of `sm/md/lg/xl`
  // invites untested intermediate widths and the photo-tool surface
  // stays binary (desktop two-column, narrow one-column).
  smDown: '@media (max-width: 720px)',

  // Override Panda's default `_light` (class-based, `[data-color-mode=light] &`)
  // to instead activate on system preference. This lets the
  // semanticTokens' `_light:` slot ship a true automatic
  // light/dark theme without any explicit toggle wiring; users
  // who flip their OS to light mode see the light palette
  // immediately. A future explicit toggle can re-introduce the
  // class-based condition alongside this one.
  light: '@media (prefers-color-scheme: light)',
} as const;
