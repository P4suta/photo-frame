import { describe, expect, test } from 'vitest';
import { framedName, type RowStatus, statusLabel, stringifyError, uint8ToBuffer } from './format';

describe('framedName', () => {
  test('inserts _framed before a standard extension', () => {
    expect(framedName('photo.jpg')).toBe('photo_framed.jpg');
  });

  test('preserves the stem case but normalises the extension to .jpg', () => {
    expect(framedName('Vacation.JPG')).toBe('Vacation_framed.jpg');
    expect(framedName('shot.png')).toBe('shot_framed.jpg');
  });

  test('handles names without an extension', () => {
    expect(framedName('photo')).toBe('photo_framed.jpg');
  });

  test('splits on the *last* dot for multi-dot names', () => {
    expect(framedName('vacation.2024.jpg')).toBe('vacation.2024_framed.jpg');
  });

  test('treats a leading dot as not-an-extension (dotfiles stay intact)', () => {
    // `.config.jpg` — last dot is the extension separator; the
    // leading dot is part of the stem. The implementation is
    // expected to keep the dotfile prefix.
    expect(framedName('.config.jpg')).toBe('.config_framed.jpg');
  });
});

describe('stringifyError', () => {
  test('returns the .message for Error subclasses', () => {
    expect(stringifyError(new Error('boom'))).toBe('boom');
    expect(stringifyError(new TypeError('bad'))).toBe('bad');
  });

  test('passes strings through verbatim', () => {
    expect(stringifyError('plain')).toBe('plain');
  });

  test('falls back to String() for other shapes', () => {
    expect(stringifyError(42)).toBe('42');
    expect(stringifyError(null)).toBe('null');
    expect(stringifyError(undefined)).toBe('undefined');
    expect(stringifyError({ foo: 'bar' })).toBe('[object Object]');
  });
});

describe('uint8ToBuffer', () => {
  test('preserves every byte', () => {
    const src = new Uint8Array([1, 2, 3, 4, 5]);
    const buf = uint8ToBuffer(src);
    expect(buf.byteLength).toBe(5);
    expect(Array.from(new Uint8Array(buf))).toEqual([1, 2, 3, 4, 5]);
  });

  test('returns a fresh ArrayBuffer (does not alias)', () => {
    const src = new Uint8Array([10, 20, 30]);
    const buf = uint8ToBuffer(src);
    // The result is a *copy* — mutating the source after the
    // call must not affect the buffer.
    src[0] = 99;
    expect(new Uint8Array(buf)[0]).toBe(10);
  });

  test('handles an empty Uint8Array', () => {
    const buf = uint8ToBuffer(new Uint8Array(0));
    expect(buf.byteLength).toBe(0);
  });
});

describe('statusLabel', () => {
  test('returns the right label for each status', () => {
    const cases: ReadonlyArray<[RowStatus, string]> = [
      ['queued', 'Queued'],
      ['processing', 'Processing'],
      ['done', '✓ Done'],
      ['error', '✗ Error'],
    ];
    for (const [input, expected] of cases) {
      expect(statusLabel(input)).toBe(expected);
    }
  });
});
