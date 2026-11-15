#!/usr/bin/env node
/**
 * Checks that every place that states the app version agrees: package.json, the Cargo workspace,
 * src-tauri/Cargo.toml, src-tauri/tauri.conf.json and the crates in Cargo.lock (the Android version is
 * read from tauri.conf.json at build time).
 *
 *   node scripts/check-version.mjs                 # versions agree
 *   node scripts/check-version.mjs --tag v6.6.0    # ... and match this release tag
 *   node scripts/check-version.mjs --changelog     # ... and CHANGELOG.md has a section for the version
 *
 * Prints the version on success; exits with status 1 and the differences otherwise.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = p => readFileSync(join(root, p), 'utf8');

const tomlVersion = (text, section) => {
  const start = text.indexOf(`[${section}]`);
  if (start < 0) return null;
  const rest = text.slice(start + section.length + 2);
  const end = rest.search(/^\[/m);
  const m = (end < 0 ? rest : rest.slice(0, end)).match(/^version\s*=\s*"([^"]+)"/m);
  return m ? m[1] : null;
};

const lockVersion = (lock, name) => {
  const m = lock.match(new RegExp(`\\[\\[package\\]\\]\\nname = "${name}"\\nversion = "([^"]+)"`));
  return m ? m[1] : null;
};

const lock = read('Cargo.lock');
const found = {
  'package.json': JSON.parse(read('package.json')).version,
  'Cargo.toml [workspace.package]': tomlVersion(read('Cargo.toml'), 'workspace.package'),
  'src-tauri/Cargo.toml': tomlVersion(read('src-tauri/Cargo.toml'), 'package'),
  'src-tauri/tauri.conf.json': JSON.parse(read('src-tauri/tauri.conf.json')).version,
  'Cargo.lock dbflow': lockVersion(lock, 'dbflow'),
  'Cargo.lock dbflow-core': lockVersion(lock, 'dbflow-core'),
  'Cargo.lock dbflow-wasm': lockVersion(lock, 'dbflow-wasm'),
};

const args = process.argv.slice(2);
const tagIndex = args.indexOf('--tag');
const tag = tagIndex >= 0 ? args[tagIndex + 1] : null;
const version = found['package.json'];
const problems = [];

if (!/^\d+\.\d+\.\d+$/.test(version ?? '')) problems.push(`package.json version "${version}" is not MAJOR.MINOR.PATCH`);
for (const [where, v] of Object.entries(found)) {
  if (v !== version) problems.push(`${where}: ${v ?? 'missing'} (package.json says ${version})`);
}
if (tag !== null && tag !== `v${version}`) problems.push(`tag ${tag} does not match version ${version} (expected v${version})`);
if (args.includes('--changelog') && !read('CHANGELOG.md').includes(`## [${version}]`)) {
  problems.push(`CHANGELOG.md has no "## [${version}]" section`);
}

if (problems.length) {
  console.error(`Version check failed:\n- ${problems.join('\n- ')}`);
  process.exit(1);
}
console.log(version);
