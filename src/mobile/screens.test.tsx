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
    expect(screen.getByRole('radio', { name: /3NF schema/ }).getAttribute('aria-checked')).toBe('true');
    fireEvent.click(screen.getByRole('radio', { name: /All steps/ }));
    expect(onSteps).toHaveBeenCalledWith(planForGoal(GOALS.find(g => g.id === 'all')!));
    expect(screen.getByRole('checkbox', { name: /^Data Loader/ }).getAttribute('aria-checked')).toBe('true');
    const bcnf = screen.getByRole('checkbox', { name: /^BCNF/ });
    expect(bcnf.getAttribute('aria-checked')).toBe('false');
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
    render(<DataScreen files={files} onAdd={() => {}} onRemove={onRemove} busy={false} />);
    expect(screen.getByText('customers.csv')).toBeTruthy();
    expect(screen.getByText('XLSX')).toBeTruthy();
    expect(screen.getByText('7 B')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Remove shop.xlsx' }));
    expect(onRemove).toHaveBeenCalledWith(1);
  });

  it('results: invites a first run when there is nothing yet', () => {
    const onGo = vi.fn();
    render(<ResultsScreen results={null} onGoData={onGo} onMerge={() => {}} />);
    fireEvent.click(screen.getByRole('button', { name: 'Go to Data' }));
    expect(onGo).toHaveBeenCalled();
  });

  it('bottom bar: shows the file count and marks the current screen', () => {
    const onGo = vi.fn();
    render(<BottomNav screen="pipeline" onGo={onGo} files={3} resultsReady />);
    expect(screen.getByRole('button', { name: /Pipeline/ }).getAttribute('aria-current')).toBe('page');
    expect(screen.getByText('3')).toBeTruthy();
    expect(screen.getByText('new')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /Results/ }));
    expect(onGo).toHaveBeenCalledWith('results');
  });
});
