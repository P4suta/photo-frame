// Vitest setup — runs before every test file. The polyfills
// below only patch globals when a DOM-ish runtime is around
// (= a test file declared `// @vitest-environment jsdom`),
// since the node-environment majority of the suite has no
// `window` to touch.
//
// All shims are *deliberately minimal* — they exist to keep
// jsdom from throwing on browser APIs the tests don't actually
// exercise, not to simulate them faithfully. Component tests
// that need a real ResizeObserver / createImageBitmap should
// install their own per-test mock via `vi.fn()`.

import { vi } from 'vitest';

if (typeof window !== 'undefined') {
  // jsdom doesn't ship a ResizeObserver. Stub it so components
  // that call `new ResizeObserver(...)` in a `createEffect`
  // don't crash at construction time.
  if (typeof globalThis.ResizeObserver === 'undefined') {
    class ResizeObserverStub {
      observe(): void {
        // No-op: tests that care about the layout side of ResizeObserver
        // install their own per-test mock via `vi.fn()`.
      }
      unobserve(): void {
        // No-op: see `observe`.
      }
      disconnect(): void {
        // No-op: see `observe`.
      }
    }
    globalThis.ResizeObserver = ResizeObserverStub as unknown as typeof ResizeObserver;
  }

  // `createImageBitmap` is browser-only. The default mock
  // returns a 100×100 dummy bitmap with a no-op `close`. Tests
  // that need a specific width/height should replace this with
  // `vi.spyOn(globalThis, 'createImageBitmap').mockResolvedValue(...)`.
  if (typeof globalThis.createImageBitmap === 'undefined') {
    globalThis.createImageBitmap = vi.fn().mockResolvedValue({
      width: 100,
      height: 100,
      close: () => undefined,
    }) as unknown as typeof createImageBitmap;
  }

  // `URL.createObjectURL` / `revokeObjectURL` are counting mocks
  // (returning monotonic fake URLs) so tests can assert that
  // URLs are revoked when expected without depending on jsdom's
  // partial implementation.
  if (typeof URL.createObjectURL === 'undefined') {
    let nextId = 1;
    URL.createObjectURL = vi.fn(() => `blob:test/${nextId++}`);
    URL.revokeObjectURL = vi.fn();
  }
}
