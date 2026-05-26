import { describe, expect, test } from 'vitest';
import { stageToPercent } from './progress';

/**
 * Stage → percent mapping is calibrated from `BENCHMARKS.md`. The
 * tests pin the cumulative numbers so any drift forces a deliberate
 * weight update — the progress bar feels off if these don't match
 * real timings.
 */
describe('stageToPercent', () => {
  test('decode completion lands around one third', () => {
    expect(stageToPercent('decode')).toBe(33);
  });

  test('frame completion barely moves past decode', () => {
    // frame is the cheapest stage (~3% of wall-clock) so the bar
    // only ticks up slightly between decode and encode.
    expect(stageToPercent('frame')).toBe(36);
  });

  test('encode completion finishes the item', () => {
    expect(stageToPercent('encode')).toBe(100);
  });

  test('unknown stage labels collapse to zero defensively', () => {
    expect(stageToPercent('upload')).toBe(0);
    expect(stageToPercent('')).toBe(0);
  });
});
