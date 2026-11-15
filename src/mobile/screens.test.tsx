// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { PipelineScreen } from './PipelineScreen';
import { DataScreen } from './DataScreen';
import { ResultsScreen } from './ResultsScreen';
import { BottomNav } from './ui';
import { GOALS, planForGoal } from './pipeline';

const idleAsk = { busy: false, message: null, candidates: [] };

describe('phone screens', () => {
  afterEach(cleanup);

  it('pipeline: marks the current goal, picks another, and toggles single steps', () => {
    const onSteps = vi.fn();
    const onToggle = vi.fn();
    render(
      <PipelineScreen steps={planForGoal(GOALS[0])} onSteps={onSteps} onToggle={onToggle} ask={idleAsk} onAsk={() => {}} onPickCandidate={() => {}} />,
    );
    expect(screen.getByRole('button', { name: /Clean schema \(3NF\)/ }).getAttribute('aria-pressed')).toBe('true');
    fireEvent.click(screen.getByRole('button', { name: /Everything/ }));
    expect(onSteps).toHaveBeenCalledWith(planForGoal(GOALS.find(g => g.id === 'all')!));
    const bcnf = screen.getByRole('button', { name: /^BCNF/ });
    expect(bcnf.getAttribute('aria-pressed')).toBe('false');
    fireEvent.click(bcnf);
    expect(onToggle).toHaveBeenCalledWith('BCNF');
  });

  it('pipeline: offers "did you mean" choices from the request box', () => {
    const onPick = vi.fn();
    render(
      <PipelineScreen
        steps={[]}
        onSteps={() => {}}
        onToggle={() => {}}
        ask={{ busy: false, message: 'Did you mean one of these?', candidates: ['3NF', 'BCNF'] }}
        onAsk={() => {}}
        onPickCandidate={onPick}
      />,
    );
    expect(screen.getByText('Did you mean one of these?')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: '3NF' }));
    expect(onPick).toHaveBeenCalledWith('3NF');
  });

  it('data: lists the files and removes one', () => {
    const onRemove = vi.fn();
    const files = [new File(['a,b\n1,2'], 'customers.csv'), new File(['x'], 'shop.xlsx')];
    render(<DataScreen files={files} onAdd={() => {}} onRemove={onRemove} onNext={() => {}} busy={false} />);
    expect(screen.getByText('2 files · 8 B')).toBeTruthy();
    expect(screen.getByText(/Excel workbook/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Remove shop.xlsx' }));
    expect(onRemove).toHaveBeenCalledWith(1);
  });

  it('results: invites a first run when there is nothing yet', () => {
    const onGo = vi.fn();
    render(<ResultsScreen results={null} onGoData={onGo} onMerge={() => {}} />);
    fireEvent.click(screen.getByRole('button', { name: 'Get started' }));
    expect(onGo).toHaveBeenCalled();
  });

  it('bottom bar: shows the file count and marks the current screen', () => {
    const onGo = vi.fn();
    render(<BottomNav screen="pipeline" onGo={onGo} files={3} resultsReady />);
    expect(screen.getByRole('button', { name: /Pipeline/ }).getAttribute('aria-current')).toBe('page');
    expect(screen.getByText('3')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /Results/ }));
    expect(onGo).toHaveBeenCalledWith('results');
  });
});
