// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const save = vi.fn();
const writeFile = vi.fn();
vi.mock('@tauri-apps/plugin-dialog', () => ({ save }));
vi.mock('@tauri-apps/plugin-fs', () => ({ writeFile }));

import { isTauriMobile, saveFile } from './saveFile';

const ua = (value: string) => vi.spyOn(navigator, 'userAgent', 'get').mockReturnValue(value);
const ANDROID = 'Mozilla/5.0 (Linux; Android 14; Pixel 7) AppleWebKit/537.36 Chrome/131 Mobile Safari/537.36';

describe('saveFile', () => {
  beforeEach(() => {
    URL.createObjectURL = vi.fn(() => 'blob:x');
    URL.revokeObjectURL = vi.fn();
  });
  afterEach(() => {
    vi.restoreAllMocks();
    save.mockReset();
    writeFile.mockReset();
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  });

  it('downloads through a link in a browser and on the desktop', async () => {
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    expect(isTauriMobile()).toBe(false);
    await expect(saveFile('schema.sql', 'CREATE TABLE t (id int);', 'application/sql')).resolves.toBe(true);
    expect(click).toHaveBeenCalledTimes(1);
    expect(save).not.toHaveBeenCalled();
  });

  it('asks where to save and writes the bytes in the Android app', async () => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    ua(ANDROID);
    expect(isTauriMobile()).toBe(true);
    save.mockResolvedValue('content://downloads/schema.sql');
    await expect(saveFile('schema.sql', 'abc')).resolves.toBe(true);
    expect(save).toHaveBeenCalledWith({ defaultPath: 'schema.sql' });
    expect(writeFile).toHaveBeenCalledWith('content://downloads/schema.sql', new Uint8Array([97, 98, 99]));
  });

  it('writes nothing when the save picker is cancelled', async () => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    ua(ANDROID);
    save.mockResolvedValue(null);
    await expect(saveFile('a.csv', 'x')).resolves.toBe(false);
    expect(writeFile).not.toHaveBeenCalled();
  });
});
