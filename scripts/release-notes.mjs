#!/usr/bin/env node
/**
 * Release notes for one version: its CHANGELOG.md section, followed by installation notes.
 *
 *   node scripts/release-notes.mjs 6.6.0 > notes.md
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const version = process.argv[2];
if (!version) {
  console.error('usage: release-notes.mjs <version>');
  process.exit(2);
}

const changelog = readFileSync(join(root, 'CHANGELOG.md'), 'utf8');
const start = changelog.indexOf(`## [${version}]`);
let section = '';
if (start >= 0) {
  const body = changelog.slice(changelog.indexOf('\n', start) + 1);
  const next = body.search(/^## \[/m);
  section = (next < 0 ? body : body.slice(0, next)).trim();
}

const repo = process.env.GITHUB_REPOSITORY ?? 'miskatul-anwar/dbflow';
console.log(`${section || `See [CHANGELOG.md](https://github.com/${repo}/blob/main/CHANGELOG.md).`}

## Install

| Platform | File |
|---|---|
| Windows | \`.exe\` (recommended) or \`.msi\` |
| macOS | \`.dmg\` (\`aarch64\` for Apple Silicon, \`x64\` for Intel) |
| Linux | \`.AppImage\`, \`.deb\` or \`.rpm\` |
| Android 7 or newer (64-bit ARM) | \`.apk\` |

> macOS and Windows builds are not code-signed: on macOS right-click → **Open** on first launch; on Windows choose *More info → Run anyway* if SmartScreen asks. On Android, allow your browser or file manager to install apps when asked.

\`SHA256SUMS.txt\` lists the SHA-256 checksum of every file.`);
