import type { JSX } from 'solid-js';
import { field, fieldBody, fieldLabel } from './Field.styles';

/** Label + body wrapper used by every sidebar field. The label
 *  reads as the small-caps caption above the control body; the
 *  body is the slot for any input (segmented, switch, etc.). */
export const Field = (props: { label: string; children: JSX.Element }): JSX.Element => (
  <div class={field}>
    <div class={fieldLabel}>{props.label}</div>
    <div class={fieldBody}>{props.children}</div>
  </div>
);
