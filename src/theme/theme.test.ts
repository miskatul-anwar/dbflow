// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const setTheme = vi.fn(async () => {});
let tauri = false;
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => tauri }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ setTheme }) }));

import { applyTheme } from './theme';

const flush = () => new Promise(r => setTimeout(r, 0));

describe('window title bar theme', () => {
  beforeEach(() => localStorage.clear());
  afterEach(() => {
    tauri = false;
    setTheme.mockClear();
  });

  it('follows the chosen theme in the desktop app', async () => {
    tauri = true;
    localStorage.setItem('dbflow.theme', 'dark');
    applyTheme('dark');
    await flush();
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(setTheme).toHaveBeenLastCalledWith('dark');
  });

  it('leaves the title bar to the system while the user has not chosen', async () => {
    tauri = true;
    applyTheme('dark');
    await flush();
    expect(setTheme).toHaveBeenLastCalledWith(null);
  });

  it('does nothing outside the desktop app', async () => {
    applyTheme('dark');
    await flush();
    expect(setTheme).not.toHaveBeenCalled();
  });
});
