// @vitest-environment jsdom

import { fireEvent, render } from '@solidjs/testing-library';
import { createSignal } from 'solid-js';
import { describe, expect, test, vi } from 'vitest';
import { AppHeader } from './AppHeader';

describe('<AppHeader>', () => {
  test('renders the status text reactively', () => {
    const [status, setStatus] = createSignal('idle');
    const { getByText } = render(() => (
      <AppHeader status={status} disabled={() => false} onResetToEmpty={() => undefined} />
    ));
    expect(getByText('idle')).toBeTruthy();
    setStatus('framing…');
    expect(getByText('framing…')).toBeTruthy();
  });

  test('wordmark is disabled and untitled when `disabled` is true (empty mode)', () => {
    const { getByRole } = render(() => (
      <AppHeader status={() => ''} disabled={() => true} onResetToEmpty={() => undefined} />
    ));
    const wm = getByRole('button', { name: 'Start over' }) as HTMLButtonElement;
    expect(wm.disabled).toBe(true);
    // No tooltip when disabled (nothing to reset).
    expect(wm.title).toBe('');
  });

  test('clicking the wordmark fires onResetToEmpty', () => {
    const onReset = vi.fn();
    const { getByRole } = render(() => (
      <AppHeader status={() => ''} disabled={() => false} onResetToEmpty={onReset} />
    ));
    fireEvent.click(getByRole('button', { name: 'Start over' }));
    expect(onReset).toHaveBeenCalledTimes(1);
  });

  test('enabled wordmark surfaces the "Start over" tooltip', () => {
    const { getByRole } = render(() => (
      <AppHeader status={() => ''} disabled={() => false} onResetToEmpty={() => undefined} />
    ));
    const wm = getByRole('button', { name: 'Start over' }) as HTMLButtonElement;
    expect(wm.disabled).toBe(false);
    expect(wm.title).toBe('Start over');
  });
});
