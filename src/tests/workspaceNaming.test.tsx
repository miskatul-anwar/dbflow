// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import App from '../App';

beforeAll(() => {
  (globalThis as any).ResizeObserver ??= class { observe() {} unobserve() {} disconnect() {} };
  (globalThis as any).DOMMatrixReadOnly ??= class { m22 = 1; constructor() {} };
});
afterEach(() => {
  cleanup();
  window.history.pushState({}, '', '/');
});

const dialog = () => screen.queryByRole('dialog');
const field = () => screen.getByLabelText('Workspace name') as HTMLInputElement;
const tabTitles = () => Array.from(document.querySelectorAll('.nf-tab-title')).map(e => e.textContent);

describe('naming a workspace', () => {
  it('asks for a name when the app opens, with the default selected, and names the first tab', () => {
    render(<App />);
    expect(dialog()).toBeTruthy();
    expect(screen.getByRole('heading', { name: 'Name your workspace' })).toBeTruthy();
    expect(field().value).toBe('Workspace 1');
    expect(document.activeElement).toBe(field());

    fireEvent.change(field(), { target: { value: 'Sales data' } });
    fireEvent.click(screen.getByRole('button', { name: 'Start' }));
    expect(dialog()).toBeNull();
    expect(tabTitles()).toEqual(['Sales data']);
  });

  it('cannot be dismissed on launch (a workspace must exist) and will not accept an empty name', () => {
    render(<App />);
    fireEvent.keyDown(dialog()!, { key: 'Escape' });
    expect(dialog()).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Cancel' })).toBeNull();
    fireEvent.change(field(), { target: { value: '   ' } });
    expect((screen.getByRole('button', { name: 'Start' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText('Give it a name to continue.')).toBeTruthy();
  });

  it('Enter confirms', () => {
    render(<App />);
    fireEvent.change(field(), { target: { value: 'Orders' } });
    fireEvent.submit(field().closest('form')!);
    expect(tabTitles()).toEqual(['Orders']);
  });

  it('the + button asks for a name before adding the tab, and Cancel adds nothing', () => {
    render(<App />);
    fireEvent.click(screen.getByRole('button', { name: 'Start' }));
    fireEvent.click(screen.getByRole('button', { name: 'New workspace' }));
    expect(screen.getByRole('heading', { name: 'New workspace' })).toBeTruthy();
    expect(tabTitles()).toEqual(['Workspace 1']); // nothing yet

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(dialog()).toBeNull();
    expect(tabTitles()).toEqual(['Workspace 1']);

    fireEvent.click(screen.getByRole('button', { name: 'New workspace' }));
    fireEvent.keyDown(dialog()!, { key: 'Escape' });
    expect(dialog()).toBeNull();
    expect(tabTitles()).toEqual(['Workspace 1']);

    fireEvent.click(screen.getByRole('button', { name: 'New workspace' }));
    fireEvent.change(field(), { target: { value: 'Chinook' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create workspace' }));
    expect(tabTitles()).toEqual(['Workspace 1', 'Chinook']);
    expect(document.querySelector('[aria-current="true"].nf-tab-title')!.textContent).toBe('Chinook');
  });

  it('warns about a duplicate name but still allows it', () => {
    render(<App />);
    fireEvent.click(screen.getByRole('button', { name: 'Start' }));
    fireEvent.click(screen.getByRole('button', { name: 'New workspace' }));
    fireEvent.change(field(), { target: { value: 'workspace 1' } });
    expect(screen.getByText(/already has this name/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Create workspace' }));
    expect(tabTitles().length).toBe(2);
  });

  it('Ctrl+T opens the same dialog', () => {
    render(<App />);
    fireEvent.click(screen.getByRole('button', { name: 'Start' }));
    fireEvent.keyDown(window, { key: 't', ctrlKey: true });
    expect(screen.getByRole('heading', { name: 'New workspace' })).toBeTruthy();
  });

  it('a name chosen here is not overwritten by the first uploaded file', () => {
    render(<App />);
    fireEvent.change(field(), { target: { value: 'My name' } });
    fireEvent.click(screen.getByRole('button', { name: 'Start' }));
    // renamed tabs have autoTitle off, so suggestTitle leaves them alone (covered in useWorkspaceTabs tests)
    expect(tabTitles()).toEqual(['My name']);
  });
});

describe('closing workspaces', () => {
  it('closing the only workspace asks for the name of the one that replaces it (Cancel keeps it)', () => {
    render(<App />);
    fireEvent.click(screen.getByRole('button', { name: 'Start' }));
    fireEvent.click(screen.getByRole('button', { name: 'Close Workspace 1' }));
    expect(screen.getByRole('heading', { name: 'Close “Workspace 1”' })).toBeTruthy();
    expect(screen.getByText(/only workspace, so a new one opens/)).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(tabTitles()).toEqual(['Workspace 1']);

    fireEvent.click(screen.getByRole('button', { name: 'Close Workspace 1' }));
    fireEvent.change(field(), { target: { value: 'Enrollment' } });
    fireEvent.click(screen.getByRole('button', { name: 'Close and create' }));
    expect(dialog()).toBeNull();
    expect(tabTitles()).toEqual(['Enrollment']);
  });

  it('asks in an in-app dialog (never the browser confirm box) before closing a workspace with steps', () => {
    const confirmSpy = vi.spyOn(window, 'confirm');
    render(<App />);
    fireEvent.click(screen.getByRole('button', { name: 'Start' }));
    fireEvent.click(screen.getByRole('button', { name: 'New workspace' }));
    fireEvent.click(screen.getByRole('button', { name: 'Create workspace' }));
    // Give the new workspace a step.
    const pane = document.querySelector('.nf-tab-pane:not([hidden])') as HTMLElement;
    fireEvent.click(within(pane).getAllByText('Data Loader')[0]);
    fireEvent.click(screen.getByRole('button', { name: 'Close Workspace 2' }));

    const alert = screen.getByRole('alertdialog');
    expect(within(alert).getByText(/has 1 pipeline step/)).toBeTruthy();
    // Destructive question: focus starts on Cancel, and the page behind is inert.
    expect(document.activeElement).toBe(within(alert).getByRole('button', { name: 'Cancel' }));
    const app = document.querySelector('.nf-app')!.parentElement!;
    expect(app.hasAttribute('inert')).toBe(true);

    fireEvent.keyDown(alert, { key: 'Escape' });
    expect(tabTitles()).toEqual(['Workspace 1', 'Workspace 2']);
    expect(app.hasAttribute('inert')).toBe(false);

    fireEvent.click(screen.getByRole('button', { name: 'Close Workspace 2' }));
    fireEvent.click(screen.getByRole('button', { name: 'Close workspace' }));
    expect(tabTitles()).toEqual(['Workspace 1']);
    expect(confirmSpy).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });

  it('closes an empty workspace without asking when others remain', () => {
    render(<App />);
    fireEvent.click(screen.getByRole('button', { name: 'Start' }));
    fireEvent.click(screen.getByRole('button', { name: 'New workspace' }));
    fireEvent.click(screen.getByRole('button', { name: 'Create workspace' }));
    fireEvent.click(screen.getByRole('button', { name: 'Close Workspace 2' }));
    expect(dialog()).toBeNull();
    expect(tabTitles()).toEqual(['Workspace 1']);
  });
});
