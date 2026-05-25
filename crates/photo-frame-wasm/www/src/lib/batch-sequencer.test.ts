import { describe, expect, test } from 'vitest';
import { createGenerationGate } from './batch-sequencer';

describe('createGenerationGate', () => {
  test('starts at 0 and bumps monotonically', () => {
    const gate = createGenerationGate();
    expect(gate.current()).toBe(0);
    expect(gate.bump()).toBe(1);
    expect(gate.bump()).toBe(2);
    expect(gate.bump()).toBe(3);
    expect(gate.current()).toBe(3);
  });

  test('isCurrent: the freshly-issued gen is current', () => {
    const gate = createGenerationGate();
    const gen = gate.bump();
    expect(gate.isCurrent(gen)).toBe(true);
  });

  test('isCurrent: an older gen loses currency after the next bump', () => {
    const gate = createGenerationGate();
    const first = gate.bump();
    gate.bump();
    expect(gate.isCurrent(first)).toBe(false);
  });

  test('isCurrent: rejects a future or stranger gen number', () => {
    const gate = createGenerationGate();
    gate.bump();
    expect(gate.isCurrent(999)).toBe(false);
    expect(gate.isCurrent(0)).toBe(false);
  });

  test('isCurrent stays true across reads (pure query, no state)', () => {
    const gate = createGenerationGate();
    const gen = gate.bump();
    expect(gate.isCurrent(gen)).toBe(true);
    expect(gate.isCurrent(gen)).toBe(true);
    expect(gate.current()).toBe(gen);
  });

  test('two gates are independent', () => {
    // The prepare / thumbnail / batch gates in App.tsx must not
    // shadow each other — bumping one mid-flight should not
    // invalidate the others.
    const a = createGenerationGate();
    const b = createGenerationGate();
    const aGen = a.bump();
    b.bump();
    b.bump();
    expect(a.isCurrent(aGen)).toBe(true);
    expect(b.current()).toBe(2);
    expect(a.current()).toBe(1);
  });

  test('simulated async race: handler captures gen, two bumps later it bails', async () => {
    // The exact shape of the App.tsx race the gate exists to
    // guard against — pin it as an executable scenario.
    const gate = createGenerationGate();
    const stale = gate.bump();
    const fresh = gate.bump();
    const handler = async (): Promise<'wrote' | 'skipped'> => {
      await Promise.resolve();
      return gate.isCurrent(stale) ? 'wrote' : 'skipped';
    };
    await expect(handler()).resolves.toBe('skipped');
    expect(gate.isCurrent(fresh)).toBe(true);
  });

  test('handler issued after bump still valid', async () => {
    const gate = createGenerationGate();
    gate.bump();
    const gen = gate.bump();
    const handler = async (): Promise<boolean> => {
      await Promise.resolve();
      return gate.isCurrent(gen);
    };
    await expect(handler()).resolves.toBe(true);
  });
});
