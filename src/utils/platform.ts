/** True inside the Tauri mobile app (Android or iOS), as opposed to a browser or the desktop app. */
export function isTauriMobile(): boolean {
  if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) return false;
  return /Android|iPhone|iPad/i.test(navigator.userAgent);
}
