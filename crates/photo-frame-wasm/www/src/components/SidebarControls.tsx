import type { JSX } from 'solid-js';
import type { FrameStyle, FrameTheme } from '../frame-client';
import {
  CAPTION_MODES,
  type CaptionMode,
  fromCaptionMode,
  toCaptionMode,
} from '../lib/caption-mode';
import { LONG_EDGE_OPTIONS, type LongEdgeKey, presetDisplayName } from '../lib/long-edge';
import type { AppSettings } from '../state/app-settings';
import { Field } from './Field';
import { Segmented } from './Segmented';
import { advancedBody, advancedGroup, advancedSummary, controls } from './SidebarControls.styles';

// `value` mirrors the Rust enum (`paper`/`ink`), `label` is the
// UI face — direct colour names read more honestly than the
// material metaphors did.
const THEMES = [
  { value: 'paper' as const, label: 'White', description: 'White frame, dark text' },
  { value: 'ink' as const, label: 'Black', description: 'Black frame, light text' },
] satisfies ReadonlyArray<{ value: FrameTheme; label: string; description: string }>;

// Outer silhouette of the framed canvas. Lives next to `Theme`
// because both pick the *visual identity* of the frame; caption is
// strictly downstream (text composition inside the chosen frame).
const FRAME_STYLES = [
  {
    value: 'standard' as const,
    label: 'Standard',
    description: 'Uniform mat with caption strip below',
  },
  {
    value: 'polaroid' as const,
    label: 'Polaroid',
    description: 'Top-anchored photo with a thick caption band underneath',
  },
] satisfies ReadonlyArray<{ value: FrameStyle; label: string; description: string }>;

type Props = {
  settings: AppSettings;
  /** Source long-edge accessor — drives the Long-edge segmented's
   *  disabled flags (caps larger than this can't be reached). */
  sourceLongEdge: () => number | null;
};

// The Quality slider used to live here, but it was a leaky
// abstraction: changing the number didn't snap the Preset
// segmented above back to a sensible state, and a 1-100 dial
// without a live preview doesn't communicate "more / less
// quality" to anyone outside the JPEG encoding world. The
// preset names (SNS / Standard / Maximum) carry the same
// information in user-readable form, so the manual dial is
// gone — `quality` still flows through the signals via
// `applyPreset`, just not editable on its own.
export const SidebarControls = (props: Props): JSX.Element => (
  <div class={controls}>
    <Field label="Preset">
      <Segmented
        options={props.settings.state.presets().map((p) => ({
          value: p.label,
          label: presetDisplayName(p.label),
        }))}
        value={props.settings.state.preset()}
        onChange={props.settings.actions.applyPreset}
        ariaLabel="Quality preset"
      />
    </Field>

    {/* Resolution lives behind a closed-by-default <details>
        because Full is the right choice for almost everyone;
        the picker is here for the minority who deliberately
        want a smaller export. Pushing it down the visual
        hierarchy keeps the primary controls (Preset / Theme
        / Caption) uncluttered without hiding the feature. */}
    <details class={advancedGroup}>
      <summary class={advancedSummary}>Resolution</summary>
      <div class={advancedBody}>
        <Field label="Long edge">
          <Segmented
            options={Object.entries(LONG_EDGE_OPTIONS).map(([key, info]) => {
              const src = props.sourceLongEdge();
              const oversize = info.maxLongEdge !== null && src !== null && info.maxLongEdge > src;
              return {
                value: key as LongEdgeKey,
                label: info.label,
                title: oversize
                  ? `Source is only ${src} px on the long edge — ${info.maxLongEdge} px would be a no-op`
                  : info.maxLongEdge === null
                    ? 'Source size unchanged'
                    : `Cap at ${info.maxLongEdge} px on the long edge`,
                disabled: oversize,
              };
            })}
            value={props.settings.state.longEdge()}
            onChange={props.settings.actions.setLongEdge}
            ariaLabel="Maximum image size"
          />
        </Field>
      </div>
    </details>

    <Field label="Background color">
      <Segmented
        options={THEMES.map((t) => ({ value: t.value, label: t.label, title: t.description }))}
        value={props.settings.state.theme()}
        onChange={props.settings.actions.setTheme}
        ariaLabel="Frame background colour"
      />
    </Field>

    <Field label="Frame">
      <Segmented
        options={FRAME_STYLES.map((s) => ({
          value: s.value,
          label: s.label,
          title: s.description,
        }))}
        value={props.settings.state.frameStyle()}
        onChange={props.settings.actions.setFrameStyle}
        ariaLabel="Frame silhouette"
      />
    </Field>

    {/* Caption is a single 3-state choice rather than the prior
        "Layout" picker + "Show metadata" checkbox: when there's
        no caption, the layout picker has nothing to arrange, so
        a disabled/hidden control was always going to be a kludge.
        Folding the two into one segmented makes the dependency
        explicit — `Off` is its own state, the other two imply
        "show + arrange this way". The Edges/Centered choice is
        independent of `FrameStyle` — Polaroid's bottom band hosts
        either arrangement just as the standard strip does. */}
    <Field label="Caption">
      <Segmented
        options={CAPTION_MODES.map((m) => ({
          value: m.value,
          label: m.label,
          title: m.description,
        }))}
        value={toCaptionMode({
          layout: props.settings.state.layout(),
          metaPolicy: props.settings.state.metaPolicy(),
        })}
        onChange={(v: CaptionMode) => {
          const next = fromCaptionMode(v, props.settings.state.layout());
          props.settings.actions.setMetaPolicy(next.metaPolicy);
          props.settings.actions.setLayout(next.layout);
        }}
        ariaLabel="Caption metadata"
      />
    </Field>
  </div>
);
