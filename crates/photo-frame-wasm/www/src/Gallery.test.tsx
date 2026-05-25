// @vitest-environment jsdom

import { render } from '@solidjs/testing-library';
import { describe, expect, test } from 'vitest';
import { Gallery, type GalleryRow } from './Gallery';

const rowOf = (overrides: Partial<GalleryRow> & { key: string; name: string }): GalleryRow => ({
  status: 'queued',
  ...overrides,
});

describe('<Gallery>', () => {
  test('renders one <li> per row in the input order', () => {
    const rows: GalleryRow[] = [
      rowOf({ key: 'a.jpg', name: 'a.jpg' }),
      rowOf({ key: 'b.jpg', name: 'b.jpg' }),
      rowOf({ key: 'c.jpg', name: 'c.jpg' }),
    ];
    const { container } = render(() => <Gallery rows={rows} />);
    const items = Array.from(container.querySelectorAll('li'));
    expect(items).toHaveLength(3);
    expect(items.map((li) => li.getAttribute('data-status'))).toEqual([
      'queued',
      'queued',
      'queued',
    ]);
  });

  test('writes the row status into the data-status attribute', () => {
    const rows: GalleryRow[] = [
      rowOf({ key: 'q.jpg', name: 'q.jpg', status: 'queued' }),
      rowOf({ key: 'p.jpg', name: 'p.jpg', status: 'processing' }),
      rowOf({ key: 'd.jpg', name: 'd.jpg', status: 'done' }),
      rowOf({ key: 'e.jpg', name: 'e.jpg', status: 'error' }),
    ];
    const { container } = render(() => <Gallery rows={rows} />);
    const statuses = Array.from(container.querySelectorAll('li')).map((li) =>
      li.getAttribute('data-status'),
    );
    expect(statuses).toEqual(['queued', 'processing', 'done', 'error']);
  });

  test('shows the human-readable status text from `statusLabel`', () => {
    const rows: GalleryRow[] = [
      rowOf({ key: 'q.jpg', name: 'q.jpg', status: 'queued' }),
      rowOf({ key: 'p.jpg', name: 'p.jpg', status: 'processing' }),
      rowOf({ key: 'd.jpg', name: 'd.jpg', status: 'done' }),
      rowOf({ key: 'e.jpg', name: 'e.jpg', status: 'error' }),
    ];
    const { getByText } = render(() => <Gallery rows={rows} />);
    expect(getByText('Queued')).toBeTruthy();
    expect(getByText('Processing')).toBeTruthy();
    expect(getByText('✓ Done')).toBeTruthy();
    expect(getByText('✗ Error')).toBeTruthy();
  });

  test('paints a placeholder div when no thumbnailUrl is present', () => {
    const rows: GalleryRow[] = [rowOf({ key: 'pending.jpg', name: 'pending.jpg' })];
    const { container } = render(() => <Gallery rows={rows} />);
    expect(container.querySelector('img')).toBeNull();
    // The placeholder is a leaf div inside the thumb wrapper.
    expect(container.querySelector('.gallery-thumb')?.firstElementChild?.tagName).toBe('DIV');
  });

  test('paints an <img> with src=thumbnailUrl when one is present', () => {
    const rows: GalleryRow[] = [
      rowOf({
        key: 'ok.jpg',
        name: 'ok.jpg',
        status: 'processing',
        thumbnailUrl: 'blob:test/1',
      }),
    ];
    const { container } = render(() => <Gallery rows={rows} />);
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('src')).toBe('blob:test/1');
    // Decorative — the visible file name carries the identity.
    expect(img?.getAttribute('alt')).toBe('');
  });

  test('the row name becomes the title attribute (full-name hover)', () => {
    // Long file names get truncated visually; the title carries
    // the full name so the user can hover to read it. Pin the
    // wiring so a future style change can't quietly drop it.
    const longName = 'a-really-long-camera-export-name-from-the-card.JPG';
    const rows: GalleryRow[] = [rowOf({ key: longName, name: longName })];
    const { container } = render(() => <Gallery rows={rows} />);
    const nameEl = container.querySelector('[title]');
    expect(nameEl?.getAttribute('title')).toBe(longName);
    expect(nameEl?.textContent).toBe(longName);
  });
});
