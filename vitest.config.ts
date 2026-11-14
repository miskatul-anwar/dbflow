import { defineConfig } from 'vitest/config';
import path from 'node:path';

export default defineConfig({
  resolve: {
    alias: {
      '@': path.resolve(__dirname, 'src'),
    },
  },
  test: {
    environment: 'node',
    include: ['src/**/*.test.{ts,tsx}', 'tests/site/**/*.test.mjs'],
    // The engine is Rust compiled to WebAssembly; load it before any test runs.
    setupFiles: ['src/engine/nodeInit.ts'],
    // Spreadsheet fixtures are written by SheetJS, whose date serials depend on the local time zone.
    env: { TZ: 'UTC' },
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov'],
      include: ['src/services/**', 'src/utils/**', 'src/hooks/**'],
    },
  },
});
