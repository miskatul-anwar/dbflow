// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { TitleTooltips } from './TitleTooltips';

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe('TitleTooltips', () => {
  it("shows a title as the app's tooltip, hides the native one meanwhile, and restores it on leave", () => {
    vi.useFakeTimers();
    render(
      <>
        <TitleTooltips />
        <div title="Validates scalar values">Atomicity Checker</div>
      </>,
    );
    const node = screen.getByText('Atomicity Checker');
    fireEvent.pointerOver(node);
    expect(node.getAttribute('title')).toBeNull(); // no native tooltip as well
    act(() => vi.advanceTimersByTime(400));
    expect(screen.getByRole('tooltip').textContent).toBe('Validates scalar values');
    fireEvent.pointerOut(node, { relatedTarget: document.body });
    expect(node.getAttribute('title')).toBe('Validates scalar values');
  });

  it('shows nothing for an element without a title', () => {
    vi.useFakeTimers();
    render(
      <>
        <TitleTooltips />
        <div>plain</div>
      </>,
    );
    fireEvent.pointerOver(screen.getByText('plain'));
    act(() => vi.advanceTimersByTime(400));
    expect(screen.queryByRole('tooltip')).toBeNull();
  });
});
