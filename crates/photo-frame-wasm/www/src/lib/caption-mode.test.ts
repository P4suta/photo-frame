import * as fc from 'fast-check';
import { describe, expect, test } from 'vitest';
import { CAPTION_MODES, type CaptionMode, fromCaptionMode, toCaptionMode } from './caption-mode';

describe('toCaptionMode', () => {
  test("metaPolicy: 'never' reads as 'off' regardless of layout", () => {
    expect(toCaptionMode({ layout: 'edges', metaPolicy: 'never' })).toBe<CaptionMode>('off');
    expect(toCaptionMode({ layout: 'centered', metaPolicy: 'never' })).toBe<CaptionMode>('off');
  });

  test("metaPolicy: 'auto' surfaces the actual layout", () => {
    expect(toCaptionMode({ layout: 'edges', metaPolicy: 'auto' })).toBe<CaptionMode>('edges');
    expect(toCaptionMode({ layout: 'centered', metaPolicy: 'auto' })).toBe<CaptionMode>('centered');
  });
});

describe('fromCaptionMode', () => {
  test("'off' keeps the previous layout and sets metaPolicy='never'", () => {
    expect(fromCaptionMode('off', 'edges')).toEqual({ layout: 'edges', metaPolicy: 'never' });
    expect(fromCaptionMode('off', 'centered')).toEqual({
      layout: 'centered',
      metaPolicy: 'never',
    });
  });

  test("non-off modes set both layout and metaPolicy='auto'", () => {
    expect(fromCaptionMode('edges', 'centered')).toEqual({ layout: 'edges', metaPolicy: 'auto' });
    expect(fromCaptionMode('centered', 'edges')).toEqual({
      layout: 'centered',
      metaPolicy: 'auto',
    });
  });
});

describe('on → off → on preserves the last layout', () => {
  test('round-trip via the off state restores the user-picked layout', () => {
    let layout: 'edges' | 'centered' = 'centered';
    let metaPolicy: 'auto' | 'never' = 'auto';

    // User clicks 'off' — keep centered as the latent choice.
    ({ layout, metaPolicy } = fromCaptionMode('off', layout));
    expect(layout).toBe('centered');
    expect(metaPolicy).toBe('never');

    // User clicks 'edges' — now show edges (= new explicit choice).
    ({ layout, metaPolicy } = fromCaptionMode('edges', layout));
    expect(layout).toBe('edges');
    expect(metaPolicy).toBe('auto');
  });
});

describe('CAPTION_MODES property test', () => {
  test('round-trip through to/fromCaptionMode preserves the picked mode for non-off', () => {
    const captionValues = CAPTION_MODES.map((m) => m.value);
    const layoutValues = captionValues.filter((v): v is Exclude<CaptionMode, 'off'> => v !== 'off');
    fc.assert(
      fc.property(
        fc.constantFrom(...layoutValues),
        fc.constantFrom(...layoutValues),
        (pick, prev) => {
          const next = fromCaptionMode(pick, prev);
          expect(toCaptionMode(next)).toBe(pick);
        },
      ),
    );
  });
});
