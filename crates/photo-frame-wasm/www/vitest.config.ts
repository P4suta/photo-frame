import { defineConfig } from 'vitest/config';

// Vitest configuration — node environment by design.
// The tests in `src/**/*.test.ts` exercise pure-function geometry
// (`golden-spiral.ts`) that has no DOM dependency, so the
// default `jsdom` environment would only add a useless install
// requirement. Component tests that *do* need DOM should opt in
// per-file with a `// @vitest-environment jsdom` pragma.
export default defineConfig({
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
});
