// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const save = vi.fn();
const writeFile = vi.fn();
vi.mock('@tauri-apps/plugin-dialog', () => ({ save }));
vi.mock('@tauri-apps/plugin-fs', () => ({ writeFile }));
vi.mock('@tauri-apps/api/path', () => ({
  downloadDir: async () => '/home/me/Downloads',
  join: async (...parts: string[]) => parts.join('/'),
}));

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
    delete (window as unknown as Record<string, unknown>).showSaveFilePicker;
  });

  it('falls back to a download link in a browser without a save picker', async () => {
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    expect(isTauriMobile()).toBe(false);
    await expect(saveFile('schema.sql', 'CREATE TABLE t (id int);', 'application/sql')).resolves.toBe(true);
    expect(click).toHaveBeenCalledTimes(1);
    expect(save).not.toHaveBeenCalled();
  });

  it("asks where to save in a browser with a save picker, and writes there", async () => {
    const write = vi.fn();
    const close = vi.fn();
    const picker = vi.fn(async () => ({ createWritable: async () => ({ write, close }) }));
    (window as unknown as Record<string, unknown>).showSaveFilePicker = picker;
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    await expect(saveFile('erd.svg', '<svg/>', 'image/svg+xml')).resolves.toBe(true);
    expect(picker).toHaveBeenCalledWith({
      suggestedName: 'erd.svg',
      types: [{ description: 'SVG file', accept: { 'image/svg+xml': ['.svg'] } }],
    });
    expect(write).toHaveBeenCalledTimes(1);
    expect(close).toHaveBeenCalledTimes(1);
    expect(click).not.toHaveBeenCalled();
  });

  it('discards an unfinished write and reports the failure', async () => {
    const abort = vi.fn(async () => {});
    const write = vi.fn(async () => Promise.reject(new Error('disk full')));
    (window as unknown as Record<string, unknown>).showSaveFilePicker = vi.fn(async () => ({
      createWritable: async () => ({ write, close: vi.fn(), abort }),
    }));
    await expect(saveFile('data.csv', 'a,b')).rejects.toThrow('disk full');
    expect(abort).toHaveBeenCalledTimes(1);
  });

  it('saves nothing when the browser save picker is cancelled', async () => {
    const abort = Object.assign(new Error('cancelled'), { name: 'AbortError' });
    (window as unknown as Record<string, unknown>).showSaveFilePicker = vi.fn(async () => Promise.reject(abort));
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    await expect(saveFile('report.pdf', new Blob(['%PDF'], { type: 'application/pdf' }))).resolves.toBe(false);
    expect(click).not.toHaveBeenCalled();
  });

  it('asks where to save in the desktop app, starting in Downloads, and writes the bytes there', async () => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    save.mockResolvedValue('/home/me/Documents/report.pdf');
    await expect(saveFile('report.pdf', 'abc', 'application/pdf')).resolves.toBe(true);
    expect(save).toHaveBeenCalledWith({
      defaultPath: '/home/me/Downloads/report.pdf',
      filters: [{ name: 'PDF file', extensions: ['pdf'] }],
    });
    expect(writeFile).toHaveBeenCalledWith('/home/me/Documents/report.pdf', new Uint8Array([97, 98, 99]));
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
