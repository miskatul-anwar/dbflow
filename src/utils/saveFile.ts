/**
 * Saves generated content as a file the user keeps: the one place every download in the app goes through.
 *
 * In a browser and in the desktop app a temporary link downloads the file. The Android app's WebView cannot follow
 * such links (the click does nothing), so there the system "Save as" picker asks where to put the file and the Tauri
 * file-system plugin writes it.
 */

import { isTauriMobile } from './platform';

export { isTauriMobile };

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

/**
 * Save `data` as `name`. Resolves to false when the user cancelled the save picker (Android), true otherwise.
 * Rejects when the file could not be written.
 */
export async function saveFile(name: string, data: Blob | string, type = 'text/plain'): Promise<boolean> {
  const blob = typeof data === 'string' ? new Blob([data], { type }) : data;
  if (!isTauriMobile()) {
    downloadLink(blob, name);
    return true;
  }
  const [{ save }, { writeFile }] = await Promise.all([import('@tauri-apps/plugin-dialog'), import('@tauri-apps/plugin-fs')]);
  const path = await save({ defaultPath: name });
  if (!path) return false;
  await writeFile(path, new Uint8Array(await blob.arrayBuffer()));
  return true;
}

/** `saveFile` for click handlers: failures are logged instead of being lost in an unhandled promise. */
export function saveFileInBackground(name: string, data: Blob | string, type = 'text/plain'): void {
  saveFile(name, data, type).catch(err => console.error(`Could not save ${name}:`, err));
}
