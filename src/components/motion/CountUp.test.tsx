// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import { CountUp } from './CountUp';

afterEach(cleanup);

describe('CountUp', () => {
  it('starts a plain number from 0, so the final figure never flashes first', () => {
    const { container } = render(<CountUp value="120" />);
    expect(container.textContent).toBe('0');
    expect(container.querySelector('[aria-label="120"]')).toBeTruthy();
  });

  it('shows text that is not a plain number as it is', () => {
    const { container } = render(<CountUp value="0.15s" />);
    expect(container.textContent).toBe('0.15s');
  });
});
