// @vitest-environment jsdom

import { fireEvent, render } from '@solidjs/testing-library';
import { describe, expect, test, vi } from 'vitest';
import { Field, Segmented, type SegmentedOption } from './Segmented';

type Picks = 'paper' | 'ink';

const themeOptions: SegmentedOption<Picks>[] = [
  { value: 'paper', label: 'White' },
  { value: 'ink', label: 'Black' },
];

describe('<Segmented>', () => {
  test('renders one button per option with the option label', () => {
    const { getByRole, getAllByRole } = render(() => (
      <Segmented options={themeOptions} value="paper" onChange={() => {}} ariaLabel="Theme" />
    ));
    expect(getByRole('radiogroup', { name: 'Theme' })).toBeTruthy();
    const buttons = getAllByRole('radio');
    expect(buttons).toHaveLength(2);
    expect(buttons[0]?.textContent).toBe('White');
    expect(buttons[1]?.textContent).toBe('Black');
  });

  test('marks the active option with aria-checked="true" and the rest "false"', () => {
    // Mirrors the regression the user hit when the active state
    // was visually indistinguishable — the aria attribute *and*
    // the recipe's `active` variant are tied to the same prop,
    // so a passing test here protects both.
    const { getAllByRole } = render(() => (
      <Segmented options={themeOptions} value="ink" onChange={() => {}} ariaLabel="Theme" />
    ));
    const [white, black] = getAllByRole('radio');
    expect(white?.getAttribute('aria-checked')).toBe('false');
    expect(black?.getAttribute('aria-checked')).toBe('true');
  });

  test('clicking an inactive option fires onChange with that value', () => {
    const onChange = vi.fn();
    const { getByRole } = render(() => (
      <Segmented options={themeOptions} value="paper" onChange={onChange} ariaLabel="Theme" />
    ));
    fireEvent.click(getByRole('radio', { name: 'Black' }));
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith('ink');
  });

  test('disabled options render as disabled and skip onChange on click', () => {
    // The Long-edge picker greys out caps the source can't
    // reach — a disabled button must not fire onChange even
    // if the click event reaches the DOM node.
    const onChange = vi.fn();
    const options: SegmentedOption<Picks>[] = [
      { value: 'paper', label: 'White' },
      { value: 'ink', label: 'Black', disabled: true },
    ];
    const { getByRole } = render(() => (
      <Segmented options={options} value="paper" onChange={onChange} ariaLabel="Theme" />
    ));
    const black = getByRole('radio', { name: 'Black' }) as HTMLButtonElement;
    expect(black.disabled).toBe(true);
    fireEvent.click(black);
    expect(onChange).not.toHaveBeenCalled();
  });

  test('the option `title` surfaces as the button title attribute (tooltip)', () => {
    // The Long-edge picker uses the title to explain why a cap
    // is disabled — pin the wiring so a future refactor doesn't
    // accidentally drop the prop.
    const options: SegmentedOption<Picks>[] = [
      { value: 'paper', label: 'White', title: 'White frame, dark text' },
      { value: 'ink', label: 'Black', title: 'Black frame, light text' },
    ];
    const { getByRole } = render(() => (
      <Segmented options={options} value="paper" onChange={() => {}} ariaLabel="Theme" />
    ));
    expect(getByRole('radio', { name: 'White' }).getAttribute('title')).toBe(
      'White frame, dark text',
    );
  });

  test('clicking the already-active option still fires onChange (idempotent set)', () => {
    // Some pickers (e.g. the Caption mode) treat clicking the
    // active value as a no-op; that decision belongs to the
    // parent component, not the segmented. Pin the contract.
    const onChange = vi.fn();
    const { getByRole } = render(() => (
      <Segmented options={themeOptions} value="paper" onChange={onChange} ariaLabel="Theme" />
    ));
    fireEvent.click(getByRole('radio', { name: 'White' }));
    expect(onChange).toHaveBeenCalledWith('paper');
  });
});

describe('<Field>', () => {
  test('renders the label text above the body content', () => {
    const { getByText, container } = render(() => (
      <Field label="Background color">
        <span>child body</span>
      </Field>
    ));
    const label = getByText('Background color');
    const body = getByText('child body');
    expect(label).toBeTruthy();
    expect(body).toBeTruthy();
    // The label DOM order is "label, then body" — pin it so the
    // visual stacking can't silently invert.
    expect(container.firstElementChild?.children[0]).toBe(label);
    expect(container.firstElementChild?.children[1]?.firstElementChild).toBe(body);
  });
});
