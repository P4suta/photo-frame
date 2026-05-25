import { describe, expect, test } from 'vitest';
import { type CaptionMode, fromCaptionMode, toCaptionMode } from './caption-mode';

describe('toCaptionMode', () => {
  test("showMeta: false reads as 'off' regardless of layout", () => {
    expect(toCaptionMode({ layout: 'edges', showMeta: false })).toBe<CaptionMode>('off');
    expect(toCaptionMode({ layout: 'centered', showMeta: false })).toBe<CaptionMode>('off');
  });

  test('showMeta: true surfaces the actual layout', () => {
    expect(toCaptionMode({ layout: 'edges', showMeta: true })).toBe<CaptionMode>('edges');
    expect(toCaptionMode({ layout: 'centered', showMeta: true })).toBe<CaptionMode>('centered');
  });
});

describe('fromCaptionMode', () => {
  test("'off' keeps the previous layout and sets showMeta=false", () => {
    expect(fromCaptionMode('off', 'edges')).toEqual({ layout: 'edges', showMeta: false });
    expect(fromCaptionMode('off', 'centered')).toEqual({ layout: 'centered', showMeta: false });
  });

  test("'edges' sets both layout and showMeta=true", () => {
    expect(fromCaptionMode('edges', 'centered')).toEqual({ layout: 'edges', showMeta: true });
  });

  test("'centered' sets both layout and showMeta=true", () => {
    expect(fromCaptionMode('centered', 'edges')).toEqual({ layout: 'centered', showMeta: true });
  });
});

describe('on → off → on preserves the last layout', () => {
  test('round-trip via the off state restores the user-picked layout', () => {
    // Start at centered with showMeta on:
    let layout: 'edges' | 'centered' = 'centered';
    let showMeta = true;

    // User clicks 'off' — keep centered as the latent choice.
    ({ layout, showMeta } = fromCaptionMode('off', layout));
    expect(layout).toBe('centered');
    expect(showMeta).toBe(false);

    // User clicks 'edges' — now show edges (= new explicit choice).
    ({ layout, showMeta } = fromCaptionMode('edges', layout));
    expect(layout).toBe('edges');
    expect(showMeta).toBe(true);
  });
});
