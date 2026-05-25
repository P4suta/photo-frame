// @vitest-environment jsdom

import { fireEvent, render } from '@solidjs/testing-library';
import { createSignal } from 'solid-js';
import { describe, expect, test, vi } from 'vitest';
import type { CaptionLayout, FrameTheme } from '../frame-client';
import { type LongEdgeKey, type PresetKey, PRESETS } from '../lib/long-edge';
import type { AppSettings } from '../state/app-settings';
import { SidebarControls } from './SidebarControls';

type FakeSettings = AppSettings & {
  /** Typed handles to the underlying mocks so tests can assert on them. */
  spies: {
    applyPreset: ReturnType<typeof vi.fn>;
    setLongEdge: ReturnType<typeof vi.fn>;
    setTheme: ReturnType<typeof vi.fn>;
    setLayout: ReturnType<typeof vi.fn>;
    setShowMeta: ReturnType<typeof vi.fn>;
  };
};

// Build a fake `AppSettings` whose state accessors come from in-test
// signals and whose actions are `vi.fn` spies. The spies are
// re-exposed via `.spies` so tests can assert on them without
// fighting Vitest's loose mock type against AppSettings' strict
// function signatures.
const makeFakeSettings = (
  overrides: Partial<{
    preset: PresetKey;
    quality: number;
    longEdge: LongEdgeKey;
    theme: FrameTheme;
    layout: CaptionLayout;
    showMeta: boolean;
  }> = {},
): FakeSettings => {
  const [preset] = createSignal<PresetKey>(overrides.preset ?? 'standard');
  const [quality] = createSignal<number>(overrides.quality ?? PRESETS.standard.quality);
  const [longEdge] = createSignal<LongEdgeKey>(overrides.longEdge ?? 'full');
  const [theme] = createSignal<FrameTheme>(overrides.theme ?? 'paper');
  const [layout] = createSignal<CaptionLayout>(overrides.layout ?? 'edges');
  const [showMeta] = createSignal<boolean>(overrides.showMeta ?? true);

  const spies = {
    applyPreset: vi.fn(),
    setLongEdge: vi.fn(),
    setTheme: vi.fn(),
    setLayout: vi.fn(),
    setShowMeta: vi.fn(),
  };

  return {
    state: {
      preset,
      quality,
      longEdge,
      theme,
      layout,
      showMeta,
      effectiveMaxLongEdge: () => null as number | null,
      buildFrameOptions: () => ({
        theme: theme(),
        layout: layout(),
        show_meta: showMeta(),
        max_long_edge: null,
      }),
      buildPipelineOptions: () => ({
        theme: theme(),
        layout: layout(),
        show_meta: showMeta(),
        max_long_edge: null,
        jpeg_quality: quality(),
      }),
    },
    actions: {
      applyPreset: (k: PresetKey) => spies.applyPreset(k),
      setLongEdge: (k: LongEdgeKey) => spies.setLongEdge(k),
      setTheme: (t: FrameTheme) => spies.setTheme(t),
      setLayout: (l: CaptionLayout) => spies.setLayout(l),
      setShowMeta: (v: boolean) => spies.setShowMeta(v),
    },
    spies,
  };
};

describe('<SidebarControls>', () => {
  test('renders the four labelled fields', () => {
    const settings = makeFakeSettings();
    const { getByText } = render(() => (
      <SidebarControls settings={settings} sourceLongEdge={() => null} />
    ));
    expect(getByText('Preset')).toBeTruthy();
    expect(getByText('Resolution')).toBeTruthy();
    expect(getByText('Background color')).toBeTruthy();
    expect(getByText('Caption')).toBeTruthy();
  });

  test('clicking a Preset option fires applyPreset', () => {
    const settings = makeFakeSettings({ preset: 'standard' });
    const { getByRole } = render(() => (
      <SidebarControls settings={settings} sourceLongEdge={() => null} />
    ));
    fireEvent.click(getByRole('radio', { name: 'SNS' }));
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

  test('Long-edge options larger than the source are disabled with explanatory title', () => {
    const settings = makeFakeSettings();
    const { getByRole } = render(() => (
      <SidebarControls settings={settings} sourceLongEdge={() => 1500} />
    ));
    // 1500 px source: 4K (3840) and FHD (1920) are oversize, HD
    // (1280) and Full (null) are fine.
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

  test('Caption picker fires setShowMeta + setLayout together', () => {
    const settings = makeFakeSettings({ layout: 'edges', showMeta: true });
    const { getByRole } = render(() => (
      <SidebarControls settings={settings} sourceLongEdge={() => null} />
    ));
    fireEvent.click(getByRole('radio', { name: 'Off' }));
    // 'Off' maps to { showMeta: false, layout: <unchanged> }
    expect(settings.spies.setShowMeta).toHaveBeenCalledWith(false);
    expect(settings.spies.setLayout).toHaveBeenCalledWith('edges');
  });
});
