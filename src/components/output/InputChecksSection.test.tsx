// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { InputChecksSection } from './InputChecksSection';
import type { InputChecks } from '../../types';

const overlap = {
  files: ['customers.csv', 'customers_v2.csv'] as [string, string],
  rows: [60, 61] as [number, number],
  sharedRows: 57,
  onlyInFirst: 3,
  onlyInSecond: 4,
  key: 'customer_id',
  sharedKeys: 60,
  conflicts: 3,
  conflictKeys: ['5', '17', '33'],
};

describe('InputChecksSection', () => {
  afterEach(cleanup);

  it('renders nothing when the checks found nothing', () => {
    const { container } = render(<InputChecksSection checks={{ skipped: [], duplicates: [], overlaps: [], merged: [] }} />);
    expect(container.innerHTML).toBe('');
  });

  it('lists skipped, identical and overlapping files, and offers the merge', () => {
    const onMerge = vi.fn();
    const checks: InputChecks = {
      skipped: [{ file: 'main.csv', reason: 'main.csv looks like program source code, not a table.' }],
      duplicates: [{ file: 'clients.csv', sameAs: 'customers.csv' }],
      overlaps: [overlap],
      merged: [],
    };
    render(<InputChecksSection checks={checks} onMerge={onMerge} />);
    expect(screen.getByText(/looks like program source code/)).toBeTruthy();
    expect(screen.getByText('clients.csv')).toBeTruthy();
    expect(screen.getByText(/share 57 rows/)).toBeTruthy();
    expect(screen.getByText(/for example 5, 17, 33/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /merge overlapping files and run again/i }));
    expect(onMerge).toHaveBeenCalledTimes(1);
  });

  it('reports a merge and no longer offers one', () => {
    render(
      <InputChecksSection
        checks={{ overlaps: [overlap], merged: [{ into: 'customers.csv', from: ['customers_v2.csv'], rows: 61, conflictsResolved: 3 }] }}
        onMerge={() => {}}
      />,
    );
    expect(screen.getByText(/61 distinct rows/)).toBeTruthy();
    expect(screen.getByText(/3 conflicting records/)).toBeTruthy();
    expect(screen.queryByRole('button', { name: /merge/i })).toBeNull();
  });
});
