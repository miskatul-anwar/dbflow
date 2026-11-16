/**
 * Names the user typed and the on-device naming switch, remembered on this machine (localStorage) by relation signature,
 * i.e. table + columns + key. The same set of columns is found again on the next run and keeps the user's name.
 */
import { isTauriMobile } from '../../utils/platform';

const NAMES_KEY = 'dbflow.relationNames';
const MODEL_KEY = 'dbflow.modelNaming';

export function loadUserNames(): Record<string, string> {
  try {
    const raw = JSON.parse(localStorage.getItem(NAMES_KEY) ?? '{}');
    return raw && typeof raw === 'object' ? (raw as Record<string, string>) : {};
  } catch {
    return {};
  }
}

export function saveUserName(signature: string, name: string): void {
  try {
    const all = loadUserNames();
    if (name.trim()) all[signature] = name.trim();
    else delete all[signature];
    localStorage.setItem(NAMES_KEY, JSON.stringify(all));
  } catch {
    /* storage unavailable: the name lasts for this session only */
  }
}

const namingListeners = new Set<() => void>();
/** The switch when storage is unavailable (private window): kept for this session. */
let memoryChoice: boolean | null = null;

/** Called whenever the naming switch changes (every workspace shows the same switch). */
export function subscribeModelNaming(listener: () => void): () => void {
  namingListeners.add(listener);
  return () => {
    namingListeners.delete(listener);
  };
}

/**
 * Whether this build can run the naming model. Not in the phone app: the model (about 390 MB) needs more memory than
 * Android gives an app's WebView, which Android then kills, closing the app; the Android package leaves it out.
 */
export function modelNamingAvailable(): boolean {
  return !isTauriMobile();
}

/** True when the on-device model names new relations; false when names are built from the key columns. */
export function modelNamingEnabled(): boolean {
  if (!modelNamingAvailable()) return false;
  try {
    return localStorage.getItem(MODEL_KEY) !== 'off';
  } catch {
    return memoryChoice ?? true;
  }
}

/** True once the user has chosen (the switch was used on this device). */
export function modelNamingChosen(): boolean {
  try {
    return localStorage.getItem(MODEL_KEY) !== null;
  } catch {
    return memoryChoice !== null;
  }
}

export function setModelNaming(on: boolean): void {
  try {
    localStorage.setItem(MODEL_KEY, on ? 'on' : 'off');
  } catch {
    /* storage unavailable: the choice lasts for this session only */
    memoryChoice = on;
  }
  namingListeners.forEach(l => l());
}
