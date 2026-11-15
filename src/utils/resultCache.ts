/**
 * Result cache — single source of truth for the latest pipeline output of each workspace tab.
 *
 * The full result lives in memory, so large inputs never hit the storage
 * quota. A slimmed copy (row arrays cut to a preview) goes to sessionStorage
 * so a page reload still shows the schema, ERD, SQL and report. Anything that
 * does not fit is dropped, never half-written.
 */

const STORAGE_KEY = 'dbflow_latest_results';
const DEFAULT_KEY = 'default';
const storageKey = (key: string) => (key === DEFAULT_KEY ? STORAGE_KEY : `${STORAGE_KEY}:${key}`);
const ROW_PREVIEW = 50;

const memory = new Map<string, unknown>();

/** Generated files larger than this stay in memory only (sessionStorage holds about 5 MB in total). */
const MAX_STORED_CONTENT = 200_000;
export const DROPPED_CONTENT_NOTE = 'This file was too large to keep across a reload. Run the pipeline again to download it.';

/** JSON replacer that keeps only a preview of row arrays and leaves very large file contents out. */
function slimReplacer(key: string, value: unknown): unknown {
  if (key === 'rows' && Array.isArray(value) && value.length > ROW_PREVIEW) {
    return value.slice(0, ROW_PREVIEW);
  }
  if (key === 'content' && typeof value === 'string' && value.length > MAX_STORED_CONTENT) {
    return DROPPED_CONTENT_NOTE;
  }
  return value;
}

const pendingWrites = new Map<string, ReturnType<typeof setTimeout>>();

function writeToStorage(result: unknown, key: string): void {
  try {
    sessionStorage.setItem(storageKey(key), JSON.stringify(result, slimReplacer));
  } catch {
    // Quota exceeded or storage unavailable: the in-memory copy still serves this session.
    try {
      sessionStorage.removeItem(storageKey(key));
    } catch {
      // ignore
    }
  }
}

export function loadCachedResult<T = unknown>(key: string = DEFAULT_KEY): T | null {
  if (memory.get(key)) return memory.get(key) as T;
  try {
    const cached = sessionStorage.getItem(storageKey(key));
    if (!cached) return null;
    const parsed = JSON.parse(cached);
    if (parsed && (parsed.success || parsed.tables)) return parsed as T;
  } catch {
    // Corrupt cache — ignore and treat as empty.
  }
  return null;
}

/**
 * Keeps the result in memory at once; the reload copy is written a moment later, off the path of the
 * click that produced it, so serialising a large result never freezes the page.
 */
export function saveCachedResult(result: unknown, key: string = DEFAULT_KEY): void {
  if (memory.get(key) === result) return;
  memory.set(key, result);
  const queued = pendingWrites.get(key);
  if (queued !== undefined) clearTimeout(queued);
  pendingWrites.set(
    key,
    setTimeout(() => {
      pendingWrites.delete(key);
      if (memory.get(key) === result) writeToStorage(result, key);
    }, 250),
  );
}

export function clearCachedResult(key: string = DEFAULT_KEY): void {
  memory.delete(key);
  const queued = pendingWrites.get(key);
  if (queued !== undefined) {
    clearTimeout(queued);
    pendingWrites.delete(key);
  }
  try {
    sessionStorage.removeItem(storageKey(key));
  } catch {
    // ignore
  }
}
