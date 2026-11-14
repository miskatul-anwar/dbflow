#!/usr/bin/env node
/**
 * Builds the Rust engine (crates/dbflow-core) to WebAssembly for the browser build and the tests:
 *   cargo build -p dbflow-wasm --target wasm32-unknown-unknown --release
 *   wasm-bindgen --target web --out-dir src/engine/pkg
 *
 * Skips the build when the output is newer than every Rust source file. The desktop app links the
 * same crate natively (src-tauri), so this step only matters for the webview fallback and Vitest.
 *
 * Requirements: `rustup target add wasm32-unknown-unknown` and `cargo install wasm-bindgen-cli`
 * (the version must match the `wasm-bindgen` crate in crates/dbflow-wasm/Cargo.toml).
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const out = join(root, 'src', 'engine', 'pkg');
const wasmOut = join(out, 'dbflow_wasm_bg.wasm');
const force = process.argv.includes('--force');

function newest(dir) {
  let t = 0;
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    if (e.name === 'target') continue;
    const p = join(dir, e.name);
    if (e.isDirectory()) t = Math.max(t, newest(p));
    else if (/\.(rs|toml)$/.test(e.name)) t = Math.max(t, statSync(p).mtimeMs);
  }
  return t;
}

const sources = Math.max(newest(join(root, 'crates')), statSync(join(root, 'Cargo.toml')).mtimeMs);
if (!force && existsSync(wasmOut) && statSync(wasmOut).mtimeMs >= sources) {
  console.log('[engine] WebAssembly engine is up to date');
  process.exit(0);
}

const run = (cmd, args) => execFileSync(cmd, args, { cwd: root, stdio: 'inherit' });
console.log('[engine] building Rust engine for WebAssembly…');
run('cargo', ['build', '-p', 'dbflow-wasm', '--target', 'wasm32-unknown-unknown', '--release']);
run('wasm-bindgen', [
  '--target', 'web',
  '--out-dir', out,
  '--out-name', 'dbflow_wasm',
  join(root, 'target', 'wasm32-unknown-unknown', 'release', 'dbflow_wasm.wasm'),
]);
console.log('[engine] done:', out);
