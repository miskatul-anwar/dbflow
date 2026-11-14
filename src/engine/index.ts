/**
 * Bridge to the Rust engine (crates/dbflow-core).
 *
 * In the desktop app the full analysis runs natively through Tauri commands (see
 * `services/api/engineClient.ts`). The same crate is also compiled to WebAssembly (`./pkg`, built by
 * `scripts/build-engine.mjs`) and loaded here, so browser development, Web Workers and the test
 * suite use exactly the same code. Every engine function exchanges JSON.
 */
import init, * as wasm from './pkg/dbflow_wasm.js';

let ready = false;
let loading: Promise<void> | null = null;

/** Load the WebAssembly engine once (idempotent). */
export function initEngine(source?: BufferSource | URL | string): Promise<void> {
  if (ready) return Promise.resolve();
  loading ??= (async () => {
    const module_or_path = source ?? new URL('./pkg/dbflow_wasm_bg.wasm', import.meta.url);
    await init({ module_or_path });
    ready = true;
  })();
  return loading;
}

/** Synchronous load from bytes (Node and tests). */
export function initEngineSync(bytes: BufferSource): void {
  if (ready) return;
  wasm.initSync({ module: bytes });
  ready = true;
}

export function engineReady(): boolean {
  return ready;
}

function unwrap<T>(json: string): T {
  const r = JSON.parse(json) as { ok?: T; err?: string };
  if (r.err !== undefined) throw new Error(r.err);
  return r.ok as T;
}

function ensureReady(): void {
  if (!ready) throw new Error('The DBFlow engine is not loaded yet (call initEngine() first).');
}

/**
 * JSON for the engine: `undefined` array items become null (as JSON.stringify does) while
 * `undefined` properties are left out, and BigInt values are sent as their decimal text.
 */
function replacer(this: unknown, _key: string, value: unknown): unknown {
  if (typeof value === 'bigint') return value.toString();
  if (value === undefined && Array.isArray(this)) return null;
  return value;
}

/** Call an engine method with positional arguments. */
export function engine<T = any>(method: string, ...args: unknown[]): T {
  ensureReady();
  return unwrap<T>(wasm.call(method, JSON.stringify(args, replacer)));
}

export interface NamingItem {
  key: string;
  columns: string[];
}

export interface EngineFile {
  name: string;
  bytes: Uint8Array;
}

/** Phase 1 of a run: parse and analyse every file. */
export function enginePrepare(
  files: EngineFile[],
  operations: string[],
  options: Record<string, unknown>,
  onProgress: (p: unknown) => void,
): { session: number; naming: NamingItem[] } {
  ensureReady();
  return JSON.parse(
    wasm.prepare(
      JSON.stringify(files.map(f => f.name)),
      files.map(f => f.bytes),
      JSON.stringify(operations),
      JSON.stringify(options),
      (p: string) => onProgress(JSON.parse(p)),
    ),
  );
}

/** Phase 2 of a run: apply names and build the outputs. */
export function engineFinish<T>(session: number, answers: Record<string, string> | null, namerError: string | null, onProgress: (p: unknown) => void): T {
  ensureReady();
  return unwrap<T>(wasm.finish(session, answers ? JSON.stringify(answers) : '', namerError ?? '', (p: string) => onProgress(JSON.parse(p))));
}

export function engineDiscard(session: number): void {
  if (ready) wasm.discard(session);
}

export function parseCsvBytes<T>(bytes: Uint8Array, options: Record<string, unknown> = {}): T {
  ensureReady();
  return unwrap<T>(wasm.parse_csv_bytes(bytes, JSON.stringify(options)));
}

export function decodeBytesRust(bytes: Uint8Array): { text: string; encoding: string } {
  ensureReady();
  return JSON.parse(wasm.decode_bytes(bytes));
}

export function detectEncodingRust(bytes: Uint8Array): string {
  ensureReady();
  return wasm.detect_encoding(bytes);
}

export function spreadsheetToCsvRust(name: string, bytes: Uint8Array): { files: Array<{ name: string; text: string }>; notes: string[] } {
  ensureReady();
  return unwrap(wasm.spreadsheet_to_csv(name, bytes));
}

export function isSpreadsheetRust(name: string, bytes: Uint8Array): boolean {
  ensureReady();
  return wasm.is_spreadsheet(name, bytes);
}
