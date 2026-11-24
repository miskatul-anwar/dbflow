/**
 * Saves generated content as a file the user keeps: the one place every download in the app goes through
 * (the ER diagram, the report PDF, SQL, data files, saved pipelines…).
 *
 * The user is always asked where to put the file, in the system's own file dialog:
 *  - Desktop and Android app: the Tauri "Save as" dialog (on the desktop it opens in Downloads), and the Tauri
 *    file-system plugin writes the file. (Android's WebView cannot follow download links at all.)
 *  - Browser: the File System Access save picker where the browser has one (Chrome, Edge). Browsers without it
 *    (Firefox, Safari) download through a link, which asks only when the browser is set to ask where to save.
 */

import { isTauriMobile } from './platform';

export { isTauriMobile };

function inTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

function extensionOf(name: string): string {
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : '';
}

function downloadLink(blob: Blob, name: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  a.style.display = 'none';
  document.body.appendChild(a);
  a.click();
  a.remove();
  // Revoked after the click has been handled; revoking at once can cancel the download in some browsers.
  setTimeout(() => URL.revokeObjectURL(url), 0);
}

/** Where the desktop dialog opens: the Downloads folder with the suggested name, or just the name. */
async function suggestedPath(name: string): Promise<string> {
  try {
    const { downloadDir, join } = await import('@tauri-apps/api/path');
    return await join(await downloadDir(), name);
  } catch {
    return name;
  }
}

async function saveInApp(blob: Blob, name: string): Promise<boolean> {
  const [{ save }, { writeFile }] = await Promise.all([import('@tauri-apps/plugin-dialog'), import('@tauri-apps/plugin-fs')]);
  const ext = extensionOf(name);
  const path = isTauriMobile()
    ? await save({ defaultPath: name })
    : await save({
        defaultPath: await suggestedPath(name),
        filters: ext ? [{ name: `${ext.toUpperCase()} file`, extensions: [ext] }] : undefined,
      });
  if (!path) return false;
  await writeFile(path, new Uint8Array(await blob.arrayBuffer()));
  return true;
}

interface SavePickerWindow {
  showSaveFilePicker?: (options: {
    suggestedName: string;
    types?: Array<{ description: string; accept: Record<string, string[]> }>;
  }) => Promise<{ createWritable: () => Promise<{ write: (data: Blob) => Promise<void>; close: () => Promise<void> }> }>;
}

/** The browser's save picker; `null` when the browser has none. */
async function saveWithPicker(blob: Blob, name: string): Promise<boolean | null> {
  const picker = (window as unknown as SavePickerWindow).showSaveFilePicker;
  if (typeof picker !== 'function') return null;
  const ext = extensionOf(name);
  const mime = blob.type.split(';')[0] || 'application/octet-stream';
  let handle;
  try {
    handle = await picker.call(window, {
      suggestedName: name,
      types: ext ? [{ description: `${ext.toUpperCase()} file`, accept: { [mime]: [`.${ext}`] } }] : undefined,
    });
  } catch (err) {
    if ((err as { name?: string })?.name === 'AbortError') return false; // the user cancelled
    return null; // not allowed here (e.g. no user gesture left): fall back to a download
  }
  const writable = await handle.createWritable();
  await writable.write(blob);
  await writable.close();
  return true;
}

/**
 * Save `data` as `name`, asking the user where. Resolves to false when the user cancelled, true otherwise.
 * Rejects when the file could not be written.
 */
export async function saveFile(name: string, data: Blob | string, type = 'text/plain'): Promise<boolean> {
  const blob = typeof data === 'string' ? new Blob([data], { type }) : data;
  if (inTauri()) return saveInApp(blob, name);
  const picked = await saveWithPicker(blob, name);
  if (picked !== null) return picked;
  downloadLink(blob, name);
  return true;
}

/** `saveFile` for click handlers: failures are logged instead of being lost in an unhandled promise. */
export function saveFileInBackground(name: string, data: Blob | string, type = 'text/plain'): void {
  saveFile(name, data, type).catch(err => console.error(`Could not save ${name}:`, err));
}
