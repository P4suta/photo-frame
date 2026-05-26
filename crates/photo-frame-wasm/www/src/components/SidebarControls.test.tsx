// @vitest-environment jsdom

import { fireEvent, render, within } from '@solidjs/testing-library';
import { createSignal } from 'solid-js';
import { describe, expect, test, vi } from 'vitest';
import type {
  CaptionLayout,
  FrameStyle,
  FrameTheme,
  MetaPolicy,
  PipelineSpec,
  Preset,
} from '../frame-client';
import type { LongEdgeKey } from '../lib/long-edge';
import type { AppSettings } from '../state/app-settings';
import { SidebarControls } from './SidebarControls';

const FIXTURE_PRESETS: readonly Preset[] = [
  {
    label: 'sns',
    spec: {
      frame_style: 'standard',
      theme: 'paper',
      layout: 'edges',
      meta_policy: 'auto',
      jpeg_quality: 78,
      max_long_edge: 2048,
    },
  },
  {
    label: 'standard',
    spec: {
      frame_style: 'standard',
      theme: 'paper',
      layout: 'edges',
      meta_policy: 'auto',
      jpeg_quality: 92,
      max_long_edge: null,
    },
  },
  {
    label: 'maximum',
    spec: {
      frame_style: 'standard',
      theme: 'paper',
      layout: 'edges',
      meta_policy: 'auto',
      jpeg_quality: 98,
      max_long_edge: null,
    },
  },
];

type FakeSettings = AppSettings & {
  /** Typed handles to the underlying mocks so tests can assert on them. */
  spies: {
    applyPreset: ReturnType<typeof vi.fn>;
    setLongEdge: ReturnType<typeof vi.fn>;
    setFrameStyle: ReturnType<typeof vi.fn>;
    setTheme: ReturnType<typeof vi.fn>;
    setLayout: ReturnType<typeof vi.fn>;
    setMetaPolicy: ReturnType<typeof vi.fn>;
  };
};

const makeFakeSettings = (
  overrides: Partial<{
    preset: string;
    quality: number;
    longEdge: LongEdgeKey;
    frameStyle: FrameStyle;
    theme: FrameTheme;
    layout: CaptionLayout;
    metaPolicy: MetaPolicy;
  }> = {},
): FakeSettings => {
  const [preset] = createSignal<string>(overrides.preset ?? 'standard');
  const [quality] = createSignal<number>(overrides.quality ?? 92);
  const [longEdge] = createSignal<LongEdgeKey>(overrides.longEdge ?? 'full');
  const [frameStyle] = createSignal<FrameStyle>(overrides.frameStyle ?? 'standard');
  const [theme] = createSignal<FrameTheme>(overrides.theme ?? 'paper');
  const [layout] = createSignal<CaptionLayout>(overrides.layout ?? 'edges');
  const [metaPolicy] = createSignal<MetaPolicy>(overrides.metaPolicy ?? 'auto');

  const spies = {
    applyPreset: vi.fn(),
    setLongEdge: vi.fn(),
    setFrameStyle: vi.fn(),
    setTheme: vi.fn(),
    setLayout: vi.fn(),
    setMetaPolicy: vi.fn(),
  };

  return {
    state: {
      preset,
      quality,
      longEdge,
      frameStyle,
      theme,
      layout,
      metaPolicy,
      effectiveMaxLongEdge: () => null as number | null,
      presets: () => FIXTURE_PRESETS,
      buildSpec: (maxLongEdge): PipelineSpec => ({
        frame_style: frameStyle(),
        theme: theme(),
        layout: layout(),
        meta_policy: metaPolicy(),
        jpeg_quality: quality(),
        max_long_edge: maxLongEdge,
      }),
    },
    actions: {
      applyPreset: (k: string) => spies.applyPreset(k),
      setLongEdge: (k: LongEdgeKey) => spies.setLongEdge(k),
      setFrameStyle: (s: FrameStyle) => spies.setFrameStyle(s),
      setTheme: (t: FrameTheme) => spies.setTheme(t),
      setLayout: (l: CaptionLayout) => spies.setLayout(l),
      setMetaPolicy: (m: MetaPolicy) => spies.setMetaPolicy(m),
    },
    spies,
  };
};

describe('<SidebarControls>', () => {
  test('renders the five labelled fields', () => {
    const settings = makeFakeSettings();
    const { getByText } = render(() => (
      <SidebarControls settings={settings} sourceLongEdge={() => null} />
    ));
    expect(getByText('Preset')).toBeTruthy();
    expect(getByText('Resolution')).toBeTruthy();
    expect(getByText('Frame')).toBeTruthy();
    expect(getByText('Background color')).toBeTruthy();
    expect(getByText('Caption')).toBeTruthy();
  });

  test('renders one Preset radio per Rust-side preset (with display-name styling)', () => {
    const settings = makeFakeSettings();
    const { getByRole } = render(() => (
      <SidebarControls settings={settings} sourceLongEdge={() => null} />
    ));
    // The Preset group and the Frame group both contain a "Standard"
    // radio (different meaning, same label), so scope each query to
    // its own radiogroup via the `aria-label` Segmented attaches.
    const presetGroup = within(getByRole('radiogroup', { name: 'Quality preset' }));
    expect(presetGroup.getByRole('radio', { name: 'SNS' })).toBeTruthy();
    expect(presetGroup.getByRole('radio', { name: 'Standard' })).toBeTruthy();
    expect(presetGroup.getByRole('radio', { name: 'Maximum' })).toBeTruthy();
  });

  test('clicking a Preset option fires applyPreset', () => {
    const settings = makeFakeSettings({ preset: 'standard' });
    const { getByRole } = render(() => (
      <SidebarControls settings={settings} sourceLongEdge={() => null} />
    ));
    const presetGroup = within(getByRole('radiogroup', { name: 'Quality preset' }));
    fireEvent.click(presetGroup.getByRole('radio', { name: 'SNS' }));
    expect(settings.spies.applyPreset).toHaveBeenCalledTimes(1);
    expect(settings.spies.applyPreset).toHaveBeenCalledWith('sns');
  });

  test('clicking a Background color option fires setTheme', () => {
    const settings = makeFakeSettings({ theme: 'paper' });
    const { getByRole } = render(() => (
      <SidebarControls settings={settings} sourceLongEdge={() => null} />
    ));
    fireEvent.click(getByRole('radio', { name: 'Black' }));
    expect(settings.spies.setTheme).toHaveBeenCalledTimes(1);
    expect(settings.spies.setTheme).toHaveBeenCalledWith('ink');
  });

  test('clicking the Polaroid frame option fires setFrameStyle', () => {
    const settings = makeFakeSettings({ frameStyle: 'standard' });
    const { getByRole } = render(() => (
      <SidebarControls settings={settings} sourceLongEdge={() => null} />
    ));
    const frameGroup = within(getByRole('radiogroup', { name: 'Frame silhouette' }));
    fireEvent.click(frameGroup.getByRole('radio', { name: 'Polaroid' }));
    expect(settings.spies.setFrameStyle).toHaveBeenCalledTimes(1);
    expect(settings.spies.setFrameStyle).toHaveBeenCalledWith('polaroid');
  });

  test('Long-edge options larger than the source are disabled with explanatory title', () => {
    const settings = makeFakeSettings();
    const { getByRole } = render(() => (
      <SidebarControls settings={settings} sourceLongEdge={() => 1500} />
    ));
    const fourK = getByRole('radio', { name: '4K' }) as HTMLButtonElement;
    const fhd = getByRole('radio', { name: 'FHD' }) as HTMLButtonElement;
    const hd = getByRole('radio', { name: 'HD' }) as HTMLButtonElement;
    const full = getByRole('radio', { name: 'Full' }) as HTMLButtonElement;
    expect(fourK.disabled).toBe(true);
    expect(fhd.disabled).toBe(true);
    expect(hd.disabled).toBe(false);
    expect(full.disabled).toBe(false);
    expect(fourK.title).toContain('1500');
  });

  test("Caption picker — 'Off' fires setMetaPolicy('never') and keeps layout", () => {
    const settings = makeFakeSettings({ layout: 'edges', metaPolicy: 'auto' });
    const { getByRole } = render(() => (
      <SidebarControls settings={settings} sourceLongEdge={() => null} />
    ));
    fireEvent.click(getByRole('radio', { name: 'Off' }));
    expect(settings.spies.setMetaPolicy).toHaveBeenCalledWith('never');
    expect(settings.spies.setLayout).toHaveBeenCalledWith('edges');
  });

  test('Caption picker stays fully active under Polaroid', () => {
    // Frame style and caption arrangement are independent axes —
    // Polaroid's bottom band hosts either Edges or Centered just as
    // the standard strip does.
    const settings = makeFakeSettings({ frameStyle: 'polaroid' });
    const { getByRole } = render(() => (
      <SidebarControls settings={settings} sourceLongEdge={() => null} />
    ));
    const off = getByRole('radio', { name: 'Off' }) as HTMLButtonElement;
    const edges = getByRole('radio', { name: 'Edges' }) as HTMLButtonElement;
    const centered = getByRole('radio', { name: 'Centered' }) as HTMLButtonElement;
    expect(off.disabled).toBe(false);
    expect(edges.disabled).toBe(false);
    expect(centered.disabled).toBe(false);
  });
});
