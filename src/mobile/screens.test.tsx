// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { PipelineScreen, type PipelineControls } from './PipelineScreen';
import { NLPanel } from './NLPanel';
import { DataScreen } from './DataScreen';
import { ResultsScreen } from './ResultsScreen';
import { WorkspacesSheet, WorkspaceForm } from './WorkspaceSheets';
import { BottomNav } from './ui';
import { GOALS, planForGoal } from './pipeline';
import type { NLWorkflow } from '../hooks/useNLWorkflow';

const controls = (steps: string[], over: Partial<PipelineControls> = {}): PipelineControls => ({
  steps,
  canUndo: true,
  canRedo: false,
  undo: vi.fn(),
  redo: vi.fn(),
  setSteps: vi.fn(),
  toggle: vi.fn(),
  save: vi.fn(),
  open: vi.fn(),
  ...over,
});

const nlStub = (over: Partial<NLWorkflow> = {}): NLWorkflow =>
  ({
    query: '',
    resolution: null,
    busy: false,
    embeddings: 'offline',
    hasNodes: true,
    placed: ['Data Loader'],
    hasFiles: true,
    submit: vi.fn(),
    pickCandidate: vi.fn(),
    apply: vi.fn(),
    dismiss: vi.fn(),
    canUndoBuild: false,
    undoBuild: vi.fn(),
    ...over,
  }) as NLWorkflow;

describe('phone screens', () => {
  afterEach(cleanup);

  it('pipeline: undo/redo/open/save, goals, filter and the step checklist', () => {
    const c = controls(planForGoal(GOALS[0]));
    render(<PipelineScreen pipeline={c} nl={nlStub()} />);
    fireEvent.click(screen.getByRole('button', { name: /Undo/ }));
    expect(c.undo).toHaveBeenCalled();
    expect((screen.getByRole('button', { name: /Redo/ }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: /Save/ }));
    expect(c.save).toHaveBeenCalled();
    expect(screen.getByRole('radio', { name: /3NF schema/ }).getAttribute('aria-checked')).toBe('true');
    fireEvent.click(screen.getByRole('radio', { name: /All steps/ }));
    expect(c.setSteps).toHaveBeenCalledWith(planForGoal(GOALS.find(g => g.id === 'all')!));
    const bcnf = screen.getByRole('checkbox', { name: /^BCNF/ });
    expect(bcnf.getAttribute('aria-checked')).toBe('false');
    fireEvent.click(bcnf);
    expect(c.toggle).toHaveBeenCalledWith('BCNF');
    fireEvent.change(screen.getByRole('searchbox', { name: 'Filter steps' }), { target: { value: 'foreign' } });
    expect(screen.getByRole('checkbox', { name: /^Relation Detector/ })).toBeTruthy();
    expect(screen.queryByRole('checkbox', { name: /^BCNF/ })).toBeNull();
  });

  it('request box: previews the plan and applies it with Replace, Extend or Apply & run', () => {
    const resolution = {
      targets: ['BCNF'],
      excluded: [],
      plan: ['Data Loader', 'BCNF'],
      addedAsDependencies: ['Data Loader'],
      options: { dialect: 'mysql' },
      confidence: 'high',
      candidates: [],
      source: 'lexical',
    };
    const nl = nlStub({ resolution: resolution as never, query: 'bcnf in mysql' });
    render(<NLPanel nl={nl} />);
    expect(screen.getByText('2 steps')).toBeTruthy();
    expect(screen.getByText('SQL dialect: mysql')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Replace' }));
    expect(nl.apply).toHaveBeenCalledWith(resolution, { run: false, mode: 'replace' });
    fireEvent.click(screen.getByRole('button', { name: /Apply & run/ }));
    expect(nl.apply).toHaveBeenCalledTimes(2);
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
    expect(nl.dismiss).toHaveBeenCalled();
  });

  it('request box: offers "did you mean" choices and the undo of the last build', () => {
    const nl = nlStub({
      resolution: { plan: [], candidates: [{ operation: '3NF', score: 0.4 }], message: 'Did you mean…', addedAsDependencies: [], targets: [], options: {} } as never,
    });
    const { unmount } = render(<NLPanel nl={nl} />);
    fireEvent.click(screen.getByRole('button', { name: '3NF' }));
    expect(nl.pickCandidate).toHaveBeenCalledWith('3NF');
    unmount();
    const undo = nlStub({ canUndoBuild: true });
    render(<NLPanel nl={undo} />);
    fireEvent.click(screen.getByRole('button', { name: 'Undo last build' }));
    expect(undo.undoBuild).toHaveBeenCalled();
  });

  it('workspaces: switch, edit, close and create', () => {
    const on = { onSelect: vi.fn(), onNew: vi.fn(), onEdit: vi.fn(), onClose: vi.fn(), onDismiss: vi.fn() };
    render(
      <WorkspacesSheet
        tabs={[
          { id: 't1', title: 'Sales', autoTitle: false, color: '#2563eb' },
          { id: 't2', title: 'HR', autoTitle: false, color: null },
        ]}
        activeId="t1"
        status={{ t1: { running: true, operations: 16 }, t2: { running: false, operations: 4 } }}
        {...on}
      />,
    );
    expect(screen.getByText('Running')).toBeTruthy();
    expect(screen.getByText('4 steps')).toBeTruthy();
    fireEvent.click(screen.getByText('HR'));
    expect(on.onSelect).toHaveBeenCalledWith('t2');
    fireEvent.click(screen.getByRole('button', { name: 'Close HR' }));
    expect(on.onClose).toHaveBeenCalledWith('t2');
    fireEvent.click(screen.getByRole('button', { name: /New workspace/ }));
    expect(on.onNew).toHaveBeenCalled();
  });

  it('workspace form: renames and picks a colour', () => {
    const onConfirm = vi.fn();
    render(<WorkspaceForm title="Edit workspace" initialName="Sales" initialColor={null} withColor confirmLabel="Save" onConfirm={onConfirm} onDismiss={() => {}} />);
    fireEvent.change(screen.getByRole('textbox', { name: 'Workspace name' }), { target: { value: 'Sales 2026' } });
    fireEvent.click(screen.getByRole('button', { name: 'Violet' }));
    fireEvent.click(screen.getByRole('button', { name: /Save/ }));
    expect(onConfirm).toHaveBeenCalledWith('Sales 2026', '#7c3aed');
  });

  it('data: lists the files and removes one', () => {
    const onRemove = vi.fn();
    const files = [new File(['a,b\n1,2'], 'customers.csv'), new File(['x'], 'shop.xlsx')];
    render(<DataScreen files={files} onAdd={() => {}} onRemove={onRemove} busy={false} />);
    expect(screen.getByText('customers.csv')).toBeTruthy();
    expect(screen.getByText('XLSX')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Remove shop.xlsx' }));
    expect(onRemove).toHaveBeenCalledWith(1);
  });

  it('results: points to Data when there is nothing yet', () => {
    const onGo = vi.fn();
    render(<ResultsScreen results={null} onGoData={onGo} onMerge={() => {}} />);
    fireEvent.click(screen.getByRole('button', { name: 'Go to Data' }));
    expect(onGo).toHaveBeenCalled();
  });

  it('bottom bar: shows the file count, new results, and marks the current screen', () => {
    const onGo = vi.fn();
    render(<BottomNav screen="pipeline" onGo={onGo} files={3} resultsReady />);
    expect(screen.getByRole('button', { name: /Pipeline/ }).getAttribute('aria-current')).toBe('page');
    expect(screen.getByText('3')).toBeTruthy();
    expect(screen.getByText('new')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /Results/ }));
    expect(onGo).toHaveBeenCalledWith('results');
  });
});
