/**
 * Loads the WebAssembly engine synchronously in Node (Vitest, the CLI and benchmark scripts).
 */
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { initEngineSync } from './index';

function wasmPath(): string {
  try {
    const p = fileURLToPath(new URL('./pkg/dbflow_wasm_bg.wasm', import.meta.url));
    if (existsSync(p)) return p;
  } catch {
    // import.meta.url is not a file URL (e.g. jsdom test environment)
  }
  return resolve(process.cwd(), 'src/engine/pkg/dbflow_wasm_bg.wasm');
}

initEngineSync(readFileSync(wasmPath()));
