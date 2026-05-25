import type { JSX } from 'solid-js';
import { appHeader, brand, headerStatus, tagline, wordmark } from '../App.styles';

type Props = {
  /** Live header status line (set by sessions during long ops). */
  status: () => string;
  /** Wordmark "Start over" button disable state — true in empty
   *  mode (no session to clear). */
  disabled: () => boolean;
  /** Brand-wordmark click target. */
  onResetToEmpty: () => void;
};

export const AppHeader = (props: Props): JSX.Element => (
  <header class={appHeader}>
    <div class={brand}>
      <button
        type="button"
        class={wordmark}
        disabled={props.disabled()}
        aria-label="Start over"
        title={props.disabled() ? undefined : 'Start over'}
        onClick={props.onResetToEmpty}
      >
        photo-frame
      </button>
      <span class={tagline}>Liit-style golden-ratio framing, in your browser.</span>
    </div>
    <div class={headerStatus} aria-live="polite">
      {props.status()}
    </div>
  </header>
);
