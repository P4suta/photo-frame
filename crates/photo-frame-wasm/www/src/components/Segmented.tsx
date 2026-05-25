import { For, type JSX } from 'solid-js';
import { segmentedButton } from '../../styled-system/recipes';
import { field, fieldBody, fieldLabel, segmented } from '../App.styles';

/** A single option in a `Segmented`. `title` powers the native
 *  tooltip on hover (used by the Long-edge picker to explain
 *  why a given cap is greyed out); `disabled` greys the button
 *  and skips the click handler. */
export type SegmentedOption<T extends string> = {
  value: T;
  label: string;
  title?: string;
  disabled?: boolean;
};

/** A radiogroup-styled multi-choice picker. The buttons mirror
 *  native `<input type="radio">` semantics via `role="radio"` +
 *  `aria-checked`, but stay as `<button>` elements so they can
 *  pick up the cohesive sidebar styling without inheriting the
 *  default radio look. */
export const Segmented = <T extends string>(props: {
  options: SegmentedOption<T>[];
  value: T;
  onChange: (v: T) => void;
  ariaLabel: string;
}): JSX.Element => (
  <div class={segmented} role="radiogroup" aria-label={props.ariaLabel}>
    <For each={props.options}>
      {(opt) => (
        // biome-ignore lint/a11y/useSemanticElements: segmented buttons keep custom styling; native radios would lose the cohesive look used across the sidebar.
        <button
          type="button"
          role="radio"
          aria-checked={props.value === opt.value}
          title={opt.title}
          disabled={opt.disabled}
          class={segmentedButton({ active: props.value === opt.value })}
          onClick={() => props.onChange(opt.value)}
        >
          {opt.label}
        </button>
      )}
    </For>
  </div>
);

/** Label + body wrapper used by every sidebar field. The label
 *  reads as the small-caps caption above the control body; the
 *  body is the slot for any input (segmented, switch, etc.). */
export const Field = (props: { label: string; children: JSX.Element }): JSX.Element => (
  <div class={field}>
    <div class={fieldLabel}>{props.label}</div>
    <div class={fieldBody}>{props.children}</div>
  </div>
);
