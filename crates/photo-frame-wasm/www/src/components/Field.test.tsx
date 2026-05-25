// @vitest-environment jsdom

import { render } from '@solidjs/testing-library';
import { describe, expect, test } from 'vitest';
import { Field } from './Field';

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
    // Pin DOM order: label first, body second.
    expect(container.firstElementChild?.children[0]).toBe(label);
    expect(container.firstElementChild?.children[1]?.firstElementChild).toBe(body);
  });
});
