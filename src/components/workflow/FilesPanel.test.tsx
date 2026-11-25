// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { FilesPanel, type FilesPanelProps } from './FilesPanel';

afterEach(cleanup);

const csv = (name: string) => new File(['a,b\n1,2\n'], name, { type: 'text/csv' });

function setup(over: Partial<FilesPanelProps> = {}) {
  const props: FilesPanelProps = {
    files: [csv('orders.csv'), csv('customers.csv')],
    onAdd: vi.fn(),
    onRename: vi.fn(() => undefined),
    onReplace: vi.fn(),
    onMove: vi.fn(),
    onRemove: vi.fn(),
    onClear: vi.fn(),
    onDownload: vi.fn(),
    ...over,
  };
  render(<FilesPanel {...props} />);
  return props;
}

/** Radix menus open on pointerdown (or Enter), not on a synthetic click. */
const openMenu = (name: string) => {
  const trigger = screen.getByRole('button', { name: `Actions for ${name}` });
  fireEvent.pointerDown(trigger, { button: 0, ctrlKey: false, pointerType: 'mouse' });
};

describe('FilesPanel', () => {
  it('lists the files with their count', () => {
    setup();
    expect(screen.getByRole('heading', { name: 'Files' })).toBeTruthy();
    expect(screen.getByText('2')).toBeTruthy();
    expect(screen.getByText('orders.csv')).toBeTruthy();
    expect(screen.getByText('customers.csv')).toBeTruthy();
  });

  it('adds several files at once from the picker, and by dropping them', () => {
    const p = setup();
    const input = screen.getByTestId('files-add-input') as HTMLInputElement;
    expect(input.multiple).toBe(true);
    const picked = [csv('a.csv'), csv('b.csv')];
    fireEvent.change(input, { target: { files: picked } });
    expect(p.onAdd).toHaveBeenLastCalledWith(picked);
    const dropped = [csv('c.csv')];
    fireEvent.drop(screen.getByTestId('files-panel'), { dataTransfer: { files: dropped, types: ['Files'] } });
    expect(p.onAdd).toHaveBeenLastCalledWith(dropped);
  });

  it('renames in place on double-click, keeping the extension, and shows a refusal', () => {
    const onRename = vi.fn((_: number, name: string) => (name === 'customers' ? 'Another file is already named customers.csv.' : undefined));
    setup({ onRename });
    fireEvent.doubleClick(screen.getByText('orders.csv'));
    const field = screen.getByRole('textbox', { name: 'New name for orders.csv' }) as HTMLInputElement;
    expect(field.value).toBe('orders');
    fireEvent.change(field, { target: { value: 'customers' } });
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(screen.getByRole('alert').textContent).toMatch(/already named/);
    fireEvent.change(field, { target: { value: 'sales' } });
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(onRename).toHaveBeenLastCalledWith(0, 'sales');
    expect(screen.queryByRole('textbox', { name: /New name/ })).toBeNull();
  });

  it('offers rename, replace, save, move and remove for each file', () => {
    const p = setup();
    openMenu('customers.csv');
    for (const item of ['Rename', 'Replace…', 'Save a copy', 'Move up', 'Move down', 'Remove']) {
      expect(screen.getByRole('menuitem', { name: item })).toBeTruthy();
    }
    expect(screen.getByRole('menuitem', { name: 'Move down' }).getAttribute('data-disabled')).not.toBeNull(); // last file
    fireEvent.click(screen.getByRole('menuitem', { name: 'Move up' }));
    expect(p.onMove).toHaveBeenCalledWith(1, -1);
    openMenu('orders.csv');
    fireEvent.click(screen.getByRole('menuitem', { name: 'Remove' }));
    expect(p.onRemove).toHaveBeenCalledWith(0);
    openMenu('orders.csv');
    fireEvent.click(screen.getByRole('menuitem', { name: 'Save a copy' }));
    expect(p.onDownload).toHaveBeenCalledWith(0);
  });

  it('replaces a file with one picked from disk', () => {
    const p = setup();
    openMenu('orders.csv');
    fireEvent.click(screen.getByRole('menuitem', { name: 'Replace…' }));
    const next = csv('orders-v2.csv');
    fireEvent.change(screen.getByTestId('files-replace-input'), { target: { files: [next] } });
    expect(p.onReplace).toHaveBeenCalledWith(0, next);
  });

  it('removes every file from the header, and invites a first upload when empty', () => {
    const p = setup();
    fireEvent.click(screen.getByRole('button', { name: 'Remove all files' }));
    expect(p.onClear).toHaveBeenCalled();
    cleanup();
    setup({ files: [] });
    expect(screen.getByRole('button', { name: /Add or drop files/ })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Remove all files' })).toBeNull();
  });

  it('is read-only during a run', () => {
    setup({ busy: true });
    expect((screen.getByRole('button', { name: 'Add files' }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.doubleClick(screen.getByText('orders.csv'));
    expect(screen.queryByRole('textbox')).toBeNull();
  });
});
