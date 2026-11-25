// @vitest-environment jsdom
import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/react';
import Heading from './heading';

afterEach(cleanup);

describe('header layout', () => {
  const pos = (el: Element, ref: Element) => (el.compareDocumentPosition(ref) & Node.DOCUMENT_POSITION_FOLLOWING ? 'before' : 'after');

  it('keeps undo, redo and reset beside the logo, file actions in the centre, run and output on the right', () => {
    const { container } = render(<Heading />);
    const zones = Array.from(container.querySelector('.sm\\:grid')!.children) as HTMLElement[];
    expect(zones).toHaveLength(3);
    const [left, centre, right] = zones;
    const inZone = (z: HTMLElement) => Array.from(z.querySelectorAll('button')).map(b => b.getAttribute('aria-label') ?? b.textContent);
    expect(left.querySelector('img')).toBeTruthy();
    expect(inZone(left)).toEqual(['Undo (Ctrl+Z)', 'Redo (Ctrl+Y)', 'Reset workspace']);
    expect(inZone(centre)).toEqual(['Open workflow', 'Save workflow']); // files are added in the Files card
    expect(inZone(right)).toEqual(['Run the pipeline steps on the canvas', 'View results']);
    expect(pos(left, centre)).toBe('before');
    expect(pos(centre, right)).toBe('before');
  });

  it('centres the middle zone between two equal-width side columns, and wraps the zones on a phone', () => {
    const { container } = render(<Heading />);
    const bar = container.querySelector('.sm\\:grid')!;
    expect(bar.className).toContain('sm:grid-cols-[1fr_auto_1fr]');
    expect(bar.className).toContain('flex-wrap');
  });
});
