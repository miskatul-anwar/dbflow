/** True inside the Tauri mobile app (Android or iOS), as opposed to a browser or the desktop app. */
export function isTauriMobile(): boolean {
  if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) return false;
  return /Android|iPhone|iPad/i.test(navigator.userAgent);
}

/**
 * Whether to show the phone interface: in the mobile app, or in a browser with `?ui=mobile` (for development and
 * tests). The desktop app and the browser build otherwise keep the desktop interface.
 */
export function wantsMobileUi(): boolean {
  if (isTauriMobile()) return true;
  try {
    return new URLSearchParams(window.location.search).get('ui') === 'mobile';
  } catch {
    return false;
  }
}
