// Phase-1 smoke test — proves the vitest pipeline runs at all
// (config picks up `src/**/*.test.{ts,tsx}`, setup file loads
// without throwing in the node environment). Deleted in
// Phase 2 once real `src/lib/*.test.ts` files take over.
import { expect, test } from 'vitest';

test('vitest pipeline is wired', () => {
  expect(1 + 1).toBe(2);
});
