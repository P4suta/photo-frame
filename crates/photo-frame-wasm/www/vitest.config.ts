import solid from 'vite-plugin-solid';
import { defineConfig } from 'vitest/config';

// Vitest configuration — node environment by default. Pure-
// function tests (`src/lib/**`) run in node since they have no
// DOM dependency; component tests opt in per-file with a
// `// @vitest-environment jsdom` pragma at the top, and pick
// up the jsdom polyfills from `src/test-setup.ts`.
//
// `vite-plugin-solid` is required here (not just in vite.config)
// so the test pipeline transforms the JSX in `*.test.tsx` files —
// vitest reads this config in isolation and doesn't inherit the
// dev/build plugin stack.
//
// `include` accepts `.tsx` too so component test files
// (Gallery.test.tsx etc.) are picked up alongside the bare-TS
// utility tests.
export default defineConfig({
  plugins: [solid()],
  test: {
    environment: 'node',
    include: ['src/**/*.test.{ts,tsx}'],
    setupFiles: ['./src/test-setup.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov'],
      // Coverage targets the testable surface — pure modules in
      // `src/lib/`, Solid-reactive primitives in `src/state/`,
      // and the display-only components. `App.tsx` is wiring
      // (effects + JSX) that's intentionally not in scope for
      // unit tests — its logic gets pulled into `src/lib/` /
      // `src/state/` as the refactor lands.
      include: [
        'src/lib/**',
        'src/state/**',
        'src/components/**',
        'src/Gallery.tsx',
        'src/DropZone.tsx',
      ],
    },
  },
});
